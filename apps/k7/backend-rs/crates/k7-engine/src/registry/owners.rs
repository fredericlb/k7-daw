//! owners.rs — which ACCOUNT each engine id belongs to and whom its owner shared it with, kept
//! in `engine-owners.json`. The owner is the account a heartbeat PROVES with a device token, never
//! whoever opened it first (`apps/k7/backend-rs/docs/engine/ownership.md`).

use std::path::PathBuf;

use indexmap::IndexMap;
use serde_json::{Map, Value, json};

use super::state_file::{load_json, save_json};
use crate::js;
use crate::log::Sink;

/// The file's name under a host's state dir.
pub const OWNERS_FILE: &str = "engine-owners.json";
/// Past this many owned ids the oldest record goes.
const MAX_OWNED: usize = 10_000;
/// How many accounts one engine may be shared with.
pub const MAX_SHARED: usize = 50;

#[derive(Clone, Debug, PartialEq)]
pub struct Ownership {
    pub owner: String,
    /// Lowercase emails.
    pub shared: Vec<String>,
    pub everyone: bool,
    /// Epoch ms this owner was assigned, as JavaScript holds a number.
    pub since: f64,
}

impl Ownership {
    /// `{ owner, shared, everyone, since }`, in that order.
    pub fn json(&self) -> Map<String, Value> {
        let v = json!({ "owner": self.owner, "shared": self.shared, "everyone": self.everyone, "since": js::num(self.since) });
        v.as_object().cloned().unwrap_or_default()
    }
}

/// What an account is to one engine: only the first three may claim it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Owner,
    Shared,
    Server,
    Unowned,
    None,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Owner => "owner",
            Role::Shared => "shared",
            Role::Server => "server",
            Role::Unowned => "unowned",
            Role::None => "none",
        }
    }

    /// `mayUse`: whether the role may claim the engine and read its token through the relay.
    pub fn may_use(self) -> bool {
        matches!(self, Role::Owner | Role::Shared | Role::Server)
    }
}

/// `assign`'s three answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Assigned {
    Same,
    New,
    Moved,
}

/// `roleOf`: what `holder` is to an engine with record `o`; the server's own engine is everybody's.
pub fn role_of(o: Option<&Ownership>, holder: &str, server: bool) -> Role {
    if server {
        return Role::Server;
    }
    let Some(o) = o else { return Role::Unowned };
    if holder.is_empty() {
        return Role::None;
    }
    if o.owner == holder {
        return Role::Owner;
    }
    let lower = js_lower(holder);
    if o.everyone || o.shared.contains(&lower) {
        Role::Shared
    } else {
        Role::None
    }
}

/// `toLowerCase()`, which Rust's `to_lowercase` matches for every email a person types.
fn js_lower(s: &str) -> String {
    s.to_lowercase()
}

/// `cleanShares`: trimmed, lowercased, deduplicated, or the first entry that is not an email.
pub fn clean_shares(raw: &[String]) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for r in raw {
        let e = js_lower(js::trim(r));
        if e.is_empty() {
            continue;
        }
        if js::utf16_len(&e) > 254 || !is_email(&e) {
            return Err(r.clone());
        }
        if !out.contains(&e) {
            out.push(e);
        }
    }
    Ok(out)
}

/// `/^[^\s@]+@[^\s@]+$/`.
fn is_email(e: &str) -> bool {
    let mut parts = e.split('@');
    let (Some(a), Some(b), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    let plain = |s: &str| !s.is_empty() && !s.chars().any(js::is_space);
    plain(a) && plain(b)
}

pub struct Owners {
    file: Option<PathBuf>,
    map: IndexMap<String, Ownership>,
    log: Sink,
}

impl Owners {
    pub fn new(file: Option<PathBuf>, log: Sink, now_ms: i64) -> Self {
        let mut map = IndexMap::new();
        if let Some(f) = &file
            && let Some(Value::Object(o)) = load_json(f, "engine owners", &log, now_ms)
                .as_ref()
                .and_then(|r| r.get("engines"))
        {
            for (id, v) in o {
                let Some(owner) = v
                    .get("owner")
                    .and_then(Value::as_str)
                    .filter(|o| !o.is_empty())
                else {
                    continue;
                };
                let shared = v
                    .get("shared")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(Value::as_str)
                            .map(str::to_string)
                            .collect()
                    })
                    .unwrap_or_default();
                map.insert(
                    id.clone(),
                    Ownership {
                        owner: owner.to_string(),
                        shared,
                        everyone: v.get("everyone") == Some(&Value::Bool(true)),
                        since: v.get("since").and_then(Value::as_f64).unwrap_or(0.0),
                    },
                );
            }
        }
        Owners { file, map, log }
    }

    pub fn get(&self, id: &str) -> Option<&Ownership> {
        self.map.get(id)
    }

    pub fn assign(&mut self, id: &str, owner: &str, now_ms: i64) -> Assigned {
        let was = self.map.get(id).map(|o| o.owner.clone());
        if was.as_deref() == Some(owner) {
            return Assigned::Same;
        }
        let fresh = Ownership {
            owner: owner.to_string(),
            shared: Vec::new(),
            everyone: false,
            since: now_ms as f64,
        };
        // A JavaScript Map keeps a re-set key where it was.
        self.map.insert(id.to_string(), fresh);
        if self.map.len() > MAX_OWNED {
            let oldest = self
                .map
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| a.1.since.total_cmp(&b.1.since))
                .map(|(i, _)| i);
            if let Some(i) = oldest {
                self.map.shift_remove_index(i);
            }
        }
        (self.log)(&match &was {
            Some(w) => format!("engine {id} moved from {w} to {owner}, its heartbeat proving it"),
            None => format!("engine {id} is {owner}'s, its heartbeat proving it"),
        });
        self.save();
        if was.is_some() {
            Assigned::Moved
        } else {
            Assigned::New
        }
    }

    /// Replaces the share list (and the flag when given); false when the id has no owner.
    pub fn share(&mut self, id: &str, emails: &[String], everyone: Option<bool>) -> bool {
        let Some(o) = self.map.get_mut(id) else {
            return false;
        };
        o.shared = emails.iter().take(MAX_SHARED).cloned().collect();
        if let Some(e) = everyone {
            o.everyone = e;
        }
        self.save();
        true
    }

    pub fn release(&mut self, id: &str) -> bool {
        if self.map.shift_remove(id).is_none() {
            return false;
        }
        (self.log)(&format!("engine {id} has no owner any more"));
        self.save();
        true
    }

    pub fn all(&self) -> impl Iterator<Item = (&String, &Ownership)> {
        self.map.iter()
    }

    fn save(&self) {
        let Some(file) = &self.file else { return };
        let engines: Map<String, Value> = self
            .map
            .iter()
            .map(|(id, o)| (id.clone(), Value::Object(o.json())))
            .collect();
        save_json(
            file,
            &json!({ "version": 1, "engines": engines }),
            "engine owners",
            &self.log,
        );
    }
}
