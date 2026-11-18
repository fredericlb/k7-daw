//! The account's engine link held natively (2210): the binding, one session per workspace owner with its socket, its
//! machines and its ordered queue, the copy over the jobs path and the pattern watch. The decisions: file-engine.md.
//! What a reader gets wrong: a link is std-Mutex state that never spans an await; a frame is queued under it (a channel
//! push) so order is the lock's order, and every slow thing (stat, compile, dial, job) runs outside it.
//! Lock order: `sessions`, then a session's `st`, then the leaves `binding` and `copies`; nothing is taken under a leaf.

pub mod copy;
pub mod filing;
pub mod flow;
pub mod hold;
pub mod host;
pub mod jobs;
pub mod patterns;
pub mod queue;
pub mod real;
pub mod routes;
pub mod socket;
pub mod spawn;
pub mod starts;
pub mod stats;

#[cfg(test)]
pub mod fake;
#[cfg(test)]
pub mod fake_plugins;
#[cfg(test)]
pub mod testkit;
#[cfg(test)]
mod tests_flow;
#[cfg(test)]
mod tests_frames;
#[cfg(test)]
mod tests_hostile;
#[cfg(test)]
mod tests_jobs_wired;
#[cfg(test)]
mod tests_link;
#[cfg(test)]
mod tests_live;
#[cfg(test)]
mod tests_order;
#[cfg(test)]
pub(crate) mod tests_real;
#[cfg(test)]
mod tests_review;
#[cfg(test)]
mod tests_routes;
#[cfg(test)]
mod tests_scenarios;
#[cfg(test)]
mod tests_scenarios_jobs;
#[cfg(test)]
mod tests_scene;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::{Duration, Instant};

use k7_file_service::engine::docs::TakeArm;
use k7_file_service::engine::sync::Copies;
use k7_file_service::engine::wire::Take;
use k7_file_service::engine::{LINK_IDLE_MS, session_of};
use serde_json::Value;
use tokio::sync::Notify;

use super::jobs::waiters::Waiters;

pub use queue::Session;
pub use stats::Stats;

/// The account the link serves: what `session_of` names sessions by.
#[derive(Clone, Debug)]
pub struct Account {
    pub files_root: String,
    pub email: String,
    /// The roots a copy may state (2224): the account's five.
    pub roots: Vec<String>,
}

/// What the preferences say of the audio device, replayed when a socket opens (2215).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Prefs {
    pub audio_device: Option<(String, String)>,
    pub input_pairs: Vec<i64>,
}

/// What the link asks of the rest of the front. Every method may do I/O: the link calls it from `spawn_blocking`.
/// What it tells (the account's console, the module watch) is `Tells`, which never blocks.
pub trait Host: Send + Sync + 'static {
    /// `cachedProjectDoc`: the project compiled, the cache answering when the sources did not move.
    fn compile(&self, root: &str, project: &str) -> Result<Value, String>;
    /// `compile` for a push, on the runtime's thread, which cannot ask the account: what only the push holds
    /// (`ask`: rev, thawed) rides along. `None` declines: the push is not answered.
    fn compile_asked(
        &self,
        root: &str,
        project: &str,
        _ask: &Value,
    ) -> Result<Option<Value>, String> {
        self.compile(root, project).map(Some)
    }
    /// `invalidateProjectDoc`.
    fn invalidate(&self, root: &str);
    /// The workspace's revision token, which the external-plugin memo is keyed by.
    fn revision(&self, root: &str) -> String;
    fn names_external(&self, root: &str, project: &str) -> bool;
    /// The records the desk's playlist names and their stems, which the copy sends beside the document's.
    fn named_files(&self, root: &str, project: &str) -> Vec<String>;
    /// Every workspace of `files_root` with the project its marker names open, as (root, project); the prefetch's reach.
    fn open_projects(&self, _files_root: &str) -> Vec<(String, String)> {
        vec![]
    }
    /// `patternFiles`: the pattern files the compile inlines.
    fn pattern_files(&self, project: &str) -> Vec<String>;
    fn preferences(&self) -> Prefs;
    /// `writeScanCopy`: the catalogue the scan ended on.
    fn write_catalogue(&self, plugins: &Value);
    /// A take filed into `project` (O1): the clip's id, or why it was not. Blocking: the bridge calls it from a blocking task.
    fn file_take(
        &self,
        root: &str,
        project: &str,
        take: &Take,
        arm: Option<&TakeArm>,
    ) -> Result<String, String>;
}

/// What the link tells the rest of the front, in the order it happened: each call is an enqueue made inline, so it must not block;
/// the implementor keeps one ordered channel per account and awaits the slow part (a filing, a delivery) in its own task.
pub trait Tells: Send + Sync + 'static {
    /// A take the engine recorded, for the filing (2242): the next one waits its turn.
    fn recorded(&self, session: &str, take: Take);
    /// A frame of a type something listens for (2217); `state` is never forwarded.
    fn forward(&self, session: &str, kind: &str, frame: &Value);
    /// A console line.
    fn say(&self, line: &str);
}

/// The binding the hub last told (2211).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Binding {
    pub url: Option<String>,
    pub jobs_url: Option<String>,
    pub engine: Option<(String, String)>,
}

pub(crate) struct Inner {
    pub account: Account,
    pub host: Arc<dyn Host>,
    pub tells: Arc<dyn Tells>,
    pub rt: tokio::runtime::Handle,
    pub stats: Stats,
    /// The last counters' line logged, so a flush says nothing twice.
    said: Mutex<String>,
    binding: Mutex<Binding>,
    sessions: Mutex<HashMap<String, Arc<Session>>>,
    pub(crate) copies: Mutex<Copies>,
    next_link: std::sync::atomic::AtomicU64,
    /// Woken when a machine has a deadline to watch, so an idle link sleeps.
    pub(crate) wake: Notify,
    /// The plugin queries awaiting the engine's answer, which any session's frame may settle (2340).
    pub(crate) waiters: Arc<Waiters>,
}

/// A std mutex read through a poisoned lock: a panic in one task must not take the session's state with it.
pub(crate) fn held<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// One account's engine link: cheap to clone, the sessions and the binding behind it.
#[derive(Clone)]
pub struct EngineLink(pub(crate) Arc<Inner>);

impl EngineLink {
    /// Must be made inside a runtime: its ticker and sweeper run on it.
    pub fn new(account: Account, host: Arc<dyn Host>, tells: Arc<dyn Tells>) -> EngineLink {
        let inner = Arc::new(Inner {
            account,
            host,
            tells,
            rt: tokio::runtime::Handle::current(),
            stats: Stats::default(),
            said: Mutex::new(String::new()),
            binding: Mutex::new(Binding::default()),
            sessions: Mutex::new(HashMap::new()),
            copies: Mutex::new(Copies::new()),
            next_link: std::sync::atomic::AtomicU64::new(1),
            wake: Notify::new(),
            waiters: Arc::default(),
        });
        tick_task(Arc::downgrade(&inner));
        sweep_task(Arc::downgrade(&inner));
        report_task(Arc::downgrade(&inner));
        EngineLink(inner)
    }

    pub fn stats(&self) -> &Stats {
        &self.0.stats
    }

    pub fn binding(&self) -> Binding {
        held(&self.0.binding).clone()
    }

    /// The jobs path of the bound machine, or none.
    pub fn jobs_url(&self) -> Option<String> {
        held(&self.0.binding).jobs_url.clone()
    }

    /// `hosted.ts`' `/hub/engine`: no URL clears the binding and drops every link; the same URL with links open changes
    /// nothing of the engine; another drops every link, none told `drop-session` (2211). Cheap enough to call before each request.
    pub fn bind(&self, told: &Value) {
        let url = told
            .get("url")
            .and_then(Value::as_str)
            .filter(|u| !u.is_empty())
            .map(str::to_string);
        let text = |k: &str| {
            told.get(k)
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        };
        let id = text("engineId");
        let (jobs_url, engine) = match &url {
            Some(_) => (
                text("jobsUrl"),
                id.map(|id| {
                    let name = text("engineName").unwrap_or_else(|| id.clone());
                    (id, name)
                }),
            ),
            None => (None, None),
        };
        // Read before the binding is taken: `binding` is a leaf under a session's lock.
        let open = self.links_open();
        let drop_links = {
            let mut b = held(&self.0.binding);
            let same = b.url == url && url.is_some() && open;
            b.url = url;
            b.jobs_url = jobs_url;
            b.engine = engine;
            !same
        };
        if drop_links {
            self.drop_all();
        }
    }

    fn links_open(&self) -> bool {
        held(&self.0.sessions).values().any(|s| s.has_socket())
    }

    /// Every session's socket closed and its machines told the engine is gone; what each served stays.
    fn drop_all(&self) {
        let all: Vec<Arc<Session>> = held(&self.0.sessions).values().cloned().collect();
        for s in all {
            s.drop_socket();
        }
    }

    /// The session for `root` (a workspace's folder, or none for the bare one), made on first asking.
    pub fn session(&self, root: Option<&str>) -> Arc<Session> {
        let name = session_of(root, &self.0.account.files_root, &self.0.account.email);
        self.session_named(&name)
    }

    pub fn session_named(&self, name: &str) -> Arc<Session> {
        let mut map = held(&self.0.sessions);
        map.entry(name.to_string())
            .or_insert_with(|| Session::new(name, Arc::downgrade(&self.0)))
            .clone()
    }

    pub fn find(&self, name: &str) -> Option<Arc<Session>> {
        held(&self.0.sessions).get(name).cloned()
    }

    /// Every session, for the served list and the sweep.
    pub fn sessions(&self) -> Vec<Arc<Session>> {
        held(&self.0.sessions).values().cloned().collect()
    }

    /// Close the sockets nothing used for `LINK_IDLE_MS` and that serve nothing; the names dropped. `now` is for the test.
    pub fn prune_idle(&self, now: Instant) -> Vec<String> {
        let mut out = vec![];
        for s in self.sessions() {
            if s.idle_since(now) >= Duration::from_millis(LINK_IDLE_MS) && s.drop_if_unserved() {
                out.push(s.name().to_string());
            }
        }
        out
    }

    /// The account's reap: every socket closed without `drop-session` (2210), the sessions forgotten.
    pub fn close(&self) {
        self.drop_all();
        held(&self.0.sessions).clear();
        // The counters' last line goes out here: `Inner` is never dropped (docs 2262).
        self.0.flush();
    }
}

impl Inner {
    pub(crate) fn binding_url(&self) -> Option<String> {
        held(&self.binding).url.clone()
    }

    pub(crate) fn jobs_url(&self) -> Option<String> {
        held(&self.binding).jobs_url.clone()
    }

    pub(crate) fn new_link(&self) -> u64 {
        self.next_link
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    pub(crate) fn sessions_snapshot(&self) -> Vec<Arc<Session>> {
        held(&self.sessions).values().cloned().collect()
    }
}

/// Every 100 ms while a session has a deadline: the machines' clock. Parks on `wake` when none does.
fn tick_task(inner: Weak<Inner>) {
    let Some(i) = inner.upgrade() else { return };
    i.rt.spawn(async move {
        loop {
            let Some(i) = inner.upgrade() else { return };
            let busy = i.sessions_snapshot().iter().fold(false, |b, s| {
                let wanted = s.tick(Instant::now());
                b || wanted
            });
            if busy {
                drop(i);
                tokio::time::sleep(Duration::from_millis(100)).await;
            } else {
                let parked = i.clone();
                drop(i);
                // A wake that came since the scan above is kept as a permit, so none is lost.
                tokio::select! {
                    _ = parked.wake.notified() => {}
                    _ = tokio::time::sleep(Duration::from_secs(1)) => {}
                }
                // …and a permit a tick itself left cannot make the loop spin.
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
    });
}

/// The counters' line once they moved: every 30 s, at once when a panic is counted, and at the end (2262).
fn report_task(inner: Weak<Inner>) {
    let Some(i) = inner.upgrade() else { return };
    i.rt.spawn(async move {
        let (mut since, mut panics) = (Instant::now(), 0);
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let Some(i) = inner.upgrade() else { return };
            let now = Stats::get(&i.stats.panics);
            if since.elapsed() >= REPORT_EVERY || now != panics {
                (since, panics) = (Instant::now(), now);
                i.flush();
            }
        }
    });
}

impl Inner {
    /// Logs the counters' line if it moved since the last one.
    fn flush(&self) {
        let line = self
            .stats
            .report(&self.account.email, &mut held(&self.said));
        if let Some(line) = line {
            self.tells.say(&line);
        }
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        self.flush();
    }
}

/// How often the counters' line may be logged.
const REPORT_EVERY: Duration = Duration::from_secs(30);

/// Every minute: sockets abandoned for five.
fn sweep_task(inner: Weak<Inner>) {
    let Some(i) = inner.upgrade() else { return };
    i.rt.spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
            let Some(i) = inner.upgrade() else { return };
            EngineLink(i).prune_idle(Instant::now());
        }
    });
}
