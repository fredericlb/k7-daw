//! registry.rs — each account's `Shares`, by its state folder, kept while a picture render runs and dropped at its reap (2540, 2555).
//! What a reader gets wrong: the account may be reaped and made again meanwhile; the `Shares` is what a second POST finds.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use super::Shares;

#[cfg(test)]
thread_local! {
    /// The `Shares` this thread made: a test of `off` holds it at zero (2511).
    pub static BUILT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn all() -> &'static Mutex<HashMap<String, Arc<Shares>>> {
    static ALL: OnceLock<Mutex<HashMap<String, Arc<Shares>>>> = OnceLock::new();
    ALL.get_or_init(Mutex::default)
}

fn table() -> std::sync::MutexGuard<'static, HashMap<String, Arc<Shares>>> {
    all().lock().unwrap_or_else(PoisonError::into_inner)
}

/// The account's key: its state folder, which the hook and the asks both know.
pub fn key(env: &std::collections::BTreeMap<String, String>) -> String {
    k7_file_service::libraries::Home::of(env, false).studio
}

/// The account's shares, made on its first need; the front's one count report is started with the first.
pub fn of(key: &str, log: &k7_hub::Log) -> Arc<Shares> {
    let mut t = table();
    if let Some(s) = t.get(key) {
        return s.clone();
    }
    #[cfg(test)]
    BUILT.with(|b| b.set(b.get() + 1));
    let shares = Shares::counting_into(counts());
    reporting(log);
    t.insert(key.to_owned(), shares.clone());
    shares
}

/// Starts the front's count reporter if none runs: one line per change, whatever the accounts.
fn reporting(log: &k7_hub::Log) {
    static SLOT: Mutex<Option<super::report::ReportTask>> = Mutex::new(None);
    super::report::ensure(&SLOT, &counts(), log, super::report::REPORT_EVERY);
}

/// The counts of the whole front: the log line the bed's judge reads is one, whatever the accounts.
pub fn counts() -> Arc<super::Counts> {
    static COUNTS: OnceLock<Arc<super::Counts>> = OnceLock::new();
    COUNTS.get_or_init(Arc::default).clone()
}

/// Whether the account has work in flight, which keeps it from being reaped.
pub fn busy(key: &str) -> bool {
    table().get(key).is_some_and(|s| s.busy())
}

/// The account was let go: its memory is dropped unless a picture render still runs (2540).
pub fn reaped(key: &str) {
    let mut t = table();
    if t.get(key).is_some_and(|s| s.busy()) {
        return;
    }
    t.remove(key);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    fn quiet() -> k7_hub::Log {
        Arc::new(|_: &str| {})
    }

    #[test]
    fn the_counts_are_the_fronts_whatever_the_accounts_and_a_reap() {
        // Other tests add to the same counts, so each figure is a floor.
        let exports = || counts().exports.load(Ordering::Relaxed);
        let (a, b) = ("counts-test-a", "counts-test-b");
        let before = exports();
        of(a, &quiet())
            .counts
            .exports
            .fetch_add(1, Ordering::Relaxed);
        of(b, &quiet())
            .counts
            .exports
            .fetch_add(1, Ordering::Relaxed);
        assert!(exports() >= before + 2);
        assert!(counts().line().contains("exports="));
        reaped(a);
        of(a, &quiet())
            .counts
            .exports
            .fetch_add(1, Ordering::Relaxed);
        assert!(exports() >= before + 3);
        reaped(a);
        reaped(b);
    }

    #[test]
    fn a_picture_render_in_flight_keeps_the_account_too() {
        let key = "registry-busy-test";
        let shares = of(key, &quiet());
        let slot = shares.begin_picture().unwrap();
        assert!(busy(key));
        drop(slot);
        assert!(!busy(key));
        reaped(key);
    }
}
