//! plugin_state.rs — what the plugin verbs share across sockets: the one scan's progress and lock, and the slots of
//! the plugin probes.
//! What a reader gets wrong: a probe slot is taken or refused, never waited for, so a client that loops cannot pile
//! up threads.

use super::scan_board::Board;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// Plugin probes loading at once; past it a probe is refused (k7-runtime's docs/plugins.md).
pub const MAX_PROBES: usize = 4;

#[derive(Default)]
pub struct PluginState {
    pub board: Board,
    /// One scan at a time for the whole engine: a second waits its turn here.
    pub scan_lock: Mutex<()>,
    probes: Arc<AtomicUsize>,
}

/// A probe's place among the four; returned when it drops, however the load ended.
pub struct Slot(Arc<AtomicUsize>);

impl Drop for Slot {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

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
}
