//! Shared by the projects' tests: a disk that counts what the plans ask of it, and the account the tables run on.

#![allow(dead_code)]

use std::sync::atomic::{AtomicUsize, Ordering};

use k7_file_service::confine::{Disk, Probe as GateProbe};
use k7_file_service::entries::Probe;

use super::ws::Acct;

/// The real disk, counting every `exists`, `is_dir` and gate probe a plan asks.
#[derive(Default)]
pub struct Counting {
    pub exists: AtomicUsize,
    pub dirs: AtomicUsize,
    pub gate: AtomicUsize,
    pub spells: AtomicUsize,
    /// The most dirents a plan may read for spellings; 0 is `Limits::default()`'s.
    pub cap: AtomicUsize,
    /// The dirents the spelling lookups read, as the plan adds them to its bound.
    pub read: AtomicUsize,
}

impl Counting {
    pub fn total(&self) -> usize {
        self.exists.load(Ordering::Relaxed)
            + self.dirs.load(Ordering::Relaxed)
            + self.gate.load(Ordering::Relaxed)
            + self.spells.load(Ordering::Relaxed)
    }
}

impl GateProbe for Counting {
    fn present(&self, p: &str) -> bool {
        self.gate.fetch_add(1, Ordering::Relaxed);
        Disk.present(p)
    }

    fn real(&self, p: &str) -> Option<String> {
        Disk.real(p)
    }
}

impl Probe for Counting {
    fn exists(&self, p: &str) -> bool {
        self.exists.fetch_add(1, Ordering::Relaxed);
        Disk.exists(p)
    }

    fn is_dir(&self, p: &str) -> bool {
        self.dirs.fetch_add(1, Ordering::Relaxed);
        Disk.is_dir(p)
    }

    fn spelled(
        &self,
        dir: &str,
        name: &str,
        seen: &mut u64,
    ) -> Result<Option<String>, k7_file_service::space::Declined> {
        self.spells.fetch_add(1, Ordering::Relaxed);
        let before = *seen;
        let cap = self.cap.load(Ordering::Relaxed) as u64;
        let limits = k7_file_service::tree::Limits {
            max_seen: if cap == 0 { u64::MAX } else { cap },
            ..Default::default()
        };
        let got = k7_file_service::workspaces::names::spelled_in(dir, name, &limits, seen);
        self.read
            .fetch_add((*seen - before) as usize, Ordering::Relaxed);
        got
    }
}

/// The workspace `songs` of a scratch account, with `a.k7` in it.
pub fn songs(name: &str) -> (Acct, String) {
    let a = Acct::new(name);
    let root = a.ws("songs");
    a.project("songs", "a.k7", "project({ name: \"A\" })\n");
    (a, root)
}
