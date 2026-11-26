//! pending.rs — which read-only queries already wait on the message thread, so an identical one is not queued behind
//! them: their answer is broadcast, so the waiting one answers the asker too (`docs/supersede.md`).
//! What a reader gets wrong: a key leaves as its job STARTS, so a query asked while one runs is queued again; a
//! ticket whose job never ran (the host went first) frees its key when dropped.

use std::collections::HashSet;
use std::sync::{Arc, Mutex, MutexGuard};

#[derive(Default)]
pub struct PendingQueries(Arc<Mutex<HashSet<String>>>);

/// The right to post one query's job; the job calls `started` first.
pub struct Ticket {
    waiting: Arc<Mutex<HashSet<String>>>,
    key: String,
    started: bool,
}

fn set(m: &Mutex<HashSet<String>>) -> MutexGuard<'_, HashSet<String>> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl PendingQueries {
    /// A ticket when no identical query waits, and it now does; `None` means an identical one will answer.
    pub fn admit(&self, key: &str) -> Option<Ticket> {
        set(&self.0).insert(key.to_string()).then(|| Ticket {
            waiting: self.0.clone(),
            key: key.to_string(),
            started: false,
        })
    }

    pub fn size(&self) -> usize {
        set(&self.0).len()
    }
}

impl Ticket {
    /// The job began: the next identical query is queued, since what it would read may have changed.
    pub fn started(mut self) {
        set(&self.waiting).remove(&self.key);
        self.started = true;
    }
}

impl Drop for Ticket {
    fn drop(&mut self) {
        if !self.started {
            set(&self.waiting).remove(&self.key);
        }
    }
}
