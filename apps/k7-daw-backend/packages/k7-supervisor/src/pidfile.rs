//! pidfile.rs — `<state>/engine.pid` as an IDENTITY rather than a number: the pid, the process's start time,
//! its executable and the port it serves, so a pid the OS has since handed to another program is never
//! adopted or killed. A bare pid still reads, as an unverified record: adopted on its port, never killed.
//! The OS half and the verdict are `k7_local::pidfile`'s, which the host's backstop kill reads by too.

use std::path::Path;

pub use k7_local::pidfile::{Check, alive, check, inspect, is_engine_exe, kill_tree};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Record {
    pub pid: u32,
    /// The OS's own start figure (macOS epoch seconds, Linux field 22 of `/proc/<pid>/stat`), when known.
    pub start: Option<String>,
    pub exe: Option<String>,
    /// The port the engine was started on; 0 in a file from before it was recorded.
    pub port: u16,
}

impl Record {
    pub fn verified(&self) -> bool {
        self.pid > 0 && self.start.is_some()
    }
    /// The record as `k7_local::pidfile` judges it.
    pub fn local(&self) -> k7_local::pidfile::Record {
        k7_local::pidfile::Record {
            pid: self.pid,
            start: self.start.clone(),
            exe: self.exe.clone(),
        }
    }
}

/// Either format: a bare pid, or the pid then `start=`, `exe=` and `port=` lines; pid 0 for anything else.
pub fn parse(text: &str) -> Record {
    let Some(r) = k7_local::pidfile::parse(text) else {
        return Record::default();
    };
    let port = text
        .lines()
        .filter_map(|l| l.trim().strip_prefix("port="))
        .filter(|p| !p.is_empty() && p.len() <= 5 && p.bytes().all(|b| b.is_ascii_digit()))
        .filter_map(|p| p.parse::<u16>().ok())
        .find(|&p| p > 0)
        .unwrap_or(0);
    Record {
        pid: r.pid,
        start: r.start,
        exe: r.exe,
        port,
    }
}

/// The pid on the first line, so a reader that only takes the leading number still gets it.
pub fn format(r: &Record) -> String {
    let mut out = format!("{}\n", r.pid);
    if let Some(s) = &r.start {
        out += &format!("start={s}\n");
    }
    if let Some(e) = &r.exe {
        out += &format!("exe={e}\n");
    }
    if r.port > 0 {
        out += &format!("port={}\n", r.port);
    }
    out
}

/// The file parsed; a missing one is pid 0.
pub fn read(path: &Path) -> Record {
    std::fs::read_to_string(path)
        .map(|t| parse(&t))
        .unwrap_or_default()
}

pub fn write(path: &Path, r: &Record) {
    let _ = std::fs::write(path, format(r));
}

/// The live identity of `pid` when the OS gives it, else a bare-pid record.
pub fn record_of(pid: u32) -> Record {
    match inspect(pid) {
        Some(l) => Record {
            pid,
            start: l.start,
            exe: l.exe,
            port: 0,
        },
        None => Record {
            pid,
            ..Record::default()
        },
    }
}

/// The verdict on `r` now, as the OS describes its pid.
pub fn judge(r: &Record) -> Check {
    check(&r.local(), inspect(r.pid).as_ref(), alive(r.pid))
}
