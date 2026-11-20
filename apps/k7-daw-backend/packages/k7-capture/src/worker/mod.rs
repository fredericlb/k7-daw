//! worker — one serial task per captured workspace: it owns the capture, runs each file step on the blocking pool with
//! the state moved in and handed back, and keeps the window's epoch (2120, 2121). What a reader gets wrong: a scan commits
//! only if the window's count is zero and its epoch unmoved since the scan began; else it is dropped and the release sweeps.

mod handle;
mod window;

use std::collections::VecDeque;
use std::io;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use k7_file_service::watch::Sink;
use k7_replica::blob::BlobId;
use serde_json::Value;
use tokio::sync::mpsc;
use tokio::time::sleep_until;

use crate::capture::{Baseline, DIRTY_SWEEP, LiveCapture, Options, QUIET_MS, RECONCILE_MS, Step};
use crate::feed::{Feed, GATHER};
use crate::source::Source;
pub use handle::Handle;
use handle::{Entry, Msg, Published, Tried};
pub use window::Window;

/// What a worker tells its owner; each is a tell, so no worker ever waits on the owner (2123, 2124).
pub trait Out: Send + Sync + 'static {
    fn batch(&self, root: &str, batch: Value);
    fn missing(&self, root: &str, blobs: Vec<BlobId>);
    fn note(&self, root: &str, line: &str);
    fn feed(&self, root: &str, paths: Vec<String>, generation: u64);
}

/// The timers' spacing; a test shrinks them, the production values are the TypeScript's (2130).
#[derive(Clone, Copy)]
pub struct Timing {
    pub quiet: Duration,
    pub feed: Duration,
    pub first_sweep: Duration,
    pub sweep: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Timing {
            quiet: Duration::from_millis(QUIET_MS),
            feed: GATHER,
            first_sweep: Duration::from_secs(1),
            sweep: Duration::from_millis(RECONCILE_MS),
        }
    }
}

/// Tries in a row an entry may make before the worker reads its mailbox.
const AGAIN_AT_MOST: u32 = 8;
const AGAIN_PAUSE: Duration = Duration::from_millis(20);

/// A call a test makes between a scan's walk and its commit, which is where a window can open.
pub type Hook = Arc<dyn Fn() + Send + Sync>;

pub struct Deps {
    pub source: Arc<dyn Source>,
    pub out: Arc<dyn Out>,
    pub timing: Timing,
    pub baseline: Option<Arc<dyn Baseline>>,
    pub hook: Option<Hook>,
}

/// The sink holds the mailbox weakly: a worker whose every `Handle` is dropped ends (2128).
fn sink_of(tx: mpsc::WeakUnboundedSender<Msg>) -> Sink {
    Arc::new(move |rels| {
        if let Some(tx) = tx.upgrade() {
            let _ = tx.send(Msg::Paths(rels));
        }
    })
}

/// The source's calls drive notify, which walks the tree: never on an async thread.
async fn pin(source: &Arc<dyn Source>, root: &str, sink: &Sink) -> bool {
    let (source, root, sink) = (source.clone(), root.to_string(), sink.clone());
    tokio::task::spawn_blocking(move || source.pin(&root, sink))
        .await
        .unwrap_or(false)
}

async fn unpin(source: &Arc<dyn Source>, root: &str, sink: &Sink) {
    let (source, root, sink) = (source.clone(), root.to_string(), sink.clone());
    let _ = tokio::task::spawn_blocking(move || source.unpin(&root, &sink)).await;
}

/// Starts the capture of a workspace and returns once it has learned it; a failed start leaves nothing running (2102).
pub async fn spawn(opts: Options, deps: Deps, window: Arc<Window>) -> io::Result<Handle> {
    let root = opts.root.clone();
    let (tx, rx) = mpsc::unbounded_channel();
    let sink = sink_of(tx.downgrade());
    // Pinned before the start reads anything, so a change during it is heard.
    let up = pin(&deps.source, &root, &sink).await;
    let started = {
        let (baseline, mut cap) = (deps.baseline.clone(), LiveCapture::new(opts));
        tokio::task::spawn_blocking(move || cap.begin(baseline).map(|first| (cap, first))).await
    };
    let (cap, first) = match started {
        Ok(Ok(started)) => started,
        Ok(Err(e)) => return fail(&deps, &root, &sink, e).await,
        Err(e) => return fail(&deps, &root, &sink, io::Error::other(e.to_string())).await,
    };
    let published = Arc::new(Published::default());
    let handle = Handle {
        root,
        window: window.clone(),
        published: published.clone(),
        tx,
    };
    let mut worker = Worker::new(cap, &handle, deps, sink, up, first);
    tokio::spawn(async move { worker.run(rx).await });
    Ok(handle)
}

async fn fail(deps: &Deps, root: &str, sink: &Sink, e: io::Error) -> io::Result<Handle> {
    unpin(&deps.source, root, sink).await;
    Err(e)
}

/// One scan's end, on the blocking pool: committed if no window opened since `epoch`, else dropped (2121).
fn finish(cap: &mut LiveCapture, win: &Window, step: io::Result<Step>, epoch: u64) {
    match step {
        Ok(step) if win.count() == 0 && win.epoch() == epoch => {
            if !step.is_empty() {
                cap.add_labels(win.take_labels());
            }
            if let Err(e) = cap.commit(step) {
                cap.step_failed(&e);
            }
        }
        Ok(step) => cap.abort(step),
        Err(e) => cap.step_failed(&e),
    }
}

struct Worker {
    cap: Option<LiveCapture>,
    root: String,
    window: Arc<Window>,
    published: Arc<Published>,
    sink: Sink,
    deps: Deps,
    feed: Feed,
    queue: VecDeque<Entry>,
    first: bool,
    quiet_at: Option<Instant>,
    young_at: Option<Instant>,
    sweep_at: Instant,
    again_at: Option<Instant>,
    scan_wait: bool,
}

impl Worker {
    fn new(cap: LiveCapture, h: &Handle, deps: Deps, sink: Sink, up: bool, first: bool) -> Worker {
        let now = Instant::now();
        let mut w = Worker {
            cap: Some(cap),
            feed: Feed::new(deps.timing.feed),
            sweep_at: now + deps.timing.first_sweep,
            root: h.root.clone(),
            window: h.window.clone(),
            published: h.published.clone(),
            sink,
            deps,
            queue: VecDeque::new(),
            first,
            quiet_at: None,
            young_at: None,
            again_at: None,
            scan_wait: false,
        };
        w.watching(up);
        w.after_step();
        let published = w.published.clone();
        published.publish_pending(w.cap());
        w
    }

    fn cap(&mut self) -> &mut LiveCapture {
        self.cap
            .as_mut()
            .expect("the capture is back from its step")
    }

    /// A step that panicked took the capture with it.
    fn dead(&self) -> bool {
        self.cap.is_none()
    }

    fn watching(&mut self, up: bool) {
        self.cap().set_watching(up);
        self.published.watching.store(up, Ordering::SeqCst);
    }

    /// `f` over the capture on the blocking pool; the state goes in and comes back (2120).
    async fn blocking<R: Send + 'static>(
        &mut self,
        f: impl FnOnce(&mut LiveCapture) -> R + Send + 'static,
    ) -> Option<R> {
        let mut cap = self.cap.take()?;
        let published = self.published.clone();
        match tokio::task::spawn_blocking(move || {
            let r = f(&mut cap);
            published.publish_pending(&cap);
            (cap, r)
        })
        .await
        {
            Ok((cap, r)) => {
                self.cap = Some(cap);
                Some(r)
            }
            Err(e) => {
                let line = format!("   ✗ live capture: {}: a step panicked: {e}", self.root);
                self.deps.out.note(&self.root, &line);
                None
            }
        }
    }

    /// What a step left for the owner: batches, blobs asked for, notes and the paths the feed gathers.
    fn after_step(&mut self) {
        let (root, out) = (self.root.clone(), self.deps.out.clone());
        let cap = self.cap();
        let batches = cap.take_batches();
        let missing = cap.take_missing();
        let (notes, paths, generation) = (cap.take_notes(), cap.take_paths(), cap.generation());
        let (young, armed) = (cap.until_young(), cap.quiet_armed());
        for b in batches {
            out.batch(&root, b);
        }
        if !missing.is_empty() {
            out.missing(&root, missing);
        }
        for n in notes {
            out.note(&root, &n);
        }
        let now = Instant::now();
        self.feed.push(paths, now);
        self.published
            .generation
            .store(generation, Ordering::SeqCst);
        self.young_at = young.map(|ms| now + Duration::from_secs_f64(ms / 1000.0));
        let free = self.window.count() == 0;
        match (armed, self.quiet_at) {
            (true, None) if free => self.quiet_at = Some(now + self.deps.timing.quiet),
            (false, _) => self.quiet_at = None,
            _ => {}
        }
    }

    async fn run(&mut self, mut rx: mpsc::UnboundedReceiver<Msg>) {
        if self.first {
            self.flush(true).await;
        }
        loop {
            if self.dead() {
                self.die().await;
                return;
            }
            self.after_step();
            let due = [
                self.quiet_at,
                self.young_at,
                self.feed.due(),
                Some(self.sweep_at),
                self.again_at,
            ];
            let wake = due.into_iter().flatten().min().unwrap_or(self.sweep_at);
            tokio::select! {
                msg = rx.recv() => match msg {
                    Some(Msg::Stop(done)) => {
                        self.stop().await;
                        let _ = done.send(());
                        return;
                    }
                    Some(Msg::Abandon(done)) => {
                        self.die().await;
                        let _ = done.send(());
                        return;
                    }
                    Some(m) => self.handle(m).await,
                    None => {
                        self.stop().await;
                        return;
                    }
                },
                () = sleep_until(wake.into()) => self.tick().await,
            }
            self.drain_entries().await;
        }
    }

    async fn handle(&mut self, msg: Msg) {
        match msg {
            Msg::Paths(rels) => {
                let cap = self.cap();
                for r in &rels {
                    cap.touch(r);
                }
                // Every report restarts the quiet period, so a burst is one flush.
                let free = self.window.count() == 0;
                self.quiet_at = free.then(|| Instant::now() + self.deps.timing.quiet);
            }
            Msg::Release => {
                self.cap().release("");
                self.quiet_at = None;
            }
            Msg::Run(task) => {
                self.blocking(task).await;
            }
            Msg::Entry(entry) => {
                self.queue.push_back(entry);
                self.published.entries.fetch_add(1, Ordering::SeqCst);
            }
            Msg::Flush(done) => {
                self.flush(true).await;
                // The scan's batch is with the owner before the caller hears the scan is over.
                if !self.dead() {
                    self.after_step();
                }
                let _ = done.send(());
            }
            Msg::Capture(files, done) => {
                self.capture(files).await;
                // The owner hears the scan's batch before it hears this answer (2122).
                if !self.dead() {
                    self.after_step();
                }
                let _ = done.send(());
            }
            Msg::Stop(_) | Msg::Abandon(_) => {}
        }
    }

    /// Whichever timers are due.
    async fn tick(&mut self) {
        let now = Instant::now();
        if self.again_at.is_some_and(|t| t <= now) {
            self.again_at = None;
        }
        if self.young_at.is_some_and(|t| t <= now) {
            self.young_at = None;
            self.cap().on_young();
        }
        if self.quiet_at.is_some_and(|t| t <= now) {
            self.quiet_at = None;
            if self.window.count() == 0
                && let Some(full) = self.cap().quiet_plan()
            {
                self.flush(full).await;
            }
        }
        if self.dead() {
            return;
        }
        if self.sweep_at <= now {
            self.sweep_at = now + self.deps.timing.sweep;
            self.sweep().await;
        }
        if self.dead() {
            return;
        }
        if self.feed.due().is_some_and(|t| t <= now) {
            let (paths, generation) = (self.feed.take(), self.cap().generation());
            self.deps.out.feed(&self.root, paths, generation);
        }
    }

    async fn sweep(&mut self) {
        let mut up = self.deps.source.up(&self.root);
        if !up {
            // A watcher that went down is pinned again, which the process's watcher rate-limits itself.
            up = pin(&self.deps.source, &self.root, &self.sink).await;
        }
        self.watching(up);
        if self.window.count() == 0 && self.cap().sweep_due(false) {
            self.flush(true).await;
        }
    }

    /// One scan and its end in one blocking step, committed only if no window opened while it ran (2121).
    async fn flush(&mut self, full: bool) {
        let win = self.window.clone();
        let epoch = win.epoch();
        if win.count() != 0 {
            return;
        }
        // A window closed since the last scan sweeps whole, even before the worker has read its release.
        let full = full || win.take_closed();
        let hook = self.deps.hook.clone();
        self.blocking(move |cap| {
            let step = cap.step_flush(full);
            if let Some(h) = hook {
                h();
            }
            finish(cap, &win, step, epoch);
        })
        .await;
    }

    /// A scan of just the named files, committed like any other only if no window opened since it began (2121).
    async fn capture(&mut self, files: Vec<String>) {
        if files.len() > DIRTY_SWEEP {
            return self.flush(true).await;
        }
        let win = self.window.clone();
        let epoch = win.epoch();
        if win.count() != 0 {
            return;
        }
        self.blocking(move |cap| {
            let step = cap.step_files(&files);
            finish(cap, &win, step, epoch);
        })
        .await;
    }

    /// Entries queued while a window was open, in the order asked, each only while none is (2122, 2127).
    async fn drain_entries(&mut self) {
        // What a scan just emitted is with the owner before an entry merges over the files it changed.
        if !self.dead() {
            self.after_step();
        }
        let mut again = 0;
        // The head entry waits for a sweep that a closed window asked for: nothing is tried before it commits.
        if self.scan_wait {
            if self.dead() || self.cap().window_closed() {
                return;
            }
            self.scan_wait = false;
        }
        while self.window.count() == 0 {
            let Some(mut entry) = self.queue.pop_front() else {
                break;
            };
            let win = self.window.clone();
            let ran = self
                .blocking(move |cap| {
                    let tried = entry(cap, &win);
                    (entry, tried)
                })
                .await;
            match ran {
                Some((entry, Tried::Window)) => {
                    self.queue.push_front(entry);
                    break;
                }
                Some((entry, Tried::Scan)) => {
                    self.queue.push_front(entry);
                    self.scan_wait = true;
                    break;
                }
                Some((entry, Tried::Again)) => {
                    self.queue.push_front(entry);
                    again += 1;
                    // A disk that moves at every try yields to the mailbox, then tries again shortly.
                    if again >= AGAIN_AT_MOST {
                        self.again_at = Some(Instant::now() + AGAIN_PAUSE);
                        break;
                    }
                }
                Some(_) => {
                    again = 0;
                    self.after_step();
                }
                None => break,
            }
        }
    }

    /// A step panicked: the worker ends as stopped, its root unpinned, and its owner starts another (2128).
    async fn die(&mut self) {
        unpin(&self.deps.source, &self.root, &self.sink).await;
        self.published.watching.store(false, Ordering::SeqCst);
        self.published.stopped.store(true, Ordering::SeqCst);
    }

    async fn stop(&mut self) {
        self.blocking(|cap| cap.stop()).await;
        unpin(&self.deps.source, &self.root, &self.sink).await;
        if self.dead() {
            self.published.stopped.store(true, Ordering::SeqCst);
            return;
        }
        self.after_step();
        let generation = self.cap.as_ref().map_or(0, LiveCapture::generation);
        let paths = self.feed.take();
        if !paths.is_empty() {
            self.deps.out.feed(&self.root, paths, generation);
        }
        self.published.stopped.store(true, Ordering::SeqCst);
    }
}
