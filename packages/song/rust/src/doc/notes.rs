// doc/notes.rs — the findings of one pass, a repeat of one file's message kept once, split into those that stop the compile and those that do not.
// A reader would get wrong that a finding stops by its place in a list: `stop` is the list a `Failure` carries, `note` the one that never does.

use crate::files::{Finding, Severity};
use std::collections::HashSet;

/// Findings gathered by a pass; dedupe is by file, severity and message.
#[derive(Debug, Default)]
pub struct Notes {
    pub stop: Vec<Finding>,
    pub note: Vec<Finding>,
    seen: HashSet<(String, bool, String)>,
}

pub(crate) fn finding(file: &str, severity: Severity, message: impl Into<String>) -> Finding {
    Finding {
        file: file.to_owned(),
        line: 1,
        col: 1,
        message: message.into(),
        severity,
    }
}

impl Notes {
    fn fresh(&mut self, f: &Finding) -> bool {
        self.seen
            .insert((f.file.clone(), f.severity == Severity::Error, f.message.clone()))
    }

    /// A finding that does not stop the compile.
    pub fn push(&mut self, f: Finding) {
        if self.fresh(&f) {
            self.note.push(f);
        }
    }

    /// A finding that stops it.
    pub fn stops(&mut self, f: Finding) {
        if self.fresh(&f) {
            self.stop.push(f);
        }
    }

    /// An ERROR at 1:1 that stops the compile.
    pub fn stop_at(&mut self, file: &str, message: impl Into<String>) {
        self.stops(finding(file, Severity::Error, message));
    }
}
