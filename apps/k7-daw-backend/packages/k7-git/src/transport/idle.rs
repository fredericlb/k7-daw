//! idle.rs — the bound on a transfer that moves nothing: a request may take as long as it keeps making progress.
//! What a reader gets wrong: progress is a chunk of the body taken by the connection or the answer arriving; the
//! clock restarts at each, so a long upload on a slow link passes and a server that goes quiet ends the call.

use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// The moment of the last progress, in milliseconds since `base`.
#[derive(Clone)]
pub(crate) struct Meter {
    base: Instant,
    last: Arc<AtomicU64>,
}

impl Meter {
    pub fn new() -> Meter {
        Meter {
            base: Instant::now(),
            last: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn tick(&self) {
        self.last
            .store(self.base.elapsed().as_millis() as u64, Ordering::SeqCst);
    }

    fn quiet_for(&self) -> Duration {
        self.base
            .elapsed()
            .saturating_sub(Duration::from_millis(self.last.load(Ordering::SeqCst)))
    }

    /// The future's output, or none when it let `idle` pass with no progress.
    pub async fn within<F: Future>(&self, idle: Duration, fut: F) -> Option<F::Output> {
        tokio::pin!(fut);
        loop {
            let wait = idle
                .saturating_sub(self.quiet_for())
                .max(Duration::from_millis(1));
            tokio::select! {
                out = &mut fut => return Some(out),
                _ = tokio::time::sleep(wait) => {
                    if self.quiet_for() >= idle {
                        return None;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn work_that_keeps_ticking_outlives_the_bound_and_work_that_goes_quiet_does_not() {
        let m = Meter::new();
        let ticker = m.clone();
        let busy = async move {
            for _ in 0..8 {
                tokio::time::sleep(Duration::from_millis(100)).await;
                ticker.tick();
            }
            7
        };
        assert_eq!(
            m.within(Duration::from_millis(300), busy).await,
            Some(7),
            "800 ms of progress under a 300 ms bound"
        );
        let quiet = Meter::new();
        let r = quiet
            .within(
                Duration::from_millis(200),
                tokio::time::sleep(Duration::from_secs(5)),
            )
            .await;
        assert!(r.is_none());
    }
}
