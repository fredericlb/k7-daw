//! runtime — the account's Logs ring, its background registry and the machine asks, in the front's memory (1n, N2).
//! They live in `Held`, for the account's whole life (2620, 2630).
//! What a reader gets wrong: no lock here is held across an await.

pub mod host;
pub mod link_machine;
pub mod logs;
pub mod report;
pub mod stop;

#[cfg(test)]
mod tests_held;
#[cfg(test)]
#[cfg(test)]
mod tests_host;
#[cfg(test)]
mod tests_logs;
#[cfg(test)]
mod tests_stop;

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use k7_file_service::runtime::background::Jobs;
use k7_file_service::runtime::ring::{Level, Ring};

pub use k7_file_service::runtime::STOP;

use self::logs::Machine;

/// What the front counts of an account's runtime, read by the report line (2680).
#[derive(Default)]
pub struct Counters {
    /// `/logs` answered.
    pub logs: AtomicU64,
    /// Jobs registered.
    pub background: AtomicU64,
}

/// A place in a bounded count, given back by its drop, so a future dropped mid-await never leaks it.
pub(super) struct Slot<'a>(&'a AtomicUsize);

impl<'a> Slot<'a> {
    pub(super) fn take(count: &'a AtomicUsize, limit: usize) -> Option<Slot<'a>> {
        if count.fetch_add(1, Ordering::AcqRel) >= limit {
            count.fetch_sub(1, Ordering::AcqRel);
            return None;
        }
        Some(Slot(count))
    }
}

impl Drop for Slot<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

struct State {
    ring: Ring,
    jobs: Jobs,
}

/// One account's ring and registry, and the machine its `/logs` and asks reach.
pub struct Runtime {
    state: Mutex<State>,
    machine: Arc<dyn Machine>,
    /// How long `/logs` waits for the machine; shrunk by the tests.
    pub(super) wait: std::time::Duration,
    /// `/logs` machine asks in flight, at most `logs::ASKS` (2624).
    pub(super) asks: AtomicUsize,
    /// `jobs.ask`s in flight: while one is, no lease lapses (2631).
    pub(super) flights: AtomicUsize,
    /// Added to the clock by the tests, which cannot wait ten minutes.
    #[cfg(test)]
    pub(super) skew: AtomicU64,
    pub counters: Counters,
    /// The account's module-watch joins (2640); the process shares the watchers beneath.
    pub watch: Arc<k7_file_service::modwatch::Watch>,
}

impl Runtime {
    pub fn new(machine: Arc<dyn Machine>) -> Arc<Runtime> {
        Runtime::with_watching(machine, true)
    }

    /// `watching` false is `K7_NO_FS_WATCH`: the joins are kept and no watcher is made.
    pub fn with_watching(machine: Arc<dyn Machine>, watching: bool) -> Arc<Runtime> {
        let origin = Instant::now();
        Arc::new(Runtime {
            state: Mutex::new(State {
                ring: Ring::new(),
                jobs: Jobs::new(origin, k7_hub::account::now_ms()),
            }),
            wait: logs::WAIT,
            machine,
            asks: AtomicUsize::new(0),
            flights: AtomicUsize::new(0),
            #[cfg(test)]
            skew: AtomicU64::new(0),
            counters: Counters::default(),
            watch: Arc::new(k7_file_service::modwatch::Watch::new(watching)),
        })
    }

    #[cfg(test)]
    pub(super) fn new_waiting(
        machine: Arc<dyn Machine>,
        wait: std::time::Duration,
    ) -> Arc<Runtime> {
        let mut r = Arc::try_unwrap(Runtime::new(machine)).ok().unwrap();
        r.wait = wait;
        Arc::new(r)
    }

    pub(super) fn now(&self) -> Instant {
        #[cfg(test)]
        return Instant::now() + std::time::Duration::from_secs(self.skew.load(Ordering::Relaxed));
        #[cfg(not(test))]
        Instant::now()
    }

    /// An ask in flight keeps its jobs' leases fresh, so only silence with nothing asked lapses them.
    fn renew_in_flight(&self, s: &mut State) {
        if self.flights.load(Ordering::Acquire) > 0 {
            s.jobs.heard(self.now());
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// A line for the panel, as `serviceLine` took it: an unknown level is `info`.
    pub fn say(&self, level: &str, text: &str) {
        let level = Level::parse(level).unwrap_or(Level::Info);
        self.lock()
            .ring
            .push(level, text, k7_hub::account::now_ms());
    }

    pub(super) fn machine(&self) -> &Arc<dyn Machine> {
        &self.machine
    }

    /// The process list's entries from the background jobs, oldest first (1j's list, 2630).
    pub fn processes(&self) -> Vec<serde_json::Value> {
        let mut s = self.lock();
        s.jobs
            .list(self.now())
            .iter()
            .map(|e| e.to_json())
            .collect()
    }

    /// Whether a registered job holds the runtime busy and awake (2631).
    pub fn holds(&self) -> bool {
        let mut s = self.lock();
        self.renew_in_flight(&mut s);
        s.jobs.holds(self.now())
    }

    /// The runtime is dropped: every job ends failed and says so (2614), and its module watches are let go.
    pub fn dropped(&self) {
        self.watch.release(None);
        let mut s = self.lock();
        for line in s.jobs.drop_all(self.now()) {
            s.ring.push(Level::Error, &line, k7_hub::account::now_ms());
        }
    }

    /// The jobs whose lease ran out are said once; they no longer hold the runtime (2631).
    pub fn lapsed(&self) {
        let mut s = self.lock();
        self.renew_in_flight(&mut s);
        for tag in s.jobs.lapsed(self.now()) {
            let line = format!(
                "background job '{tag}' was silent for 10 minutes and no longer holds the runtime"
            );
            s.ring.push(Level::Warn, &line, k7_hub::account::now_ms());
        }
    }
}

/// The module watches under `under` let go before a rename or a recycle, off the async threads (2641).
pub async fn release_watches(runtime: Option<&Arc<Runtime>>, under: &str) {
    let Some(r) = runtime.cloned() else {
        return;
    };
    let under = under.to_string();
    let _ = tokio::task::spawn_blocking(move || r.watch.release(Some(&under))).await;
}
