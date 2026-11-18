//! report.rs — the runtime's counts and the pieces' (`stats.rs`) logged every 30 s while they move, the lines `bedCounters.mjs` reads (2680).
//! What a reader gets wrong: a quiet front says nothing; a line is logged only when a count moved.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use super::super::stats;

/// How often the counts are looked at.
pub const REPORT_EVERY: Duration = Duration::from_secs(30);

/// The front's own counts, for the whole process.
#[derive(Default)]
pub struct Totals {
    pub logs: AtomicU64,
    pub background: AtomicU64,
}

pub fn totals() -> &'static Totals {
    static T: OnceLock<Totals> = OnceLock::new();
    T.get_or_init(Totals::default)
}

/// A `/logs` answered, counted in the report's `logs`.
pub fn logs_answered() {
    totals().logs.fetch_add(1, Ordering::Relaxed);
}

/// `[files] native runtime: background= logs=`.
pub fn line() -> String {
    let t = totals();
    let n = |a: &AtomicU64| a.load(Ordering::Relaxed);
    format!(
        "[files] native runtime: background={} logs={}",
        n(&t.background),
        n(&t.logs),
    )
}

/// The process' one reporter, started with the first account that has a runtime.
pub fn start_once(log: &k7_hub::Log, every: Duration) {
    static STARTED: OnceLock<()> = OnceLock::new();
    if STARTED.set(()).is_err() {
        return;
    }
    let Ok(rt) = tokio::runtime::Handle::try_current() else {
        return;
    };
    let log: Arc<_> = log.clone();
    rt.spawn(async move {
        let mut last = [line(), stats::global().line()];
        let mut tick = tokio::time::interval(every);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        tick.tick().await;
        loop {
            tick.tick().await;
            let now = [line(), stats::global().line()];
            for (was, is) in last.iter().zip(&now) {
                if was != is {
                    log(is);
                }
            }
            last = now;
        }
    });
}
