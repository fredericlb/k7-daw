//! flights.rs — the decodes in flight, one per (real path, sensitivity) in the process (2412): joined by a request and the
//! prewarm alike, locked only to join or leave. What a reader gets wrong: only requests are counted, so a prewarm's cancel
//! stops a decode no request waits on and leaves one a request joined to run.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, LazyLock, Mutex};

use k7_mediatek::sidecar::Envelope;
use tokio::sync::watch;

pub type Env = Arc<Envelope>;
pub type Outcome = Result<Env, String>;
pub type Key = (String, u64);

/// A decode running: who waits on it, whether it must stop, and where its outcome is published.
pub struct Flight {
    pub requests: AtomicUsize,
    pub stop: AtomicBool,
    pub owner: u64,
    /// The (account, workspace) prewarms that joined: a cancel withdraws its own and stops the decode when nobody else wants it.
    pub prewarms: Mutex<Vec<(u64, String)>>,
    pub rx: watch::Receiver<Option<Outcome>>,
}

static FLIGHTS: LazyLock<Mutex<HashMap<Key, Arc<Flight>>>> = LazyLock::new(Default::default);

fn map() -> std::sync::MutexGuard<'static, HashMap<Key, Arc<Flight>>> {
    FLIGHTS.lock().unwrap_or_else(|p| p.into_inner())
}

/// `source` and the threshold asked, spelled as one key (`none` for no threshold asked).
pub fn key(source: &str, sensitivity: Option<f64>) -> Key {
    (
        source.to_string(),
        sensitivity.map_or(u64::MAX, |s| s.to_bits()),
    )
}

/// A request waiting on a flight, counted until it stops waiting.
pub struct Waiter(Option<Arc<Flight>>);

impl Drop for Waiter {
    fn drop(&mut self) {
        if let Some(f) = &self.0 {
            f.requests.fetch_sub(1, Ordering::Relaxed);
        }
    }
}

/// What a caller got: the flight to wait on, and the sender when it is the one that must run it.
pub struct Joined {
    pub flight: Arc<Flight>,
    pub starts: Option<watch::Sender<Option<Outcome>>>,
    pub waiter: Waiter,
}

/// Join the flight for `key`, or make it (a stopped one is replaced); a request is counted, a prewarm is not.
pub fn join(key: &Key, request: bool, owner: u64, ws: &str) -> Joined {
    let mut flights = map();
    let live = flights
        .get(key)
        .filter(|f| !f.stop.load(Ordering::Relaxed))
        .cloned();
    let (flight, starts) = match live {
        Some(f) => (f, None),
        None => {
            let (tx, rx) = watch::channel(None);
            let f = Arc::new(Flight {
                requests: AtomicUsize::new(0),
                stop: AtomicBool::new(false),
                owner,
                prewarms: Mutex::default(),
                rx,
            });
            flights.insert(key.clone(), f.clone());
            (f, Some(tx))
        }
    };
    if request {
        flight.requests.fetch_add(1, Ordering::Relaxed);
    } else {
        let mut held = flight.prewarms.lock().unwrap_or_else(|p| p.into_inner());
        if !held.iter().any(|(o, w)| *o == owner && w == ws) {
            held.push((owner, ws.to_string()));
        }
    }
    let waiter = Waiter(request.then(|| flight.clone()));
    Joined {
        flight,
        starts,
        waiter,
    }
}

/// The flight's outcome is published: it leaves the map if the entry is still its own, and a later ask starts its own.
pub fn leave(key: &Key, flight: &Arc<Flight>) {
    let mut flights = map();
    if flights.get(key).is_some_and(|f| Arc::ptr_eq(f, flight)) {
        flights.remove(key);
    }
}

/// A workspace's cancel (2431): its prewarm leaves every flight it joined, and a flight no request and no other prewarm wants stops; the count stopped.
pub fn cancel_unwanted(owner: u64, ws: &str) -> usize {
    let mut stopped = 0;
    for f in map().values() {
        let mut held = f.prewarms.lock().unwrap_or_else(|p| p.into_inner());
        let before = held.len();
        held.retain(|(o, w)| !(*o == owner && w == ws));
        let mine = held.len() != before;
        if mine && held.is_empty() && f.requests.load(Ordering::Relaxed) == 0 {
            f.stop.store(true, Ordering::Relaxed);
            stopped += 1;
        }
    }
    stopped
}

#[cfg(test)]
/// Whether any flight is still running for `source`, for a test.
pub fn running(source: &str) -> usize {
    map().keys().filter(|k| k.0 == source).count()
}

/// Whether the owner has a decode in flight.
pub fn owns(owner: u64) -> bool {
    map().values().any(|f| f.owner == owner)
}

/// How many of the owner's flights are stopped and not yet gone: the decodes a cancel is waiting on to let go of their files.
pub fn stopping(owner: u64) -> usize {
    map()
        .values()
        .filter(|f| f.owner == owner && f.stop.load(Ordering::Relaxed))
        .count()
}
