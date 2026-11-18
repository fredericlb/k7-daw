//! bindings.rs — which TUNNEL SECRET each engine id belongs to: the first secret a heartbeat carries binds
//! the id (trust on first use), and from then on only that secret lists or tunnels under it. Kept as
//! id → sha256(secret) in `engine-bindings.json`, the file the previous release reads too (`apps/k7/backend-rs/docs/engine/token.md`).

use std::path::PathBuf;

use indexmap::IndexMap;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use super::state_file::{load_json, save_json};
use crate::js;
use crate::log::Sink;

/// The file's name under a host's state dir.
pub const BINDINGS_FILE: &str = "engine-bindings.json";
/// Past this many ids the least recently seen binding goes: an open heartbeat must not grow the file for ever.
const MAX_BINDINGS: usize = 10_000;
/// A bound id seen again rewrites its `at` at most this often.
const REFRESH_MS: f64 = 24.0 * 60.0 * 60_000.0;

/// What a secret is worth for an id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The id's own secret.
    Bound,
    /// An unbound id offering one.
    Fresh,
    /// Unbound, and none offered.
    Legacy,
    /// Bound to another.
    Refused,
}

struct Binding {
    /// sha256 of the secret, hex.
    h: String,
    /// Epoch ms it was last bound or refreshed, as JavaScript holds a number.
    at: f64,
}

pub struct Bindings {
    file: Option<PathBuf>,
    map: IndexMap<String, Binding>,
    log: Sink,
}

fn digest(secret: &str) -> String {
    Sha256::digest(secret.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

impl Bindings {
    /// The bindings at `file`, or in memory alone when None.
    pub fn new(file: Option<PathBuf>, log: Sink, now_ms: i64) -> Self {
        let mut map = IndexMap::new();
        if let Some(f) = &file
            && let Some(Value::Object(o)) = load_json(f, "engine bindings", &log, now_ms)
                .as_ref()
                .and_then(|r| r.get("engines"))
        {
            for (id, v) in o {
                if let (Some(h), Some(at)) = (
                    v.get("h").and_then(Value::as_str),
                    v.get("at").and_then(Value::as_f64),
                ) {
                    map.insert(
                        id.clone(),
                        Binding {
                            h: h.to_string(),
                            at,
                        },
                    );
                }
            }
        }
        Bindings { file, map, log }
    }

    pub fn check(&self, id: &str, secret: Option<&str>) -> Verdict {
        // An empty secret is none, as JavaScript's truthiness reads it.
        match (self.map.get(id), secret.filter(|s| !s.is_empty())) {
            (None, Some(_)) => Verdict::Fresh,
            (None, None) => Verdict::Legacy,
            (Some(b), Some(s)) if bool::from(digest(s).as_bytes().ct_eq(b.h.as_bytes())) => {
                Verdict::Bound
            }
            (Some(_), _) => Verdict::Refused,
        }
    }

    /// Binds an unbound id, or notes a bound one seen; the caller checks first.
    pub fn bind(&mut self, id: &str, secret: &str, now_ms: i64) {
        let t = now_ms as f64;
        if let Some(b) = self.map.get_mut(id) {
            if t - b.at < REFRESH_MS {
                return;
            }
            b.at = t;
            return self.save();
        }
        self.map.insert(
            id.to_string(),
            Binding {
                h: digest(secret),
                at: t,
            },
        );
        if self.map.len() > MAX_BINDINGS {
            let oldest = self
                .map
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| a.1.at.total_cmp(&b.1.at))
                .map(|(i, _)| i);
            if let Some(i) = oldest {
                self.map.shift_remove_index(i);
            }
        }
        (self.log)(&format!("engine {id} bound to its tunnel secret"));
        self.save();
    }

    fn save(&self) {
        let Some(file) = &self.file else { return };
        let engines: Map<String, Value> = self
            .map
            .iter()
            .map(|(id, b)| (id.clone(), json!({ "h": b.h, "at": js::num(b.at) })))
            .collect();
        save_json(
            file,
            &json!({ "version": 1, "engines": engines }),
            "engine bindings",
            &self.log,
        );
    }
}
