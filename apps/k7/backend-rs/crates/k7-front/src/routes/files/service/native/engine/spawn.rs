//! The two ways the link starts work of its own: a task whose panic is counted, and a blocking call whose panic is counted.
//! What a reader gets wrong: a panic is a count on `Stats`, never a failure of the task that asked.

use std::sync::Arc;

use super::Inner;
use super::stats::Stats;

impl Inner {
    pub(crate) fn spawn<F: std::future::Future<Output = ()> + Send + 'static>(
        self: &Arc<Self>,
        f: F,
    ) {
        use futures_util::FutureExt;
        let me = self.clone();
        self.rt.spawn(async move {
            if std::panic::AssertUnwindSafe(f)
                .catch_unwind()
                .await
                .is_err()
            {
                Stats::bump(&me.stats.panics);
            }
        });
    }

    pub(crate) fn spawn_blocking<F: FnOnce() + Send + 'static>(self: &Arc<Self>, f: F) {
        let me = self.clone();
        self.rt.spawn(async move {
            if tokio::task::spawn_blocking(f).await.is_err() {
                Stats::bump(&me.stats.panics);
            }
        });
    }
}
