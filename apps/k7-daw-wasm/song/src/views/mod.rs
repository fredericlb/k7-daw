// views/mod.rs — what the frontend READS of a project: surfaces, field vocabulary, clip lengths, the module lists.
// A reader would get wrong that nothing here compiles: Part A names types of 2e (Inputs) and none of 2h (docs/rustCompilerViews.md).

pub mod answer;
pub mod catalogue;
pub mod dead;
pub mod errors;
pub mod fields;
pub mod hints;
pub mod io_ports;
pub mod lengths;
pub mod nullable;
pub mod places;
pub mod surfaces;
pub mod walk;
pub mod wav;
pub mod web;

use crate::files::{Finding, Severity};
use serde::Serialize;

/// Units of 64 bytes of output, or one step of a walk, a view may spend (decision 635).
pub const BUDGET: usize = 1_000_000;

/// The diagnostics an answer lists before it says how many it left out (decision 640).
pub const MAX_DIAGNOSTICS: usize = 1_000;

/// The characters of a name a message quotes.
pub const NAME_CUT: usize = 200;

/// What one view may spend: a name copied weighs `1 + bytes / 64`, a step of a walk 1.
#[derive(Clone, Debug)]
pub struct Spend {
    spent: usize,
    limit: usize,
    capped: bool,
}

impl Default for Spend {
    fn default() -> Self {
        Spend::new()
    }
}

impl Spend {
    pub fn new() -> Spend {
        Spend::with_limit(BUDGET)
    }

    pub fn with_limit(limit: usize) -> Spend {
        Spend {
            spent: 0,
            limit,
            capped: false,
        }
    }

    fn charge(&mut self, n: usize) -> bool {
        if self.capped {
            return false;
        }
        self.spent = self.spent.saturating_add(n);
        self.capped = self.spent > self.limit;
        !self.capped
    }

    /// One step of a walk; false (for good) once the limit is passed.
    pub fn step(&mut self) -> bool {
        self.charge(1)
    }

    /// `n` steps of a walk at once, charged before the work they stand for.
    pub fn steps(&mut self, n: usize) -> bool {
        self.charge(n)
    }

    /// A copy of `n` bytes; false (for good) once the limit is passed.
    pub fn bytes(&mut self, n: usize) -> bool {
        self.charge(1 + n / 64)
    }

    pub fn capped(&self) -> bool {
        self.capped
    }

    pub fn spent(&self) -> usize {
        self.spent
    }
}

/// A finding as the frontend's `Diagnostic` reads it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Diagnostic {
    pub file: String,
    pub line: u32,
    pub col: u32,
    pub message: String,
    pub severity: Severity,
}

impl From<&Finding> for Diagnostic {
    fn from(f: &Finding) -> Diagnostic {
        Diagnostic {
            file: f.file.clone(),
            line: f.line,
            col: f.col,
            message: f.message.clone(),
            severity: f.severity,
        }
    }
}

/// A limit's WARNING, named by the file the view stopped in.
pub(crate) fn capped_finding(file: &str, what: &str) -> Finding {
    Finding {
        file: file.to_owned(),
        line: 1,
        col: 1,
        message: format!("{what} stopped at its limit — the rest is left out"),
        severity: Severity::Warning,
    }
}
