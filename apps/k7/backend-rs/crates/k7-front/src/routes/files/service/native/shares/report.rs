//! report.rs — the shares' counts logged every 30 s while they move, the line `bedCounters.mjs` reads (2573).
//! What a reader gets wrong: a line is logged only when a count changed since the last one, so an idle front says nothing.

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::runtime::Handle;
use tokio::task::JoinHandle;

use super::Counts;

/// How often the counts are looked at.
pub const REPORT_EVERY: Duration = Duration::from_secs(30);

/// The running reporter; dropping it stops it.
#[must_use = "dropping the task stops the report"]
pub struct ReportTask(JoinHandle<()>);

impl Drop for ReportTask {
    fn drop(&mut self) {
        self.0.abort();
    }
}

impl Counts {
    /// The line the front logs while the exports move (2573); the token and every path stay out of it.
    pub fn line(&self) -> String {
        let g = |c: &std::sync::atomic::AtomicU64| c.load(Ordering::Relaxed);
        format!(
            "[files] native shares: exports={} pictures={} declines={} panics={}",
            g(&self.exports),
            g(&self.pictures),
            g(&self.declines),
            g(&self.panics)
        )
    }
}

impl Counts {
    /// Logs `line` each `every` that its text changed; it holds the counts, so it outlives every account's reap.
    pub fn report(self: &Arc<Self>, log: k7_hub::Log, every: Duration, rt: &Handle) -> ReportTask {
        let counts = self.clone();
        let mut last = counts.line();
        ReportTask(rt.spawn(async move {
            let mut tick = tokio::time::interval(every);
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            tick.tick().await;
            loop {
                tick.tick().await;
                let line = counts.line();
                if line != last {
                    log(&line);
                    last = line;
                }
            }
        }))
    }
}

/// The front's one reporter in `slot`, started on the first call inside a runtime and again if its runtime has ended.
pub(super) fn ensure(
    slot: &Mutex<Option<ReportTask>>,
    counts: &Arc<Counts>,
    log: &k7_hub::Log,
    every: Duration,
) {
    let mut held = slot.lock().unwrap_or_else(|p| p.into_inner());
    if held.as_ref().is_some_and(|t| !t.0.is_finished()) {
        return;
    }
    if let Ok(rt) = Handle::try_current() {
        *held = Some(counts.report(log.clone(), every, &rt));
    }
}

#[cfg(test)]
mod tests {
    use super::super::Shares;
    use super::*;

    fn logged() -> (Arc<Mutex<Vec<String>>>, k7_hub::Log) {
        let said: Arc<Mutex<Vec<String>>> = Arc::default();
        let sink = said.clone();
        (
            said,
            Arc::new(move |l: &str| sink.lock().unwrap().push(l.to_owned())),
        )
    }

    #[tokio::test]
    async fn a_line_is_logged_only_for_a_window_in_which_a_count_moved() {
        let counts = Arc::new(Counts::default());
        let (said, log) = logged();
        let every = Duration::from_millis(60);
        let _task = counts.report(log, every, &Handle::current());
        tokio::time::sleep(every * 4).await;
        assert!(said.lock().unwrap().is_empty());
        counts.exports.fetch_add(3, Ordering::Relaxed);
        tokio::time::sleep(every * 6).await;
        let lines = said.lock().unwrap().clone();
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(lines[0].contains("exports=3"), "{lines:?}");
    }

    #[tokio::test]
    async fn many_accounts_make_one_line_per_change_and_a_change_before_the_last_reap_is_logged() {
        let counts = Arc::new(Counts::default());
        let (said, log) = logged();
        let slot = Mutex::new(None);
        let every = Duration::from_millis(80);
        let accounts: Vec<_> = (0..4)
            .map(|_| {
                ensure(&slot, &counts, &log, every);
                Shares::counting_into(counts.clone())
            })
            .collect();
        accounts[0].counts.exports.fetch_add(1, Ordering::Relaxed);
        tokio::time::sleep(every * 4).await;
        accounts[3].counts.pictures.fetch_add(1, Ordering::Relaxed);
        drop(accounts);
        tokio::time::sleep(every * 4).await;
        let lines = said.lock().unwrap().clone();
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert!(lines[0].contains("exports=1"), "{lines:?}");
        assert!(lines[1].contains("pictures=1"), "{lines:?}");
    }
}
