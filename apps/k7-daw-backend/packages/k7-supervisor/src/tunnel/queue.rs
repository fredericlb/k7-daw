//! queue.rs — tasks run one at a time, in order, on a task of their own: the Rust form of a promise chain. Closing
//! it drops what is queued and the one running, since the closer may be what that one waits on. Draining runs what
//! is queued first, within a bound (`apps/k7-daw-backend/docs/engine/tunnel.md`).

use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::FutureExt;
use tokio::sync::{mpsc, watch};

use super::flag::raised;

type Task = Pin<Box<dyn Future<Output = ()> + Send>>;
type Last = Arc<Mutex<Option<Box<dyn FnOnce() + Send>>>>;

enum Item {
    Run(Task),
    Last(Last),
}

#[derive(Default)]
struct Flags {
    closed: bool,
    draining: bool,
}

struct Shared {
    tx: mpsc::UnboundedSender<Item>,
    flags: Mutex<Flags>,
    closed: watch::Sender<bool>,
    done: watch::Sender<bool>,
}

#[derive(Clone)]
pub struct SerialQueue {
    shared: Arc<Shared>,
}

/// Runs `last` once, whichever of the queue and the watchdog gets there first.
fn finish(last: &Last) {
    let f = last.lock().unwrap_or_else(|e| e.into_inner()).take();
    if let Some(f) = f {
        let _ = std::panic::catch_unwind(AssertUnwindSafe(f));
    }
}

impl SerialQueue {
    /// Starts the queue's task; inside a tokio runtime.
    pub fn new() -> SerialQueue {
        let (tx, rx) = mpsc::unbounded_channel();
        let shared = Arc::new(Shared {
            tx,
            flags: Mutex::default(),
            closed: watch::channel(false).0,
            done: watch::channel(false).0,
        });
        tokio::spawn(run(rx, shared.clone()));
        SerialQueue { shared }
    }

    fn flags(&self) -> std::sync::MutexGuard<'_, Flags> {
        self.shared.flags.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Queue a task; dropped once the queue is closed or draining.
    pub fn push(&self, task: impl Future<Output = ()> + Send + 'static) {
        let f = self.flags();
        if f.closed || f.draining {
            return;
        }
        let _ = self.shared.tx.send(Item::Run(Box::pin(task)));
    }

    /// Drop what is queued and the task running; what comes after is refused.
    pub fn close(&self) {
        self.flags().closed = true;
        self.shared.closed.send_replace(true);
        self.shared.done.send_replace(true);
    }

    /// Refuse new tasks, run the queued ones, then `last`; past `bound`, drop the rest and run `last` from a watchdog.
    pub fn drain(&self, bound: Duration, last: impl FnOnce() + Send + 'static) {
        let last: Last = Arc::new(Mutex::new(Some(Box::new(last))));
        {
            let mut f = self.flags();
            if f.closed || f.draining {
                return;
            }
            f.draining = true;
        }
        let _ = self.shared.tx.send(Item::Last(last.clone()));
        let (queue, mut done) = (self.clone(), self.shared.done.subscribe());
        // Its own task, never the caller's: the caller may be the socket's reader, which must not wait.
        tokio::spawn(async move {
            if tokio::time::timeout(bound, raised(&mut done)).await.is_ok() {
                return;
            }
            queue.close();
            finish(&last);
        });
    }
}

impl Default for SerialQueue {
    fn default() -> Self {
        Self::new()
    }
}

async fn run(mut rx: mpsc::UnboundedReceiver<Item>, shared: Arc<Shared>) {
    let mut closed = shared.closed.subscribe();
    loop {
        let item = tokio::select! {
            biased;
            _ = raised(&mut closed) => return,
            item = rx.recv() => item,
        };
        match item {
            None => return,
            Some(Item::Last(last)) => {
                finish(&last);
                shared.done.send_replace(true);
                return;
            }
            Some(Item::Run(task)) => {
                tokio::select! {
                    biased;
                    _ = raised(&mut closed) => return,
                    // A task that panics costs itself, never the task the rest of the queue runs on.
                    ran = AssertUnwindSafe(task).catch_unwind() => {
                        if ran.is_err() {
                            crate::log::line("tunnel", "a queued task stopped unexpectedly");
                        }
                    }
                }
            }
        }
    }
}
