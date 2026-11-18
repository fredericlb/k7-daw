//! Shared by the workspaces' tests: an account's home with workspaces, projects, surfaces and markers on a real disk.

#![allow(dead_code)]

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use k7_file_service::libraries::Home;

use super::Scratch;

/// The folder every workspace of the scratch account lives in.
pub const WORKSPACES: &str = "studio/workspace";

pub struct Acct {
    pub s: Scratch,
}

impl Acct {
    pub fn new(name: &str) -> Acct {
        let s = Scratch::new(name);
        s.dir(WORKSPACES);
        s.dir("libraries/k7-core/modules");
        Acct { s }
    }

    pub fn home(&self) -> Home {
        self.s.home(false)
    }

    /// A workspace folder; the path of it.
    pub fn ws(&self, name: &str) -> String {
        self.s.dir(&format!("{WORKSPACES}/{name}"))
    }

    pub fn path(&self, ws: &str, rel: &str) -> String {
        self.s.path(&format!("{WORKSPACES}/{ws}/{rel}"))
    }

    /// A project folder holding a `project.k7x` with `body` as its text.
    pub fn project(&self, ws: &str, rel: &str, body: &str) -> String {
        self.s
            .file(&format!("{WORKSPACES}/{ws}/{rel}/project.k7x"), body);
        self.path(ws, rel)
    }

    pub fn surface(&self, ws: &str, project: &str, name: &str, text: &str) {
        self.s.file(
            &format!("{WORKSPACES}/{ws}/{project}/surfaces/{name}.k7x"),
            text,
        );
    }

    pub fn marker(&self, ws: &str, text: &str) {
        self.s
            .file(&format!("{WORKSPACES}/.active-projects/{ws}"), text);
    }
}

pub const TURNTABLES: &str = "type(\"turntables\")\nlayout(\"split\")\nplaylist(\"Fred\")\n";
pub const TIMELINE: &str = "type(\"timeline\")\nlayout(\"split\")\n";

/// `abs` given a modification time of `secs` seconds and `nanos` nanoseconds after the epoch.
pub fn touch(abs: &str, secs: u64, nanos: u32) {
    let f = std::fs::OpenOptions::new()
        .write(true)
        .open(abs)
        .or_else(|_| std::fs::File::open(abs))
        .unwrap();
    f.set_modified(UNIX_EPOCH + Duration::new(secs, nanos))
        .unwrap();
}

pub fn now() -> SystemTime {
    SystemTime::now()
}
