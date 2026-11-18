//! window.rs — a root's mutation windows, moved synchronously by whoever opens one (2121, 2127).
//! What a reader gets wrong: `hold` waits out a write phase in progress, so it may block its caller for milliseconds.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

#[derive(Default)]
pub struct Window {
    count: AtomicU32,
    epoch: AtomicU64,
    closed: AtomicBool,
    applying: Mutex<()>,
    labels: Mutex<Vec<String>>,
    longest_write_us: AtomicU64,
    /// How long each `hold` waited on `applying`, in log2 buckets of microseconds, and the longest.
    hold_waits: [AtomicU64; 32],
    longest_hold_us: AtomicU64,
}

fn locked<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Window {
    /// Opens a window; returns once no entry is mid-write, so the verb's first write is its own.
    pub fn hold(&self) {
        self.count.fetch_add(1, Ordering::SeqCst);
        self.epoch.fetch_add(1, Ordering::SeqCst);
        let began = std::time::Instant::now();
        drop(locked(&self.applying));
        let us = began.elapsed().as_micros() as u64;
        self.longest_hold_us.fetch_max(us, Ordering::SeqCst);
        let bucket = (64 - us.leading_zeros() as usize).min(31);
        self.hold_waits[bucket].fetch_add(1, Ordering::SeqCst);
    }

    /// Closes a window, clamped at zero; its label names the next batch.
    pub(crate) fn release(&self, label: &str) {
        if !label.is_empty() {
            locked(&self.labels).push(label.to_string());
        }
        // Set before the count moves, so a flush that sees the count at zero sees this too.
        self.closed.store(true, Ordering::SeqCst);
        loop {
            let c = self.count.load(Ordering::SeqCst);
            let next = c.saturating_sub(1);
            if self
                .count
                .compare_exchange(c, next, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                break;
            }
        }
        self.epoch.fetch_add(1, Ordering::SeqCst);
    }

    /// Tests only: `Handle::release` is the API, since it also wakes the worker.
    #[doc(hidden)]
    pub fn release_for_test(&self, label: &str) {
        self.release(label);
    }

    /// The longest write phase so far, in microseconds: what `hold` has had to wait for at most.
    #[doc(hidden)]
    pub fn longest_write_us(&self) -> u64 {
        self.longest_write_us.load(Ordering::SeqCst)
    }

    /// The longest a `hold` waited for a write phase, and the bucket (a power of two, in microseconds) under which 99% waited.
    #[doc(hidden)]
    pub fn hold_wait_us(&self) -> (u64, u64) {
        let counts: Vec<u64> = self
            .hold_waits
            .iter()
            .map(|c| c.load(Ordering::SeqCst))
            .collect();
        let total: u64 = counts.iter().sum();
        let (mut seen, mut p99) = (0, 0);
        for (i, c) in counts.iter().enumerate() {
            seen += c;
            if seen * 100 >= total * 99 {
                p99 = if i == 0 { 0 } else { 1u64 << i };
                break;
            }
        }
        (self.longest_hold_us.load(Ordering::SeqCst), p99)
    }

    /// How many windows were held.
    #[doc(hidden)]
    pub fn holds(&self) -> u64 {
        self.hold_waits
            .iter()
            .map(|c| c.load(Ordering::SeqCst))
            .sum()
    }

    pub(super) fn wrote_for(&self, took: std::time::Duration) {
        self.longest_write_us
            .fetch_max(took.as_micros() as u64, Ordering::SeqCst);
    }

    pub fn count(&self) -> u32 {
        self.count.load(Ordering::SeqCst)
    }

    pub fn epoch(&self) -> u64 {
        self.epoch.load(Ordering::SeqCst)
    }

    /// Whether a window closed since the last call: the next scan sweeps whole.
    pub(super) fn take_closed(&self) -> bool {
        self.closed.swap(false, Ordering::SeqCst)
    }

    /// Held for an entry's write phase: a `hold` waits for it.
    pub(super) fn applying(&self) -> MutexGuard<'_, ()> {
        locked(&self.applying)
    }

    pub(super) fn take_labels(&self) -> Vec<String> {
        std::mem::take(&mut *locked(&self.labels))
    }
}
