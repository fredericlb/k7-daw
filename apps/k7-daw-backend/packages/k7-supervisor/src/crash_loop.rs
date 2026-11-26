//! crash_loop.rs — when an engine that died unasked is started again, and in which mode. A death within
//! `QUICK` of its start is a QUICK crash; quick crashes in a row back off 0, 2, 4… up to 30 s, and from the
//! `SAFE_MODE_AFTER`-th the engine starts in SAFE MODE, loading no external plugin. A longer run starts the
//! count again. What a reader gets wrong: safe mode is never left on its own, only by a start the user asks
//! for or a new supervisor (`apps/k7-daw-engine/k7-supervisor/docs/crashLoop.md`). Pure arithmetic over the times handed in.

use std::time::{Duration, Instant};

pub const QUICK: Duration = Duration::from_secs(60);
pub const SAFE_MODE_AFTER: u32 = 3;
pub const BACKOFF_MAX_S: u64 = 30;
/// A message thread silent this long is wedged, past the engine's own 60 s exit.
pub const WEDGED_MS: f64 = 90_000.0;

/// Whether the `messageThreadMs` a whoami reports says the engine is up but frozen.
pub fn wedged(message_thread_ms: f64) -> bool {
    message_thread_ms >= WEDGED_MS
}

/// 2^(n-1) seconds, capped: the n-th step of a backoff.
fn step(n: u32) -> Duration {
    Duration::from_secs((1u64 << n.saturating_sub(1).min(5)).min(BACKOFF_MAX_S))
}

#[derive(Debug, Clone, Default)]
pub struct CrashLoop {
    started_at: Option<Instant>,
    until: Option<Instant>,
    quick: u32,
    failures: u32,
    up: bool,
    safe: bool,
}

impl CrashLoop {
    /// The engine is up, spawned or adopted.
    pub fn started(&mut self, now: Instant) {
        self.up = true;
        self.started_at = Some(now);
        self.failures = 0;
    }

    /// The engine that was up is gone, unasked; once per death.
    pub fn died(&mut self, now: Instant) {
        if !self.up {
            return;
        }
        self.up = false;
        let quick = self
            .started_at
            .is_some_and(|s| now.duration_since(s) < QUICK);
        self.quick = if quick { self.quick + 1 } else { 1 };
        if self.quick >= SAFE_MODE_AFTER {
            self.safe = true;
        }
        // The first crash is relaunched at once, as a crash after a long run deserves.
        self.until = Some(if self.quick <= 1 {
            now
        } else {
            now + step(self.quick)
        });
    }

    /// A start did not bring an engine up: backed off, but no crash, so never a way into safe mode.
    pub fn start_failed(&mut self, now: Instant) {
        self.up = false;
        self.failures += 1;
        self.until = Some(now + step(self.failures));
    }

    /// The user started or stopped the engine: normal mode, nothing counted.
    pub fn reset(&mut self) {
        *self = CrashLoop::default();
    }

    pub fn up(&self) -> bool {
        self.up
    }
    pub fn due(&self, now: Instant) -> bool {
        self.until.is_none_or(|u| now >= u)
    }
    pub fn safe_mode(&self) -> bool {
        self.safe
    }
    pub fn quick_crashes(&self) -> u32 {
        self.quick
    }
}
