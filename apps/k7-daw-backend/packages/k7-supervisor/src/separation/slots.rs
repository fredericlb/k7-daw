//! slots.rs — at most N heavy jobs at once on this machine; the others wait their turn, in the order they asked.
//! A slot is held by a guard, so a job that fails still gives it back. Why cuts are counted is `docs/separation.md`.

use std::sync::{Condvar, Mutex, MutexGuard};

#[derive(Default)]
struct State {
    running: usize,
    next_ticket: usize,
    serving: usize,
}

pub struct JobSlots {
    slots: usize,
    state: Mutex<State>,
    turn: Condvar,
}

/// One slot, from `hold` until it is dropped.
pub struct Held<'a>(&'a JobSlots);

impl JobSlots {
    /// A zero cap still runs one.
    pub fn new(slots: usize) -> JobSlots {
        JobSlots {
            slots: slots.max(1),
            state: Mutex::default(),
            turn: Condvar::new(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Wait for a slot, after every earlier caller; `waiting(ahead)` hears how many still wait in front, each time that changes.
    pub fn hold(&self, mut waiting: impl FnMut(usize)) -> Held<'_> {
        let mut st = self.lock();
        let ticket = st.next_ticket;
        st.next_ticket += 1;
        let mut said = usize::MAX;
        while ticket != st.serving || st.running >= self.slots {
            let ahead = ticket - st.serving;
            if ahead != said {
                waiting(ahead);
            }
            said = ahead;
            st = self.turn.wait(st).unwrap_or_else(|e| e.into_inner());
        }
        st.serving += 1;
        st.running += 1;
        self.turn.notify_all();
        Held(self)
    }

    pub fn running(&self) -> usize {
        self.lock().running
    }
}

impl Drop for Held<'_> {
    fn drop(&mut self) {
        self.0.lock().running -= 1;
        self.0.turn.notify_all();
    }
}
