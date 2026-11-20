//! waiters.rs — the three plugin queries' answers, matched on the link by the id or the path they were asked for (2340, 2341).
//! What a reader gets wrong: a waiter is registered BEFORE its frame is queued, and one frame settles every waiter on its key.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use k7_file_service::jobs::{NATIVE_UI_TIMEOUT, PLUGIN_BASE_TIMEOUT, PLUGIN_PARAMS_TIMEOUT};
use serde_json::Value;
use tokio::sync::oneshot;

/// What is waited for: a plugin's parameters, its window, or the write of its state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Want {
    Params,
    Window,
    Base,
    /// The cached plugin list, the end of a scan, the audio interfaces, one arm's answer and the engine's refusal of it (3f).
    Plugins,
    /// The engine's `plugin-scan` that says no scan runs: a list frame cannot end a rescan.
    ScanEnded,
    Devices,
    Armed,
    EngineError,
}

impl Want {
    /// The engine's frame type that answers it.
    fn frame(self) -> &'static str {
        match self {
            Want::Params => "plugin-param-list",
            Want::Window => "native-plugin-ui-opened",
            Want::Base => "plugin-base-saved",
            Want::Plugins => "plugins",
            Want::ScanEnded => "scan-ended",
            Want::EngineError => "error",
            Want::Devices => "audio-devices",
            Want::Armed => "armed",
        }
    }

    /// The field the answer is matched on: the plugin's id, or for a base the file it was asked to write.
    fn key(self) -> &'static str {
        match self {
            Want::Base => "path",
            Want::Armed => "track",
            _ => "id",
        }
    }

    /// How long the engine gets: a plugin loads in seconds, a window opens colder, a base is already open.
    fn limit(self) -> Duration {
        match self {
            Want::Params => PLUGIN_PARAMS_TIMEOUT,
            Want::Window => NATIVE_UI_TIMEOUT,
            Want::Base => PLUGIN_BASE_TIMEOUT,
            Want::Plugins | Want::Devices => Duration::from_secs(4),
            Want::ScanEnded => Duration::from_secs(60),
            Want::Armed | Want::EngineError => Duration::from_secs(2),
        }
    }
}

struct Entry {
    id: u64,
    want: Want,
    key: String,
    tx: oneshot::Sender<Value>,
}

/// The waiters of one link, which the sessions' inbound frames settle.
#[derive(Default)]
pub struct Waiters {
    list: Mutex<Vec<Entry>>,
    next: AtomicU64,
    /// A test's shorter wait, in milliseconds; zero keeps the real limits.
    #[cfg(test)]
    shrunk_ms: AtomicU64,
}

/// `String(msg.id ?? "")`: how the clients read the key of a frame.
fn spelled(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
    }
}

impl Waiters {
    /// Register a wait on `key`; it is dropped from the list when the answer, its limit or its caller ends it.
    pub fn wait(self: &Arc<Waiters>, want: Want, key: &str) -> Waiting {
        let (tx, rx) = oneshot::channel();
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        self.entries().push(Entry {
            id,
            want,
            key: key.to_owned(),
            tx,
        });
        Waiting {
            rx,
            id,
            want,
            owner: self.clone(),
        }
    }

    /// A frame the engine sent: every waiter on its type and key hears it, and none is left behind.
    pub fn settle(&self, kind: &str, frame: &Value) {
        let heard = {
            let mut list = self.entries();
            let (hit, rest): (Vec<Entry>, Vec<Entry>) = std::mem::take(&mut *list)
                .into_iter()
                .partition(|e| e.want.frame() == kind && e.key == spelled(frame.get(e.want.key())));
            *list = rest;
            hit
        };
        for e in heard {
            let _ = e.tx.send(frame.clone());
        }
    }

    /// How many waits are registered, for a test.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries().len()
    }

    /// The limits cut to `ms`, for a test that waits them out.
    #[cfg(test)]
    pub fn shrink(&self, ms: u64) {
        self.shrunk_ms.store(ms, Ordering::Relaxed);
    }

    fn limit(&self, want: Want) -> Duration {
        #[cfg(test)]
        if let ms @ 1.. = self.shrunk_ms.load(Ordering::Relaxed) {
            return Duration::from_millis(ms);
        }
        want.limit()
    }

    fn entries(&self) -> std::sync::MutexGuard<'_, Vec<Entry>> {
        self.list.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// A registered wait; dropping it, answered or not, takes it off the list.
pub struct Waiting {
    rx: oneshot::Receiver<Value>,
    id: u64,
    want: Want,
    owner: Arc<Waiters>,
}

impl Waiting {
    /// The frame that answered, or none once the limit passed.
    pub async fn answer(mut self) -> Option<Value> {
        let limit = self.owner.limit(self.want);
        tokio::time::timeout(limit, &mut self.rx).await.ok()?.ok()
    }
}

impl Drop for Waiting {
    fn drop(&mut self) {
        let id = self.id;
        self.owner.entries().retain(|e| e.id != id);
    }
}
