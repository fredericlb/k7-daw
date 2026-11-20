//! The engine's `--decode` under a watchdog (2416): its environment is the account's alone, a child that writes nothing
//! is killed at the deadline or the stop, and its stderr is kept to 64 KiB. The engine is a shell script here.

#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use k7_mediatek::analyse::AnalyseError;
use k7_mediatek::decode::{Engine, engine_in};

const HEADER: &str = r#"printf '{"sampleRate":8000,"channels":1,"frames":0}\n'"#;

fn script(name: &str, body: &str) -> (PathBuf, PathBuf) {
    let d = std::env::temp_dir().join(format!("k7-watchdog-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let bin = d.join("engine.sh");
    std::fs::write(&bin, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    (d, bin)
}

fn run(
    bin: &Path,
    env: &[(String, String)],
    deadline: Instant,
    stop: &dyn Fn() -> bool,
) -> Result<(), String> {
    let source = bin.with_file_name("a.wav");
    // A script just written can be busy while another thread forks.
    for _ in 0..20 {
        let r = engine_in(
            &Engine {
                bin,
                env: Some(env),
            },
            &source,
            deadline,
            stop,
            &mut |_, _| Ok(()),
        );
        match r {
            Err(AnalyseError::Failed(m)) if m.contains("Text file busy") => {
                std::thread::sleep(Duration::from_millis(50));
            }
            other => return other.map(drop).map_err(|e| e.message().to_string()),
        }
    }
    Err("busy".into())
}

fn alive(pid: &str) -> bool {
    std::process::Command::new("kill")
        .args(["-0", pid])
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn pid_of(file: &Path) -> String {
    for _ in 0..100 {
        if let Ok(t) = std::fs::read_to_string(file)
            && !t.trim().is_empty()
        {
            return t.trim().to_string();
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("the engine wrote no pid");
}

fn env(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn far() -> Instant {
    Instant::now() + Duration::from_secs(60)
}

#[test]
fn the_child_runs_under_the_accounts_environment_alone() {
    let (_, bin) = script("env", &format!("{HEADER}\n/usr/bin/env >&2\nexit 4"));
    let said = run(&bin, &env(&[("ACCOUNT_ONLY", "1")]), far(), &|| false).unwrap_err();
    assert!(said.contains("ACCOUNT_ONLY=1"), "{said}");
    // The front's own environment never reaches it.
    assert!(!said.contains("HOME="), "{said}");
    assert!(!said.contains("CARGO"), "{said}");
}

#[test]
fn a_decode_that_writes_nothing_and_never_exits_is_killed_at_the_deadline() {
    let (d, bin) = script("silent", "echo $$ > \"$PIDFILE\"\nexec /bin/sleep 20");
    let pidfile = d.join("pid");
    let e = env(&[("PIDFILE", pidfile.to_str().unwrap())]);
    let t = Instant::now();
    let said = run(&bin, &e, Instant::now() + Duration::from_secs(1), &|| false).unwrap_err();
    assert!(said.contains("took longer than"), "{said}");
    assert!(t.elapsed() < Duration::from_secs(10));
    assert!(!alive(&pid_of(&pidfile)), "the child is gone");
}

#[test]
fn a_stop_kills_a_silent_child_without_waiting_for_it() {
    let (d, bin) = script("stopped", "echo $$ > \"$PIDFILE\"\nexec /bin/sleep 20");
    let pidfile = d.join("pid");
    let e = env(&[("PIDFILE", pidfile.to_str().unwrap())]);
    // Asked once the child is running, so its pid was written.
    let stop = || pidfile.exists();
    let said = run(&bin, &e, far(), &stop).unwrap_err();
    assert!(said.contains("was aborted"), "{said}");
    assert!(!alive(&pid_of(&pidfile)));
}

#[test]
fn a_child_that_floods_stderr_is_read_to_the_end_and_keeps_64_kib() {
    let (_, bin) = script(
        "flood",
        &format!("{HEADER}\n/usr/bin/head -c 4194304 /dev/zero | /usr/bin/tr '\\0' x >&2\nexit 2"),
    );
    let said = run(&bin, &[], far(), &|| false).unwrap_err();
    assert!(
        said.len() <= 64 * 1024 && said.len() > 60_000,
        "{}",
        said.len()
    );
    assert!(said.starts_with("xxxx"));
}

#[test]
fn a_child_that_closes_its_output_and_keeps_running_is_killed_at_the_deadline() {
    let (d, bin) = script(
        "closed",
        "echo $$ > \"$PIDFILE\"\nexec >&-\nexec /bin/sleep 20",
    );
    let pidfile = d.join("pid");
    let e = env(&[("PIDFILE", pidfile.to_str().unwrap())]);
    let t = Instant::now();
    let said = run(&bin, &e, Instant::now() + Duration::from_secs(1), &|| false).unwrap_err();
    assert!(said.contains("took longer than"), "{said}");
    assert!(t.elapsed() < Duration::from_secs(10), "{:?}", t.elapsed());
    assert!(!alive(&pid_of(&pidfile)), "the child is gone");
}

#[test]
fn a_stop_kills_a_child_that_closed_its_output() {
    let (d, bin) = script(
        "closed_stop",
        "echo $$ > \"$PIDFILE\"\nexec >&-\nexec /bin/sleep 20",
    );
    let pidfile = d.join("pid");
    let e = env(&[("PIDFILE", pidfile.to_str().unwrap())]);
    let said = run(&bin, &e, far(), &|| pidfile.exists()).unwrap_err();
    assert!(said.contains("was aborted"), "{said}");
    assert!(!alive(&pid_of(&pidfile)));
}

#[test]
fn a_grandchild_holding_stderr_open_does_not_hold_a_finished_decode() {
    let (_, bin) = script(
        "grace",
        &format!("(/bin/sleep 20 >/dev/null &)\n{HEADER}\nexit 0"),
    );
    let t = Instant::now();
    run(&bin, &[], far(), &|| false).expect("a decode that finished");
    assert!(t.elapsed() < Duration::from_secs(5), "{:?}", t.elapsed());
}
