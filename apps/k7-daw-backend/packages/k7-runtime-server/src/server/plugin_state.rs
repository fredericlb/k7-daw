//! plugin_state.rs — what the plugin verbs share across sockets: the one scan's progress and lock, the slots of the
//! plugin probes, and the tickets that let a newer value of a parameter skip an older one still queued.
//! What a reader gets wrong: a probe slot is taken or refused, never waited for, so a client that loops cannot pile
//! up threads; a ticket is superseded only by a newer write of the SAME parameter of the same plugin and session.

use super::scan_board::Board;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

/// Plugin probes loading at once; past it a probe is refused (k7-runtime's docs/plugins.md).
pub const MAX_PROBES: usize = 4;

#[derive(Default)]
pub struct PluginState {
    pub board: Board,
    /// One scan at a time for the whole engine: a second waits its turn here.
    pub scan_lock: Mutex<()>,
    probes: Arc<AtomicUsize>,
    tickets: Mutex<HashMap<String, Ticket>>,
}

/// A probe's place among the four; returned when it drops, however the load ended.
pub struct Slot(Arc<AtomicUsize>);

impl Drop for Slot {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// True once a newer write has made the job pointless; read on the message thread when the job starts.
pub type Ticket = Arc<AtomicBool>;

impl PluginState {
    /// A slot for one probe, or `None` and nothing taken when every slot is in use.
    pub fn take_probe(&self) -> Option<Slot> {
        if self.probes.fetch_add(1, Ordering::SeqCst) < MAX_PROBES {
            return Some(Slot(self.probes.clone()));
        }
        self.probes.fetch_sub(1, Ordering::SeqCst);
        None
    }

    pub fn probes_in_use(&self) -> usize {
        self.probes.load(Ordering::SeqCst)
    }

    fn tickets(&self) -> MutexGuard<'_, HashMap<String, Ticket>> {
        self.tickets.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// A live parameter write: it supersedes an earlier write to the same plugin's same parameter, still queued.
    /// The ticket is taken and the job posted under one lock, or two sockets could queue in another order than seen.
    pub fn post_param(&self, session: &str, plugin: &str, name: &str, post: impl FnOnce(Ticket)) {
        let mut tickets = self.tickets();
        let ticket: Ticket = Arc::default();
        let key = format!("{session}\n{plugin}\n{name}");
        if let Some(older) = tickets.insert(key, ticket.clone()) {
            older.store(true, Ordering::Release);
        }
        post(ticket);
    }

    /// The job began: its slot is cleared unless a newer write has taken it, so the ledger never outgrows the params.
    pub fn param_started(&self, session: &str, plugin: &str, name: &str, ticket: &Ticket) {
        let mut tickets = self.tickets();
        let key = format!("{session}\n{plugin}\n{name}");
        if tickets.get(&key).is_some_and(|t| Arc::ptr_eq(t, ticket)) {
            tickets.remove(&key);
        }
    }

    pub fn tickets_held(&self) -> usize {
        self.tickets().len()
    }
}

pub fn superseded(ticket: &Ticket) -> bool {
    ticket.load(Ordering::Acquire)
}
