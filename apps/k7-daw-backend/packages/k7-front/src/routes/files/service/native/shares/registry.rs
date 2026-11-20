//! registry.rs — each account's `Shares`, by its state folder, kept for the life of the process (2540, 2555).
//! What a reader gets wrong: the `Shares` outlives the account's runtime; it is what a second POST finds.

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    fn quiet() -> k7_hub::Log {
        Arc::new(|_: &str| {})
    }

    #[test]
    fn the_counts_are_the_fronts_whatever_the_accounts() {
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
    }
}
