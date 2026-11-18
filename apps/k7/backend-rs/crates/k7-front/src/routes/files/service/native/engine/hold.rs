//! Frames held behind a document still being decided (2216, D2): one ordered stream per session, so a transport asked after a
//! document never leaves before it. A document the copy or scan gate has parked does not hold frames, as the TypeScript's does not.
//! What a reader gets wrong: the count is per session, and the frames are pushed in the order asked right after the last decision.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde_json::Value;
use tokio::sync::mpsc;

use super::held;
use super::queue::Session;

/// Documents being decided, the frames held for them, and the queue `push_doc` orders documents through.
#[derive(Default)]
pub(crate) struct Hold {
    deciding: u32,
    frames: Vec<Value>,
    jobs: Option<mpsc::UnboundedSender<Job>>,
    /// Jobs sent and not finished: while any is, a frame is a job too, so the order asked is exact (D5).
    pending: u32,
}

/// A piece of work the bridge asks of a session, run in the order asked: a document, a start, a reload, a frame.
type Job = Box<dyn FnOnce(Arc<Session>) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send>;

/// A document in the course of being decided; the frames held behind it go when the last one is dropped.
pub(crate) struct Deciding(Arc<Session>);

impl Drop for Deciding {
    fn drop(&mut self) {
        let mut st = held(&self.0.st);
        st.hold.deciding = st.hold.deciding.saturating_sub(1);
        if st.hold.deciding == 0 {
            for f in std::mem::take(&mut st.hold.frames) {
                self.0.push(&mut st, f);
            }
        }
    }
}

impl Session {
    /// Count a document as being decided, from now until the guard is dropped.
    pub(crate) fn deciding(self: &Arc<Self>) -> Deciding {
        held(&self.st).hold.deciding += 1;
        Deciding(self.clone())
    }

    /// A frame asked while a document is being decided waits for it; true when it was held.
    pub(crate) fn hold_frame(st: &mut super::queue::State, frame: &Value) -> bool {
        if st.hold.deciding == 0 {
            return false;
        }
        st.hold.frames.push(frame.clone());
        true
    }

    /// Whether jobs are pending, so a frame must queue behind them.
    pub(crate) fn jobs_pending(st: &super::queue::State) -> bool {
        st.hold.pending > 0
    }

    /// A document of the bridge's (`engine.push`, `engine.send`): see `enqueue`.
    pub fn push_doc(self: &Arc<Self>, doc: Value) {
        self.enqueue(move |s| async move { s.send_doc(doc).await });
    }

    /// A document a push compiled: dropped at its turn when a browser's document is held by then, as the TypeScript's check and send were one step.
    pub fn push_compiled(self: &Arc<Self>, doc: Value) {
        self.enqueue(move |s| async move {
            if held(&s.st).client_doc.is_some() {
                return;
            }
            s.send_doc(doc).await;
        });
    }

    /// A frame queued behind the pending jobs (D5).
    pub(crate) fn enqueue_frame(self: &Arc<Self>, frame: Value) {
        self.enqueue(move |s| async move {
            let mut st = held(&s.st);
            s.push(&mut st, frame);
        });
    }

    /// Work run one after another in the order asked (2216); a job that panics is counted and the queue goes on (B1).
    pub fn enqueue<F, Fut>(self: &Arc<Self>, work: F)
    where
        F: FnOnce(Arc<Session>) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let job: Job = Box::new(move |s| Box::pin(work(s)));
        // The send is an unbounded channel push, never blocking, so it is made under the lock that gives the order.
        let mut st = held(&self.st);
        st.hold.pending += 1;
        let mut job = Some(job);
        for _ in 0..2 {
            if st.hold.jobs.is_none() {
                st.hold.jobs = self.start_consumer();
            }
            let Some(tx) = &st.hold.jobs else { break };
            match tx.send(job.take().expect("a job to send")) {
                Ok(()) => return,
                Err(e) => {
                    job = Some(e.0);
                    st.hold.jobs = None;
                }
            }
        }
        st.hold.pending = st.hold.pending.saturating_sub(1);
    }

    fn start_consumer(self: &Arc<Self>) -> Option<mpsc::UnboundedSender<Job>> {
        use futures_util::FutureExt;
        let inner = self.inner()?;
        let (tx, mut rx) = mpsc::unbounded_channel::<Job>();
        let me = Arc::downgrade(self);
        let stats = inner.clone();
        inner.rt.spawn(async move {
            while let Some(job) = rx.recv().await {
                let Some(s) = me.upgrade() else { return };
                let s2 = s.clone();
                let ran = std::panic::AssertUnwindSafe(async move { job(s2).await })
                    .catch_unwind()
                    .await;
                if ran.is_err() {
                    super::stats::Stats::bump(&stats.stats.panics);
                }
                let mut st = held(&s.st);
                st.hold.pending = st.hold.pending.saturating_sub(1);
            }
        });
        Some(tx)
    }
}
