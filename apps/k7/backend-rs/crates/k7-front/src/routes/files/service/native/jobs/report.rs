//! report.rs — the jobs' counts logged every 30 s while they move, the line `bedCounters.mjs` reads (2362).
//! What a reader gets wrong: a line is logged only when a count changed since the last one, so an idle front says nothing.

use std::sync::{Arc, Weak};
use std::time::Duration;

use tokio::runtime::Handle;
use tokio::task::JoinHandle;

use super::Jobs;

/// How often the counts are looked at.
pub const REPORT_EVERY: Duration = Duration::from_secs(30);

/// The running reporter; dropping it stops it, as the freezer's timer does.
#[must_use = "dropping the task stops the report"]
pub struct ReportTask(JoinHandle<()>);

impl Drop for ReportTask {
    fn drop(&mut self) {
        self.0.abort();
    }
}

impl Jobs {
    /// Logs `Counts::line` each `every` that its text changed; it ends with the jobs, which it holds only weakly.
    pub fn report(
        self: &Arc<Self>,
        account: &str,
        log: k7_hub::Log,
        every: Duration,
        rt: &Handle,
    ) -> ReportTask {
        let jobs: Weak<Jobs> = Arc::downgrade(self);
        let account = account.to_owned();
        let mut last = self.counts.line(&account);
        ReportTask(rt.spawn(async move {
            let mut tick = tokio::time::interval(every);
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            tick.tick().await;
            loop {
                tick.tick().await;
                let Some(jobs) = jobs.upgrade() else { return };
                let line = jobs.counts.line(&account);
                if line != last {
                    log(&line);
                    last = line;
                }
            }
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::sync::atomic::Ordering;

    #[tokio::test]
    async fn a_line_is_logged_only_for_a_window_in_which_a_count_moved() {
        let jobs = Jobs::new(None);
        let said: Arc<Mutex<Vec<String>>> = Arc::default();
        let sink = said.clone();
        let log: k7_hub::Log = Arc::new(move |l: &str| sink.lock().unwrap().push(l.to_owned()));
        let every = Duration::from_millis(60);
        let _task = jobs.report("a@b.c", log, every, &Handle::current());
        tokio::time::sleep(every * 4).await;
        assert!(said.lock().unwrap().is_empty());
        jobs.counts.previews.fetch_add(3, Ordering::Relaxed);
        tokio::time::sleep(every * 6).await;
        let lines = said.lock().unwrap().clone();
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(lines[0].contains("previews=3"), "{lines:?}");
    }
}
