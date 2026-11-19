//! pidfile.rs — the supervisor's `engine.pid` read as an IDENTITY before anything is killed: a pid line, then the
//! `start=` and `exe=` `engine/k7-supervisor/src/PidFile.cpp` writes. Only a process the OS describes the same way,
//! with an executable named `k7-runtime`, may be killed: a bare pid is a number the OS may have handed to anything.

use std::path::Path;
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;

/// The pidfile's identity, or what the OS says of a live pid now.
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub pid: u32,
    pub start: Option<String>,
    pub exe: Option<String>,
}

/// The verdict of `PidFile.cpp`'s `check`, the same on both sides.
#[derive(Debug, PartialEq)]
pub enum Check {
    /// The very process the file names: the one case that may be killed.
    Same,
    /// Alive, but nothing proves it is the engine: left alone, and so is the file.
    Unverified,
    /// Gone, or a process that is not the engine: the file is out of date.
    Stale,
}

/// The pid on the first line, `start=` and `exe=` after it; None when the first line is not a pid.
pub fn parse(text: &str) -> Option<Record> {
    let mut lines = text.lines();
    let head = lines.next()?.trim();
    if head.is_empty() || head.len() > 9 || !head.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let pid: u32 = head.parse().ok().filter(|&p| p > 0)?;
    let mut rec = Record {
        pid,
        start: None,
        exe: None,
    };
    for line in lines.map(str::trim) {
        if let Some(s) = line.strip_prefix("start=") {
            if !s.is_empty() && s.len() <= 20 && s.bytes().all(|b| b.is_ascii_digit()) {
                rec.start = Some(s.to_string());
            }
        } else if let Some(e) = line.strip_prefix("exe=") {
            rec.exe = Some(e.to_string());
        }
    }
    Some(rec)
}

/// `engine.pid` as it is on disk now; None when it cannot be read or holds no pid.
pub fn read(path: &Path) -> Option<Record> {
    parse(&std::fs::read_to_string(path).ok()?)
}

pub fn is_engine_exe(exe: &str) -> bool {
    let name = exe
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(exe)
        .to_ascii_lowercase();
    name == "k7-runtime" || name == "k7-runtime.exe"
}

/// Only `Same` may be killed; a live process that is not the engine is never ours, whatever the file says.
pub fn check(recorded: &Record, live: Option<&Record>, alive: bool) -> Check {
    if !alive {
        return Check::Stale;
    }
    let Some((Some(start), Some(exe))) = live.map(|l| (l.start.as_ref(), l.exe.as_ref())) else {
        return Check::Unverified;
    };
    if !is_engine_exe(exe) {
        return Check::Stale;
    }
    match &recorded.start {
        None => Check::Unverified,
        Some(s) if s == start => Check::Same,
        Some(_) => Check::Stale,
    }
}

/// The output of a short command in the C locale, or None when it fails; never a console window on Windows.
fn output(bin: &str, args: &[&str]) -> Option<String> {
    let mut cmd = Command::new(bin);
    cmd.args(args).env("LC_ALL", "C");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let out = cmd.output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(windows)]
fn system32(file: &str) -> String {
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_string());
    format!("{root}\\System32\\{file}")
}

/// `ps -o lstart=` in the C locale, as local seconds since the epoch: what macOS's `pbi_start_tvsec` holds.
pub fn lstart_seconds(text: &str) -> Option<String> {
    use chrono::{Local, NaiveDateTime, TimeZone};
    let words = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let naive = NaiveDateTime::parse_from_str(&words, "%a %b %d %H:%M:%S %Y").ok()?;
    let at = Local.from_local_datetime(&naive).earliest()?;
    Some(at.timestamp().to_string())
}

/// What the OS says of `pid` now: its start time as `PidFile.cpp` spells it, and its executable.
pub fn inspect(pid: u32) -> Option<Record> {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        let start = proc_stat(pid)?.get(19)?.clone();
        let exe = std::fs::read_link(format!("/proc/{pid}/exe")).ok()?;
        let exe = exe
            .to_string_lossy()
            .trim_end_matches(" (deleted)")
            .to_string();
        Some(Record {
            pid,
            start: Some(start),
            exe: Some(exe),
        })
    }
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        let p = pid.to_string();
        let start = lstart_seconds(&output("/bin/ps", &["-o", "lstart=", "-p", &p])?)?;
        let exe = output("/bin/ps", &["-o", "comm=", "-p", &p])?
            .trim()
            .to_string();
        Some(Record {
            pid,
            start: Some(start),
            exe: Some(exe),
        })
    }
    #[cfg(windows)]
    {
        let ps = system32("WindowsPowerShell\\v1.0\\powershell.exe");
        let script = format!(
            "$p = Get-Process -Id {pid} -ErrorAction Stop; $p.StartTime.ToFileTimeUtc(); $p.Path"
        );
        let text = output(&ps, &["-NoProfile", "-NonInteractive", "-Command", &script])?;
        let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
        let start = lines.next()?.to_string();
        let exe = lines.next()?.to_string();
        Some(Record {
            pid,
            start: Some(start),
            exe: Some(exe),
        })
    }
    #[cfg(not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        windows
    )))]
    {
        let _ = pid;
        None
    }
}

/// Whether any process bears `pid`, whoever owns it.
pub fn alive(pid: u32) -> bool {
    #[cfg(windows)]
    {
        let filter = format!("PID eq {pid}");
        output(
            &system32("tasklist.exe"),
            &["/FI", &filter, "/NH", "/FO", "CSV"],
        )
        .is_some_and(|out| out.contains(&format!("\"{pid}\"")))
    }
    // procfs, not `ps`: busybox's (Alpine) has no `-p`, and a zombie has died already.
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        proc_stat(pid)
            .is_some_and(|fields| !matches!(fields.first().map(String::as_str), Some("Z" | "X")))
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "android")))]
    {
        // `ps -p` answers for another user's process too, where `kill -0` would be refused.
        output("/bin/ps", &["-o", "pid=", "-p", &pid.to_string()])
            .is_some_and(|o| !o.trim().is_empty())
    }
}

/// The fields of `/proc/<pid>/stat` after the command's name: the state first, then ppid, pgrp, and on.
#[cfg(any(target_os = "linux", target_os = "android"))]
fn proc_stat(pid: u32) -> Option<Vec<String>> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    Some(
        stat[stat.rfind(')')? + 1..]
            .split_whitespace()
            .map(str::to_string)
            .collect(),
    )
}

/// The process group `pid` belongs to.
#[cfg(not(windows))]
fn group_of(pid: u32) -> Option<String> {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        proc_stat(pid)?.get(2).cloned()
    }
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        output("/bin/ps", &["-o", "pgid=", "-p", &pid.to_string()]).map(|g| g.trim().to_string())
    }
}

/// A process and its descendants: `taskkill /T /F` on Windows, the process GROUP elsewhere when the pid leads it.
pub fn kill_tree(pid: u32) {
    let p = pid.to_string();
    #[cfg(windows)]
    {
        let _ = output(&system32("taskkill.exe"), &["/PID", &p, "/T", "/F"]);
    }
    #[cfg(not(windows))]
    {
        // A negative pid names whichever group bears that number, so only a pid that leads its group is negated.
        let leads = group_of(pid).is_some_and(|g| g == p);
        let target = if leads { format!("-{p}") } else { p };
        let _ = output("kill", &["-KILL", &target]);
    }
}

/// The engine the file names killed once proved to be it; the file stays while its process may live. Blocking.
pub fn backstop(path: &Path, log: &dyn Fn(&str)) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    let Some(recorded) = parse(&text) else {
        let _ = std::fs::remove_file(path);
        return;
    };
    let live = alive(recorded.pid);
    match check(&recorded, inspect(recorded.pid).as_ref(), live) {
        Check::Stale => {
            let _ = std::fs::remove_file(path);
        }
        Check::Unverified => log(&format!(
            "pid {} may not be the engine — left alone",
            recorded.pid
        )),
        Check::Same => {
            log(&format!(
                "engine {} outlived the backend — killed",
                recorded.pid
            ));
            kill_tree(recorded.pid);
            for _ in 0..20 {
                if !alive(recorded.pid) {
                    let _ = std::fs::remove_file(path);
                    return;
                }
                sleep(Duration::from_millis(100));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(pid: u32, start: Option<&str>, exe: Option<&str>) -> Record {
        Record {
            pid,
            start: start.map(String::from),
            exe: exe.map(String::from),
        }
    }

    #[test]
    fn the_supervisor_format_parses_by_its_first_line() {
        let r = parse("4242\nstart=123456\nexe=/opt/k7/k7-runtime\nport=8766\n").unwrap();
        assert_eq!(r, rec(4242, Some("123456"), Some("/opt/k7/k7-runtime")));
        assert_eq!(parse("4242\r\n").unwrap(), rec(4242, None, None));
        assert!(parse("start=1\n").is_none());
        assert!(parse("0\n").is_none());
        assert!(parse("").is_none());
    }

    #[test]
    fn only_the_same_engine_may_be_killed() {
        let file = rec(7, Some("100"), Some("k7-runtime"));
        let same = rec(7, Some("100"), Some("C:\\k7\\K7-RUNTIME.EXE"));
        assert_eq!(check(&file, Some(&same), true), Check::Same);
        assert_eq!(check(&file, Some(&same), false), Check::Stale);
        let later = rec(7, Some("200"), Some("k7-runtime"));
        assert_eq!(check(&file, Some(&later), true), Check::Stale);
        let vim = rec(7, Some("100"), Some("/usr/bin/vim"));
        assert_eq!(check(&file, Some(&vim), true), Check::Stale);
        assert_eq!(check(&file, None, true), Check::Unverified);
        assert_eq!(
            check(&rec(7, None, None), Some(&same), true),
            Check::Unverified
        );
    }

    #[test]
    fn lstart_reads_as_local_seconds() {
        let a = lstart_seconds("Sat Oct 31 00:12:05 2026").unwrap();
        let b = lstart_seconds("Sat  Oct  31  00:12:06  2026\n").unwrap();
        assert_eq!(b.parse::<i64>().unwrap() - a.parse::<i64>().unwrap(), 1);
        assert!(lstart_seconds("not a date").is_none());
    }

    #[test]
    fn this_process_is_alive_and_described_but_not_the_engine() {
        let me = std::process::id();
        assert!(alive(me));
        let live = inspect(me).expect("the OS describes this process");
        let file = Record {
            pid: me,
            start: live.start.clone(),
            exe: live.exe.clone(),
        };
        assert_eq!(check(&file, Some(&live), true), Check::Stale);
    }
}
