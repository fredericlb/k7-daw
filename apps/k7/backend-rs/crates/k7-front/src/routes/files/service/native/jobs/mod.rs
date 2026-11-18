//! jobs — what the account asks of its engine beyond playing, held natively (sub-step 1k; `apps/k7/backend-rs/docs/file-engine-jobs.md`).
//! What a reader gets wrong: the production `Jobs` is made with `Jobs::new(Some(real_driver))` and nowhere else; a `Jobs` with
//! no driver answers what the TypeScript answers when no machine is bound, and only tests make one.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::routes::files::service::native::callbacks::Reply;
use k7_file_service::jobs::separations::Registry;
use serde_json::Value;

pub mod driver;
pub mod freeze_run;
pub mod freezer;
pub mod host;
pub mod machine;
pub mod preview;
pub mod queries;
pub mod routes;
pub mod separate;
pub mod thawed;
pub mod waiters;

pub mod render;
pub mod report;

#[cfg(all(test, unix))]
mod tests_driver;
#[cfg(test)]
mod tests_freezer;
#[cfg(test)]
mod tests_freezer_deep;
#[cfg(test)]
mod tests_freezer_timer;
#[cfg(test)]
mod tests_hook;
#[cfg(test)]
mod tests_host;
#[cfg(test)]
mod tests_jobs_live;
#[cfg(test)]
mod tests_routes;
#[cfg(test)]
mod tests_separate;
#[cfg(test)]
mod tests_waiters;

pub use thawed::Thawed;

/// Ends a folder's wait when the reply is made or dropped unmade.
pub(super) struct AwaitedEnd(pub Arc<Awaited>, pub String);

impl Drop for AwaitedEnd {
    fn drop(&mut self) {
        self.0.end(&self.1);
    }
}

/// Folders a bridge `jobs.renderAll` waits on, counted per folder.
#[derive(Default)]
pub struct Awaited(Mutex<std::collections::HashMap<String, usize>>);

impl Awaited {
    fn map(&self) -> MutexGuard<'_, std::collections::HashMap<String, usize>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn has(&self, dir: &str) -> bool {
        self.map().contains_key(dir)
    }

    pub(super) fn begin(&self, dir: &str) {
        *self.map().entry(dir.to_owned()).or_insert(0) += 1;
    }

    pub(super) fn end(&self, dir: &str) {
        let mut m = self.map();
        if let Some(n) = m.get_mut(dir) {
            *n -= 1;
            if *n == 0 {
                m.remove(dir);
            }
        }
    }
}

/// What the front does for the stand-ins' asks: K4 implements it over the link, the renderer and the jobs path.
pub trait Driver: Send + Sync + 'static {
    /// Whether some machine can render: this one's engine binary, the bound jobs path, or the server's render machine.
    fn renderer_available(&self) -> bool;
    /// Whether a machine's jobs path is bound, which decides whether a cut can start.
    fn bound(&self) -> bool;
    /// `render.run {doc, outDir, stems, base}`, answered `{"value": {mix, stems}}`.
    fn render_run(&self, args: Value, reply: Reply) {
        self.render_run_until(args, reply, never())
    }
    /// `render_run` that stops when `gone` resolves (the asker gave up); required, so no driver ignores `gone` by default.
    fn render_run_until(&self, args: Value, reply: Reply, gone: Gone);
    /// `jobs.renderAll {dir}`, answered `{"value": impossible}`; progress goes out as `jobs` events.
    fn render_all(&self, dir: String, reply: Reply);
    /// The request of a cut the registry just started (`generation` names the record it may write).
    fn separate(&self, id: &str, generation: u64, args: Value);
    /// One freeze's render, bounced to exactly `out_wav` on this machine or the bound one (2313); the default has nowhere to render.
    fn render_freeze(&self, _doc: String, _out_wav: std::path::PathBuf) -> RenderFuture {
        Box::pin(async { Err("no engine machine to render on".to_owned()) })
    }
    /// Told once by `Jobs::new`, so a request that outlives the call can write its record back.
    fn attach(&self, _jobs: std::sync::Weak<Jobs>) {}
}

/// Resolves when whoever asked no longer wants the answer; `never()` for an ask that cannot be given up.
pub type Gone = std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>;

pub fn never() -> Gone {
    Box::pin(std::future::pending())
}

/// What a freeze's render answers: made, or the sentence of why not.
pub type RenderFuture =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send>>;

/// How many times each call came, for the log line of 2362.
#[derive(Default)]
pub struct Counts {
    pub set_thawed: AtomicU64,
    pub thaws: AtomicU64,
    pub renders: AtomicU64,
    pub freezes: AtomicU64,
    pub queries: AtomicU64,
    pub previews: AtomicU64,
    pub separations: AtomicU64,
    pub panics: AtomicU64,
}

impl Counts {
    /// The line the front logs while the jobs move (2362), which `bedCounters.mjs` reads.
    pub fn line(&self, account: &str) -> String {
        let g = |c: &AtomicU64| c.load(Ordering::Relaxed);
        format!(
            "[files] native jobs {}: renders={} freezes={} queries={} previews={} thaws={} separations={} panics={}",
            account,
            g(&self.renders),
            g(&self.freezes),
            g(&self.queries),
            g(&self.previews),
            g(&self.thaws),
            g(&self.separations),
            g(&self.panics)
        )
    }
}

/// One account's jobs: the thawed modules (2310) and the separations' registry (2330), beside the driver.
pub struct Jobs {
    pub thawed: Arc<Thawed>,
    pub counts: Counts,
    registry: Mutex<Registry>,
    driver: Option<Arc<dyn Driver>>,
    /// The freezer's executor, made by `start_freezer`; a thaw and a `renderAll` reach the machine through it.
    freezing: std::sync::OnceLock<Arc<freeze_run::Freezing>>,
    /// Where a freezer's progress goes: the bridge's sink, set when a runtime attaches.
    events: Mutex<Option<super::callbacks::Sink>>,
    /// Listeners of the same events, for a caller that shows a freeze's per-module progress (1m); never removed.
    taps: Mutex<Vec<super::callbacks::Sink>>,
    /// Folders a bridge `jobs.renderAll` waits on; only their progress reaches the sink (an idle runtime hears nothing).
    awaited: Arc<Awaited>,
}

impl Jobs {
    pub fn new(driver: Option<Arc<dyn Driver>>) -> Arc<Jobs> {
        Arc::new_cyclic(|me| {
            if let Some(d) = &driver {
                d.attach(me.clone());
            }
            Jobs {
                thawed: Arc::new(Thawed::default()),
                counts: Counts::default(),
                registry: Mutex::new(Registry::new()),
                driver,
                freezing: std::sync::OnceLock::new(),
                events: Mutex::new(None),
                taps: Mutex::new(Vec::new()),
                awaited: Arc::default(),
            }
        })
    }

    /// One module left live or let go: the route's and `jobs.thaw`'s one path.
    pub fn thaw(&self, dir: &str, module: &str, on: bool) {
        self.thawed.thaw(dir, module, on);
        self.counts.thaws.fetch_add(1, Ordering::Relaxed);
        if let Some(f) = self.freezing.get() {
            f.thaw(dir, module, on);
        }
    }

    /// The list replaced whole (`setThawed`), the freezer's machine told too.
    pub fn set_thawed(&self, dir: &str, modules: Vec<String>) {
        self.thawed.set(dir, modules.clone());
        self.counts.set_thawed.fetch_add(1, Ordering::Relaxed);
        if let Some(f) = self.freezing.get() {
            f.set_thawed(dir, &modules);
        }
    }

    /// The sink a progress event leaves by; none while no runtime is attached.
    pub fn set_events(&self, sink: Option<super::callbacks::Sink>) {
        *self.events.lock().unwrap_or_else(PoisonError::into_inner) = sink;
    }

    /// Hears every progress event `emit` sends, whether or not a runtime is attached.
    pub fn tap(&self, listener: super::callbacks::Sink) {
        self.taps
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(listener);
    }

    pub(crate) fn emit(&self, event: Value) {
        let taps = self
            .taps
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        for t in taps {
            t(event.clone());
        }
        let waited = event["dir"].as_str().is_some_and(|d| self.awaited.has(d));
        let sink = self
            .events
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        if waited && let Some(sink) = sink {
            sink(event);
        }
    }

    /// A test's stand-in for a bridge `jobs.renderAll` of `dir` in flight; it is never settled.
    #[cfg(test)]
    pub(crate) fn awaiting(&self, dir: &str) {
        self.awaited.begin(dir);
    }

    pub(crate) fn registry(&self) -> MutexGuard<'_, Registry> {
        self.registry.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
