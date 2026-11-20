//! The engine link's machines, sans I/O (2201): events and a clock in, effects out, nothing dialled, slept or read.
//! What a reader gets wrong: an Effect only SAYS what to do; the front runs it and answers with an Event.
//! The model and its rows: apps/k7/backend-rs/docs/file-engine.md.

pub mod docs;
pub mod load;
pub mod reconcile;
pub mod scan;
pub mod sync;
pub mod wire;

use std::time::Instant;

use serde_json::Value;

use crate::paths::{climbs_out, relative};

/// The bench's workspace, which plays in a session of its own beside the song.
pub const BENCH_WORKSPACE: &str = "Banc";
/// The reconnect backoff's floor and the browser's ceiling, which the native side shares.
pub const RECONNECT_MIN_MS: u64 = 1_000;
pub const RECONNECT_MAX_MS: u64 = 30_000;
/// The ceiling HERE, deliberately shorter: the engine is on this machine or behind its tunnel.
pub const LOCAL_RECONNECT_MAX_MS: u64 = 4_000;
/// Frames held while a socket is down, and how long one stays worth sending.
pub const OUTBOX_MAX: usize = 200;
pub const OUTBOX_TTL_MS: u64 = 30_000;
/// A link nothing has used for this long, and serving nothing, is dropped.
pub const LINK_IDLE_MS: u64 = 5 * 60_000;

/// Where a session's load stands on its way to playable (`EngineLoadPhase`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Copying,
    Scanning,
    Applying,
    Warming,
}

impl Phase {
    pub fn as_str(self) -> &'static str {
        match self {
            Phase::Copying => "copying",
            Phase::Scanning => "scanning",
            Phase::Applying => "applying",
            Phase::Warming => "warming",
        }
    }
}

/// Where the engine's plugin scan stands: a `plugin-scan` frame, read defensively.
#[derive(Debug, Clone, PartialEq)]
pub struct ScanStatus {
    pub scanning: bool,
    pub done: u64,
    pub total: u64,
    pub current: String,
    pub version: i64,
}

/// What the front tells a machine. A link is one number per (session, binding), kept across the reconnects of its
/// socket and new after a re-dial or a change of binding: the TypeScript's identity is the client, not the dial.
#[derive(Debug, Clone)]
pub enum Event {
    /// A document went out through `link`.
    Sent {
        link: u64,
    },
    /// The engine's answer to "does it hold my session": `held` is `None` when it did not answer in time. `run` names
    /// the load's run that asked, `None` for the reconcile, so one answer reaches only the ask it belongs to.
    Loaded {
        run: Option<u64>,
        held: Option<bool>,
    },
    Scan(ScanStatus),
    /// The `plugin-scan` question went out; nothing waits on it but the clock.
    ScanAsked,
    Closed {
        link: u64,
    },
    /// The engine's `error` frame, which the scan wait reads as "unknown type plugin-scan".
    EngineError(String),
    /// A send of the dry run's warm step failed: nothing was played, so the master is put back.
    SendFailed(String),
    Plugins(Value),
    Tick(Instant),
}

/// What a machine asks the front to do, in order.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    Send(Value),
    /// Transport stop, then rewind: the dry run's one stop.
    Stop,
    /// Send the session's latest document again, the master at its own level.
    Restore,
    /// `list-sessions`, answered as `Event::Loaded` carrying the same `run`.
    AskLoaded {
        run: Option<u64>,
    },
    AskScan,
    AskCatalogue,
    WriteCatalogue(Value),
    /// Compile and send the served project again.
    Resend,
    /// Drop the session's socket and dial a fresh one, whose open re-sends what the session serves.
    Redial,
    Log(String),
}

/// The `sessionFor` of the wire: one session per (service, person), the person slugged as the file service slugs a directory.
pub fn session_for(service: &str, who: Option<&str>) -> String {
    match who {
        None | Some("") => service.to_string(),
        Some(w) => {
            // JS replaces per UTF-16 unit, so an astral character is two underscores.
            let slug: String = w
                .chars()
                .flat_map(|c| {
                    let n = if c.is_ascii_alphanumeric() {
                        0
                    } else {
                        c.len_utf16()
                    };
                    if n == 0 { vec![c] } else { vec!['_'; n] }
                })
                .collect();
            format!("{service}:{slug}")
        }
    }
}

/// `sessionOf`: `k7`, `k7:<slug>` or `banc:<slug>`; a root outside the files root, or no owner, is the bare `k7`.
pub fn session_of(root: Option<&str>, files_root: &str, email: &str) -> String {
    let (Some(root), false) = (root, email.is_empty()) else {
        return "k7".to_string();
    };
    let rel = relative(files_root, root);
    if rel.is_empty() || climbs_out(&rel) {
        return "k7".to_string();
    }
    let first = rel.split(['/', '\\']).next().unwrap_or("");
    let service = if first == BENCH_WORKSPACE {
        "banc"
    } else {
        "k7"
    };
    session_for(service, Some(email))
}

/// `servedProject`'s answer over a session's workspace and project: `{workspace, path}`, `.` for a root project.
pub fn served_project(files_root: &str, ws_root: &str, project: &str) -> Option<(String, String)> {
    let under = relative(files_root, ws_root);
    if under.is_empty() || climbs_out(&under) {
        return None;
    }
    let workspace = under.split(['/', '\\']).next().unwrap_or("").to_string();
    let rel = relative(ws_root, project).replace('\\', "/");
    Some((workspace, if rel.is_empty() { ".".into() } else { rel }))
}

/// The wait before retry number `attempt`, half-jittered: `random` in [0, 1) picks a point in the upper half.
pub fn jittered_backoff_ms(attempt: u32, max: u64, random: f64) -> f64 {
    let step = (RECONNECT_MIN_MS as f64 * 2f64.powi(attempt.min(60) as i32)).min(max as f64);
    step / 2.0 + random * (step / 2.0)
}
