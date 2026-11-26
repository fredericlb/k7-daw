//! supersede.rs — which queued jobs a newer message has made pointless before they ran, per session.
//! What a reader gets wrong: a job is never REORDERED, only skipped, and the ticket is taken and the job posted
//! under one lock, or two sockets could queue jobs in another order than the ledger saw.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

/// True once a newer message has made the job pointless; read on the message thread when the job starts.
pub type Ticket = Arc<AtomicBool>;

pub fn superseded(t: &Ticket) -> bool {
    t.load(Ordering::Acquire)
}

fn fresh() -> Ticket {
    Arc::new(AtomicBool::new(false))
}

fn mark(slot: &Ticket) {
    slot.store(true, Ordering::Release);
}

#[derive(Default)]
struct Pending {
    /// `project` and `tracks` jobs since the last barrier, by key.
    doc: HashMap<String, Ticket>,
    /// Live parameter writes since the last barrier, by plugin and parameter name.
    params: HashMap<String, Ticket>,
}

#[derive(Default)]
pub struct Supersede(Mutex<HashMap<String, Pending>>);

const PROJECT_KEY: &str = "project";

impl Supersede {
    fn sessions(&self) -> MutexGuard<'_, HashMap<String, Pending>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// A whole document: it supersedes every document job of the session queued since its last barrier.
    pub fn post_project<R>(&self, session: &str, post: impl FnOnce(Ticket) -> R) -> R {
        let mut all = self.sessions();
        let pending = all.entry(session.to_owned()).or_default();
        for t in pending.doc.values() {
            mark(t);
        }
        pending.doc.clear();
        let ticket = fresh();
        pending.doc.insert(PROJECT_KEY.into(), ticket.clone());
        post(ticket)
    }

    /// A `tracks` delta: it supersedes only an earlier delta naming exactly the same tracks.
    pub fn post_tracks<R>(
        &self,
        session: &str,
        mut ids: Vec<String>,
        post: impl FnOnce(Ticket) -> R,
    ) -> R {
        ids.sort();
        ids.dedup();
        let key = ids.iter().fold("tracks".to_owned(), |k, id| k + "\n" + id);
        let mut all = self.sessions();
        let slot = all
            .entry(session.to_owned())
            .or_default()
            .doc
            .entry(key)
            .or_insert_with(fresh);
        mark(slot);
        *slot = fresh();
        post(slot.clone())
    }

    /// A live parameter write: it supersedes an earlier write to the same plugin's same parameter.
    pub fn post_param<R>(
        &self,
        session: &str,
        plugin: &str,
        name: &str,
        post: impl FnOnce(Ticket) -> R,
    ) -> R {
        let mut all = self.sessions();
        let slot = all
            .entry(session.to_owned())
            .or_default()
            .params
            .entry(format!("{plugin}\n{name}"))
            .or_insert_with(fresh);
        mark(slot);
        *slot = fresh();
        post(slot.clone())
    }

    /// Any other message about the session: nothing queued before it may be skipped for what comes after.
    pub fn barrier(&self, session: &str) {
        self.sessions().remove(session);
    }

    /// How many sessions hold pending tickets: the tests check the ledger does not grow.
    pub fn session_count(&self) -> usize {
        self.sessions().len()
    }
}
