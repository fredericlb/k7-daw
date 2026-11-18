//! list.rs — the browser's list: live engines, token stripped, each
//! with a LAN hint; another account's claims read as "in use" alone, so no email leaks to whoever lists.

use serde_json::{Map, Value, json};

use super::{Registry, State};
use crate::js;

/// The engine wire's oldest version a server serves (`MIN_ENGINE_PROTOCOL`).
pub const MIN_ENGINE_PROTOCOL: f64 = 1.0;

/// One engine as the list shows it, in `PublicEngine`'s key order.
pub struct Listed {
    pub id: String,
    pub hostname: String,
    pub same_source_ip: bool,
    pub score: Option<f64>,
    /// The object `/engines` spreads, every key in the TypeScript's order.
    pub json: Map<String, Value>,
}

impl Registry {
    /// The listing; `viewer` is the asking account, the only one whose claims it names.
    pub fn list(&self, requester_ip: Option<&str>, viewer: &str) -> Vec<Listed> {
        let now = self.clock.now_ms();
        let mut s: std::sync::MutexGuard<'_, State> = self.lock();
        let ids = s.live_ids(now);
        ids.iter()
            .map(|id| {
                let e = &s.engines[id];
                let holders: Vec<&super::Holder> = e.holders.values().collect();
                let claimed: Vec<&super::Holder> = holders.iter().copied().filter(|h| h.project.is_some()).collect();
                let mine = |h: &&super::Holder| !viewer.is_empty() && h.holder == viewer;
                let first = claimed
                    .iter()
                    .copied()
                    .find(|h| !h.project.as_deref().unwrap_or("").starts_with("@banc/"))
                    .or(claimed.first().copied())
                    .or(holders.first().copied());
                let owner = first.filter(|h| !viewer.is_empty() && h.holder == viewer);
                let protocol = e.protocol.unwrap_or(1.0);
                let mut o = Map::new();
                o.insert("id".into(), json!(e.id));
                o.insert("hostname".into(), json!(e.hostname));
                o.insert(
                    "addresses".into(),
                    json!({ "loopback": e.loopback, "lanIps": e.lan_ips, "port": js::num(e.port) }),
                );
                if let Some(p) = &e.performance {
                    o.insert("performance".into(), p.clone());
                }
                if let Some(l) = e.library_level {
                    o.insert("libraryLevel".into(), js::num(l));
                }
                o.insert("protocol".into(), js::num(protocol));
                if protocol < MIN_ENGINE_PROTOCOL {
                    o.insert("outdated".into(), json!(true));
                }
                if let Some(b) = &e.build {
                    o.insert("build".into(), json!(b));
                }
                if e.safe_mode {
                    o.insert("safeMode".into(), json!(true));
                }
                // The machine's own chains: what a cut uses when its caller names none (stemSplit.md).
                if let Some(c) = &e.chains {
                    let mut chains = Map::new();
                    if let Some(s) = &c.stems {
                        chains.insert("stems".into(), json!(s));
                    }
                    if let Some(s) = &c.instrumental {
                        chains.insert("instrumental".into(), json!(s));
                    }
                    o.insert("chains".into(), Value::Object(chains));
                }
                o.insert("assigned".into(), json!(!holders.is_empty()));
                if let Some(h) = owner {
                    o.insert("heldBy".into(), json!(h.holder));
                    if let Some(p) = &h.project {
                        o.insert("heldProject".into(), json!(p));
                    }
                }
                let held: Vec<Value> = claimed
                    .iter()
                    .filter(|h| mine(h))
                    .map(|h| json!({ "holder": h.holder, "project": h.project.clone().unwrap_or_default() }))
                    .collect();
                o.insert("heldProjects".into(), Value::Array(held));
                if e.legacy {
                    o.insert("legacy".into(), json!(true));
                }
                let same = requester_ip.is_some_and(|r| !r.is_empty() && e.public_ip_hint.as_deref() == Some(r));
                o.insert("sameSourceIp".into(), json!(same));
                Listed {
                    id: e.id.clone(),
                    hostname: e.hostname.clone(),
                    same_source_ip: same,
                    score: e.performance.as_ref().and_then(|p| p.get("score")).and_then(Value::as_f64),
                    json: o,
                }
            })
            .collect()
    }
}
