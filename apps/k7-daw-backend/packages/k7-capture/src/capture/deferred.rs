//! deferred.rs — the paths a scan could not read this round, left for the next scan rather than failed (`liveDeferred.ts`, 2132).
//! What a reader gets wrong: gone (`ENOENT`, `ENOTDIR`) is silent; every other code is said once per path until it reads again.

use std::collections::HashSet;
use std::io;

use k7_file_service::filter::code_of;

pub struct Deferred {
    root: String,
    held: HashSet<String>,
    notes: Vec<String>,
}

impl Deferred {
    pub fn new(root: &str) -> Self {
        Deferred {
            root: root.to_string(),
            held: HashSet::new(),
            notes: vec![],
        }
    }

    /// `rel` could not be read now: said once, until a later scan reads it.
    pub fn skip(&mut self, rel: &str, e: &io::Error) {
        let code = code_of(e);
        if matches!(code, "ENOENT" | "ENOTDIR") || !self.held.insert(rel.to_string()) {
            return;
        }
        let name = if rel.is_empty() { "." } else { rel };
        self.notes.push(format!(
            "   … live capture: {}: {name} not read ({code}), read again at the next scan",
            self.root
        ));
    }

    /// Whether a scan failed to read `rel` and has not read it since.
    pub fn holds(&self, rel: &str) -> bool {
        self.held.contains(rel)
    }

    /// A line for the worker's log, as is.
    pub fn say(&mut self, line: String) {
        self.notes.push(line);
    }

    /// `rel` was read at last, or is gone: a later failure is news again.
    pub fn clear(&mut self, rel: &str) {
        self.held.remove(rel);
    }

    /// What was said since the last call, for the worker to log.
    pub fn take_notes(&mut self) -> Vec<String> {
        std::mem::take(&mut self.notes)
    }
}
