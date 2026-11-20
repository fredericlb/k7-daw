//! handle.rs — what other threads hold of one captured workspace: the window, what the worker publishes, its mailbox.
//! What a reader gets wrong: an entry's compute runs again when a window opened before its write, so it must be repeatable.
//! A write phase never opens its own window (the lock is not reentrant) and no closure ever holds a `Handle` (a cycle).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use tokio::sync::{mpsc, oneshot};

use super::window::Window;
use crate::capture::LiveCapture;

/// What the worker publishes for any thread to read: no query waits on the worker (2123).
#[derive(Default)]
pub(super) struct Published {
    /// The files waiting for bytes, as the last step left them.
    pub pending: std::sync::Mutex<Arc<Vec<String>>>,
    pub generation: AtomicU64,
    /// How many entries the worker has taken in, a window or not: what a test waits on to know one reached it.
    pub entries: AtomicU64,
    pub watching: AtomicBool,
    pub stopped: AtomicBool,
}

impl Published {
    /// Publishes the capture's pending files when they differ from what is published.
    pub(super) fn publish_pending(&self, cap: &LiveCapture) {
        let mut now = cap.pending_paths();
        let mut held = self.pending.lock().unwrap_or_else(|e| e.into_inner());
        now.sort();
        if **held != now {
            *held = Arc::new(now);
        }
    }
}

pub(super) type Task = Box<dyn FnOnce(&mut LiveCapture) + Send + 'static>;

/// How one try of an entry ended.
#[derive(PartialEq, Eq)]
pub(super) enum Tried {
    Done,
    /// A window is open: the entry waits for its release.
    Window,
    /// The disk or the epoch moved since the compute: compute again at once.
    Again,
    /// A closed window's files are not swept yet: the entry waits for the next committed full sweep, without trying.
    Scan,
}

pub(super) type Entry = Box<dyn FnMut(&mut LiveCapture, &Window) -> Tried + Send + 'static>;

pub(super) enum Msg {
    Paths(Vec<String>),
    Release,
    Run(Task),
    Entry(Entry),
    Flush(oneshot::Sender<()>),
    Capture(Vec<String>, oneshot::Sender<()>),
    Stop(oneshot::Sender<()>),
    Abandon(oneshot::Sender<()>),
}

#[derive(Clone)]
pub struct Handle {
    pub(super) root: String,
    pub(super) window: Arc<Window>,
    pub(super) published: Arc<Published>,
    pub(super) tx: mpsc::UnboundedSender<Msg>,
}

impl Handle {
    pub fn root(&self) -> &str {
        &self.root
    }

    pub fn window(&self) -> &Arc<Window> {
        &self.window
    }

    pub fn hold(&self) {
        self.window.hold();
    }

    /// Closes a window and wakes the worker, which sweeps once the last one is closed.
    pub fn release(&self, label: &str) {
        self.window.release(label);
        let _ = self.tx.send(Msg::Release);
    }

    pub fn generation(&self) -> u64 {
        self.published.generation.load(Ordering::SeqCst)
    }

    pub fn watching(&self) -> bool {
        self.published.watching.load(Ordering::SeqCst) && !self.stopped()
    }

    pub fn stopped(&self) -> bool {
        self.published.stopped.load(Ordering::SeqCst)
    }

    /// Tests only: how many entries the worker has taken in so far.
    #[doc(hidden)]
    pub fn entries_taken(&self) -> u64 {
        self.published.entries.load(Ordering::SeqCst)
    }

    /// The files waiting for bytes, read without waiting on the worker (2123).
    pub fn pending(&self) -> Arc<Vec<String>> {
        let p = self.published.pending.lock();
        p.unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// `f` over the capture at once, between the worker's steps; None when the worker has stopped.
    pub async fn run<R: Send + 'static>(
        &self,
        f: impl FnOnce(&mut LiveCapture) -> R + Send + 'static,
    ) -> Option<R> {
        let (tx, rx) = oneshot::channel();
        let task: Task = Box::new(move |c| {
            let _ = tx.send(f(c));
        });
        self.tx.send(Msg::Run(task)).ok()?;
        rx.await.ok()
    }

    /// An entry applied once no window is open, in order: `compute` only reads, `write` is the locked phase (2127).
    pub async fn run_when_free<T, R: Send + 'static>(
        &self,
        compute: impl Fn(&LiveCapture) -> T + Send + 'static,
        write: impl FnMut(&mut LiveCapture, T) -> Option<R> + Send + 'static,
    ) -> Option<R> {
        self.entry(false, compute, write).await
    }

    /// `run_when_free`, but also not while a closed window's files wait for their sweep (the compute is not even tried).
    pub async fn run_when_swept<T, R: Send + 'static>(
        &self,
        compute: impl Fn(&LiveCapture) -> T + Send + 'static,
        write: impl FnMut(&mut LiveCapture, T) -> Option<R> + Send + 'static,
    ) -> Option<R> {
        self.entry(true, compute, write).await
    }

    async fn entry<T, R: Send + 'static>(
        &self,
        swept: bool,
        compute: impl Fn(&LiveCapture) -> T + Send + 'static,
        write: impl FnMut(&mut LiveCapture, T) -> Option<R> + Send + 'static,
    ) -> Option<R> {
        let (tx, rx) = oneshot::channel();
        let (mut write, mut tx) = (write, Some(tx));
        let entry: Entry = Box::new(move |cap, win| {
            let epoch = win.epoch();
            if win.count() != 0 {
                return Tried::Window;
            }
            if swept && cap.window_closed() {
                return Tried::Scan;
            }
            let computed = compute(cap);
            let _applying = win.applying();
            if win.count() != 0 {
                return Tried::Window;
            }
            if win.epoch() != epoch {
                return Tried::Again;
            }
            let began = std::time::Instant::now();
            let wrote = write(cap, computed);
            win.wrote_for(began.elapsed());
            match wrote {
                Some(r) => {
                    if let Some(tx) = tx.take() {
                        let _ = tx.send(r);
                    }
                    Tried::Done
                }
                None => Tried::Again,
            }
        });
        self.tx.send(Msg::Entry(entry)).ok()?;
        rx.await.ok()
    }

    /// One full sweep through the worker's windowed scan, answered once it ran (or was skipped by an open window).
    pub async fn flush(&self) -> Option<()> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(Msg::Flush(tx)).ok()?;
        rx.await.ok()
    }

    /// An ordinary scan of `files` (a whole sweep for many), answered once its batch is with the owner; an open window skips it.
    pub async fn capture(&self, files: Vec<String>) -> Option<()> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(Msg::Capture(files, tx)).ok()?;
        rx.await.ok()
    }

    /// Tests only: the worker ends as a killed process would, its index not saved; answers once it has.
    #[doc(hidden)]
    pub async fn abandon(&self) {
        let (tx, rx) = oneshot::channel();
        if self.tx.send(Msg::Abandon(tx)).is_ok() {
            let _ = rx.await;
        }
    }

    /// Stops the capture (its index saved), the timers and the watch; answers once it has.
    pub async fn stop(&self) {
        let (tx, rx) = oneshot::channel();
        if self.tx.send(Msg::Stop(tx)).is_ok() {
            let _ = rx.await;
        }
    }
}
