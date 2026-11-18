//! workspaces_memo.rs — the home page's listing kept for 5 seconds per account, as text (1670, 1674).
//! What a reader gets wrong: an entry is rebuilt when older, when an act of this piece landed, or when the root's
//! mtime differs from the one read before the walk — it is compared for inequality, never for "newer".

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// `LISTING_RESCAN_MS` of the TypeScript.
pub const WINDOW: Duration = Duration::from_secs(5);
/// Accounts remembered before the table starts over.
const CAP: usize = 1024;

/// One walk's two answers, both as the text that is sent.
#[derive(Clone)]
pub struct Entry {
    pub at: Instant,
    pub root_mtime: f64,
    pub projects: Arc<str>,
    pub surfaced: Arc<str>,
    /// What the account's acts had counted when the walk began.
    pub acts: u64,
}

#[derive(Default)]
struct Slot {
    entry: Option<Entry>,
    acts: u64,
}

fn table() -> &'static Mutex<HashMap<String, Slot>> {
    static TABLE: OnceLock<Mutex<HashMap<String, Slot>>> = OnceLock::new();
    TABLE.get_or_init(Mutex::default)
}

fn slots<T>(f: impl FnOnce(&mut HashMap<String, Slot>) -> T) -> T {
    let mut t = table().lock().unwrap_or_else(|p| p.into_inner());
    if t.len() >= CAP {
        t.clear();
    }
    f(&mut t)
}

/// The account's entry when it still holds: within the window, no act since, the root's mtime as it was.
pub fn fresh(account: &str, root_mtime: f64) -> Option<Entry> {
    slots(|t| {
        let slot = t.get(account)?;
        let e = slot.entry.as_ref()?;
        (e.at.elapsed() < WINDOW && e.acts == slot.acts && e.root_mtime == root_mtime)
            .then(|| e.clone())
    })
}

/// What the account's acts count now, read before a walk so that an act during it outdates the entry.
pub fn acts(account: &str) -> u64 {
    slots(|t| t.entry(account.to_string()).or_default().acts)
}

/// A walk's answers kept for the account.
pub fn keep(account: &str, entry: Entry) {
    slots(|t| t.entry(account.to_string()).or_default().entry = Some(entry));
}

/// An act of this piece landed (a make, a removal, a copy): the entry no longer holds.
pub fn dropped(account: &str) {
    slots(|t| {
        let slot = t.entry(account.to_string()).or_default();
        slot.acts += 1;
        slot.entry = None;
    });
}

/// The account is gone: its entry goes with it.
pub fn forget(account: &str) {
    slots(|t| {
        t.remove(account);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(acts: u64, mtime: f64) -> Entry {
        Entry {
            at: Instant::now(),
            root_mtime: mtime,
            projects: "p".into(),
            surfaced: "s".into(),
            acts,
        }
    }

    #[test]
    fn an_entry_holds_until_an_act_a_new_mtime_or_the_window() {
        let a = "memo-test-a";
        assert!(fresh(a, 1.0).is_none());
        keep(a, entry(acts(a), 1.0));
        assert!(fresh(a, 1.0).is_some());
        // Inequality, in both directions: a clock that went back is a change too.
        assert!(fresh(a, 2.0).is_none() && fresh(a, 0.5).is_none());
        dropped(a);
        assert!(fresh(a, 1.0).is_none());
        // A walk that began before an act cannot be kept as current.
        let before = acts(a);
        dropped(a);
        keep(a, entry(before, 1.0));
        assert!(fresh(a, 1.0).is_none());
        let mut old = entry(acts(a), 1.0);
        old.at = Instant::now() - WINDOW - Duration::from_millis(1);
        keep(a, old);
        assert!(fresh(a, 1.0).is_none());
        forget(a);
        assert!(acts(a) == 0);
    }
}
