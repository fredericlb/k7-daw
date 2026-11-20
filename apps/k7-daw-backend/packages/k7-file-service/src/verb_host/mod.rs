//! verb_host — the frame under the host members' routes (3f): the job table, the path gates and the error every route speaks.
//! What a reader gets wrong: the job table never reads a clock, its callers hand `now` in; see docs/verb-host.md.

pub mod docpaths;
pub mod files;
pub mod gate;
pub mod jobs;

use serde_json::{Value, json};

/// A refusal: the status, a sentence naming no folder of the server, and the code a Worker maps to an error class.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub status: u16,
    pub error: String,
    pub code: Option<&'static str>,
}

impl Error {
    pub fn new(status: u16, error: impl Into<String>) -> Error {
        Error {
            status,
            error: error.into(),
            code: None,
        }
    }

    /// A 503 or other status with its code: `off`, `busy`, `no-engine`, `no-machine`, `no-ffmpeg`, `gone`.
    pub fn coded(status: u16, error: impl Into<String>, code: &'static str) -> Error {
        Error {
            status,
            error: error.into(),
            code: Some(code),
        }
    }

    pub fn body(&self) -> Value {
        match self.code {
            Some(c) => json!({ "error": self.error, "code": c }),
            None => json!({ "error": self.error }),
        }
    }
}
