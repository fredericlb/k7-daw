//! clock.rs — "now" for the registry, the tickets and the relay gate, in epoch milliseconds as `Date.now()`.
//! The one thing a reader would get wrong: only the testbed stops it (`test-hooks`), as vitest's fake `Date` does.

use std::sync::Arc;
use std::sync::atomic::AtomicI64;
#[cfg(any(test, feature = "test-hooks"))]
use std::sync::atomic::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};

/// No frozen time: the system's.
const RUNNING: i64 = i64::MIN;

/// Read only where it can be stopped: every other build answers the system's time.
#[derive(Clone)]
#[cfg_attr(not(any(test, feature = "test-hooks")), allow(dead_code))]
pub struct Clock(Arc<AtomicI64>);

impl Default for Clock {
    fn default() -> Self {
        Clock(Arc::new(AtomicI64::new(RUNNING)))
    }
}

/// The system's epoch milliseconds.
pub fn system_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

impl Clock {
    pub fn now_ms(&self) -> i64 {
        #[cfg(any(test, feature = "test-hooks"))]
        {
            let frozen = self.0.load(Ordering::Acquire);
            if frozen != RUNNING {
                return frozen;
            }
        }
        system_ms()
    }

    /// Stops "now" at `ms`, as a test's `vi.setSystemTime` under fake timers does.
    #[cfg(any(test, feature = "test-hooks"))]
    pub fn set_now(&self, ms: i64) {
        self.0.store(ms, Ordering::Release);
    }
}
