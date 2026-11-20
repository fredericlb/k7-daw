//! log.rs — where the registry and the hub say what they refused: `[registry] …`
//! on stderr, and `tunnel | …` on stdout. A complaint about one engine is said once a minute at most.

use std::sync::{Arc, Mutex};

use indexmap::IndexMap;

use crate::clock::Clock;

/// How often the same complaint about the same engine is said again: a heartbeat comes every 5 s.
const COMPLAINT_EVERY_MS: i64 = 60_000;
/// An open route must not grow the map for ever out of ids anyone can invent.
const MAX_KEYS: usize = 1000;

/// A line sink: the process's output, or a test's capture.
pub type Sink = Arc<dyn Fn(&str) + Send + Sync>;

/// `[registry] …` on stderr, as `console.warn` writes it.
pub fn registry_sink() -> Sink {
    Arc::new(|l| eprintln!("[registry] {l}"))
}

/// `tunnel | …` on stdout, as the hub's default `console.log` writes it.
pub fn tunnel_sink() -> Sink {
    Arc::new(|l| println!("tunnel | {l}"))
}

/// `throttledLog`: each key said once per minute at most, the oldest key forgotten past MAX_KEYS.
pub struct Throttled {
    sink: Sink,
    clock: Clock,
    last: Mutex<IndexMap<String, i64>>,
}

impl Throttled {
    pub fn new(sink: Sink, clock: Clock) -> Self {
        Throttled {
            sink,
            clock,
            last: Mutex::new(IndexMap::new()),
        }
    }

    pub fn say(&self, key: &str, line: &str) {
        let t = self.clock.now_ms();
        {
            let mut last = self.last.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(seen) = last.get(key)
                && t - seen < COMPLAINT_EVERY_MS
            {
                return;
            }
            last.insert(key.to_string(), t);
            if last.len() > MAX_KEYS {
                last.shift_remove_index(0);
            }
        }
        (self.sink)(line);
    }
}
