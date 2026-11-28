//! scan_board.rs — where the plugin scan stands, as every socket may ask it: files done of files listed, the one
//! being loaded, and how many scans this process has finished (k7-runtime's `ScanProgress.h`, ported).
//! What a reader gets wrong: `total` is 0 while the plugin folders are still being listed, which is a scan, not
//! an empty one; and a load starting is said at once, behind no rate limit.

use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// At most this often is a stat-only step said.
pub const SAY_EVERY: Duration = Duration::from_millis(250);

/// One answer to "is this engine scanning, and how far along": what `plugin-scan` carries.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(rename = "PluginScanStatus"))]
pub struct Status {
    pub scanning: bool,
    pub done: i32,
    pub total: i32,
    /// The file being LOADED, "" while files are only stat'ed.
    pub current: String,
    /// Scans this process has finished: a client that saw N is told N+1 is the catalogue to read again.
    pub version: i32,
}

struct Inner {
    status: Status,
    last: Option<Instant>,
    said: String,
}

pub struct Board {
    inner: Mutex<Inner>,
    interval: Duration,
}

impl Default for Board {
    fn default() -> Self {
        Board::new(SAY_EVERY)
    }
}

impl Board {
    pub fn new(interval: Duration) -> Board {
        Board {
            inner: Mutex::new(Inner {
                status: Status::default(),
                last: None,
                said: String::new(),
            }),
            interval,
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// A scan starts: nothing listed yet. Always worth saying.
    pub fn begin(&self, now: Instant) -> Status {
        let mut i = self.lock();
        i.status.scanning = true;
        i.status.done = 0;
        i.status.total = 0;
        i.status.current.clear();
        i.last = Some(now);
        i.status.clone()
    }

    /// One file further: the status to say now, or `None` when it is not worth a frame yet.
    pub fn step(&self, done: i32, total: i32, loading: &str, now: Instant) -> Option<Status> {
        let mut i = self.lock();
        i.status.done = done;
        i.status.total = total;
        i.status.current = loading.to_string();
        let out = i.status.clone();
        // A load can take twenty seconds, so its name goes out at once rather than behind the throttle.
        let since = i
            .last
            .map_or(self.interval, |l| now.saturating_duration_since(l));
        let due = (!loading.is_empty() && loading != i.said) || since >= self.interval;
        if due {
            i.last = Some(now);
            i.said = loading.to_string();
        }
        due.then_some(out)
    }

    /// The scan ended, whether it found anything or not: the catalogue is the one to read now.
    pub fn finish(&self) -> Status {
        let mut i = self.lock();
        i.status.scanning = false;
        i.status.current.clear();
        i.status.done = i.status.total;
        i.status.version += 1;
        i.said.clear();
        i.status.clone()
    }

    pub fn snapshot(&self) -> Status {
        self.lock().status.clone()
    }
}
