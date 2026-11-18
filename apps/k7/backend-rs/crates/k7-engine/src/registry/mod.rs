//! registry — the engines a heartbeat listed, the CLAIMS saying which project each drives for whom, and the
//! two stores that outlive a listing (the secret bindings, the owners). The
//! claim's three rules are `apps/k7/backend-rs/docs/engine/claim.md`; the token proves nothing (`token.md` there).

pub mod bindings;
mod claims;
pub mod heartbeat;
mod list;
pub mod owners;
pub mod state_file;
pub mod ticket;

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use indexmap::IndexMap;
use serde_json::Value;

pub use claims::{ClaimRequest, ClaimResult, Lost};
pub use list::Listed;

use crate::clock::Clock;
use crate::log::Sink;
use bindings::{Bindings, Verdict};
use owners::{Assigned, Owners, Ownership, Role, role_of};

/// No heartbeat for this long and the engine is presumed gone; one beats every ~5 s.
pub const REGISTRY_TTL_MS: i64 = 15_000;
/// A holder unheard of for this long is forgotten: a tab cannot be relied on to say goodbye.
pub const ENGINE_ASSIGN_TTL_MS: i64 = 90_000;
/// At most this many engines listed at once: the heartbeat is open, so the list must not be.
const MAX_ENGINES: usize = 1000;
/// At most this many ids NEW to the list from one source address in a minute.
const NEW_IDS_PER_IP_PER_MINUTE: u32 = 30;

/// One tab holding one engine.
#[derive(Clone, Debug)]
pub struct Holder {
    /// `<repo>/<projet>`, or None for a surface that opens no project.
    pub project: Option<String>,
    pub holder: String,
    pub seen: i64,
}

/// One listed engine; the token never leaves the server.
#[derive(Clone, Debug)]
pub struct Entry {
    pub id: String,
    pub token: String,
    pub hostname: String,
    pub machine_fingerprint: String,
    pub loopback: String,
    pub lan_ips: Vec<String>,
    pub port: f64,
    pub performance: Option<Value>,
    pub library_level: Option<f64>,
    pub protocol: Option<f64>,
    pub build: Option<String>,
    pub safe_mode: bool,
    /// The chains the MACHINE separates with, by `want` — its own settings, carried by its heartbeat.
    pub chains: Option<heartbeat::Chains>,
    /// The source address the heartbeat came from.
    pub public_ip_hint: Option<String>,
    /// Its last heartbeat from loopback.
    pub loopback_seen: Option<i64>,
    /// Its supervisor sends no tunnel secret.
    pub legacy: bool,
    /// Live holders by their secret, in the order they claimed.
    pub holders: IndexMap<String, Holder>,
    pub seen: i64,
}

/// What a heartbeat carries into the registry.
pub struct HeartbeatInput {
    pub beat: heartbeat::ReadHeartbeat,
    pub public_ip_hint: Option<String>,
    /// The heartbeat came from this machine: its client address is loopback, which only a loopback socket makes it.
    pub from_loopback: bool,
}

/// Why a heartbeat was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    /// Another token holds the id.
    Token,
    /// The id is bound to another secret.
    Secret,
    /// The list is full.
    Full,
    /// Too many new ids from one address in a minute.
    Rate,
}

/// `isLoopbackAddress`: `::1` or `127.x.x.x`, `::ffff:` aside.
pub fn is_loopback_address(a: Option<&str>) -> bool {
    let Some(a) = a.filter(|a| !a.is_empty()) else {
        return false;
    };
    let a = strip_mapped(a);
    a == "::1" || {
        let p: Vec<&str> = a.split('.').collect();
        p.len() == 4
            && p[0] == "127"
            && p.iter()
                .all(|x| (1..=3).contains(&x.len()) && x.bytes().all(|b| b.is_ascii_digit()))
    }
}

/// An address without its `::ffff:` prefix, any case.
fn strip_mapped(a: &str) -> &str {
    if a.len() >= 7 && a[..7].eq_ignore_ascii_case("::ffff:") {
        &a[7..]
    } else {
        a
    }
}

/// Whether the engine runs on this backend's machine: a loopback heartbeat within the TTL.
pub fn on_this_machine(e: &Entry, now: i64) -> bool {
    e.loopback_seen.is_some_and(|s| now - s < REGISTRY_TTL_MS)
}

/// `hostsFor`: where the SERVER may dial an engine, the address its heartbeat came FROM and never one it names.
pub fn hosts_for(public_ip_hint: Option<&str>, local: bool) -> Vec<String> {
    if local {
        return vec!["127.0.0.1".into()];
    }
    let hint = public_ip_hint.unwrap_or("");
    let ip = hint.strip_prefix("::ffff:").unwrap_or(hint);
    // Loopback is proven by the socket alone (`local`): a hint naming it came from a header.
    if is_loopback_address(Some(ip)) {
        return Vec::new();
    }
    let parts: Vec<&str> = ip.split('.').collect();
    let v4 = parts.len() == 4
        && parts
            .iter()
            .all(|p| (1..=3).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_digit()));
    if v4 { vec![ip.to_string()] } else { Vec::new() }
}

pub(crate) struct State {
    engines: IndexMap<String, Entry>,
    arrivals: IndexMap<String, (i64, u32)>,
    pub(crate) bindings: Bindings,
    pub(crate) owners: Owners,
    max_engines: usize,
    per_ip: u32,
}

/// The registry: one lock over the listing, the claims, the bindings and the owners, never held across an await.
pub struct Registry {
    state: Mutex<State>,
    pub clock: Clock,
}

/// Where the registry keeps what outlives a listing, and its limits.
#[derive(Default)]
pub struct Options {
    /// The host's state dir; None keeps the bindings and owners in memory.
    pub state_dir: Option<std::path::PathBuf>,
    pub max_engines: Option<usize>,
    pub new_ids_per_ip_per_minute: Option<u32>,
}

impl Registry {
    pub fn new(opts: Options, log: Sink, clock: Clock) -> Self {
        let now = clock.now_ms();
        let dir = opts.state_dir.as_deref();
        let file = |name: &str| dir.map(|d: &Path| d.join(name));
        Registry {
            state: Mutex::new(State {
                engines: IndexMap::new(),
                arrivals: IndexMap::new(),
                bindings: Bindings::new(file(bindings::BINDINGS_FILE), log.clone(), now),
                owners: Owners::new(file(owners::OWNERS_FILE), log, now),
                max_engines: opts.max_engines.unwrap_or(MAX_ENGINES),
                per_ip: opts
                    .new_ids_per_ip_per_minute
                    .unwrap_or(NEW_IDS_PER_IP_PER_MINUTE),
            }),
            clock,
        }
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Records a heartbeat under the bindings' rule, pinning an unbound id's token on first sight.
    pub fn upsert(&self, input: HeartbeatInput) -> Result<(), Refused> {
        let now = self.clock.now_ms();
        let mut s = self.lock();
        let b = input.beat;
        let existing = s.get(&b.id, now).cloned();
        let verdict = s.bindings.check(&b.id, b.tunnel_secret.as_deref());
        if verdict == Verdict::Refused {
            return Err(Refused::Secret);
        }
        // The secret proves the machine, so its token may change; without one the token first seen holds the id.
        if verdict != Verdict::Bound && existing.as_ref().is_some_and(|e| e.token != b.token) {
            return Err(Refused::Token);
        }
        if existing.is_none() {
            if s.live(now) >= s.max_engines {
                return Err(Refused::Full);
            }
            if !s.admit(input.public_ip_hint.as_deref(), now) {
                return Err(Refused::Rate);
            }
        }
        if let Some(secret) = &b.tunnel_secret {
            s.bindings.bind(&b.id, secret, now);
        }
        let loopback_seen = if input.from_loopback {
            Some(now)
        } else {
            existing.as_ref().and_then(|e| e.loopback_seen)
        };
        let entry = Entry {
            id: b.id.clone(),
            token: b.token,
            hostname: b.hostname,
            machine_fingerprint: b.machine_fingerprint,
            loopback: b.loopback,
            lan_ips: b.lan_ips,
            port: b.port,
            performance: b.performance,
            library_level: b.library_level,
            protocol: b.protocol,
            build: b.build,
            safe_mode: b.safe_mode,
            chains: b.chains,
            public_ip_hint: input.public_ip_hint,
            loopback_seen,
            legacy: b.tunnel_secret.is_none(),
            holders: existing.map(|e| e.holders).unwrap_or_default(),
            seen: now,
        };
        s.engines.insert(b.id, entry);
        Ok(())
    }

    /// A live entry, a copy: a lapsed one is dropped here.
    pub fn get(&self, id: &str) -> Option<Entry> {
        let now = self.clock.now_ms();
        self.lock().get(id, now).cloned()
    }

    pub fn on_this_machine(&self, id: &str) -> bool {
        let now = self.clock.now_ms();
        self.lock()
            .get(id, now)
            .is_some_and(|e| on_this_machine(e, now))
    }

    /// The anti-spoof gate: does this id exist and match this token.
    pub fn verify_token(&self, id: &str, token: &str) -> bool {
        self.get(id).is_some_and(|e| e.token == token)
    }

    /// Whether a TUNNEL for `id` may open: its token, and its secret when the id is bound to one.
    pub fn verify_tunnel(&self, id: &str, token: &str, secret: Option<&str>) -> bool {
        let now = self.clock.now_ms();
        let mut s = self.lock();
        let known = s.get(id, now).is_some_and(|e| e.token == token);
        known && s.bindings.check(id, secret) != Verdict::Refused
    }

    /// Deregisters; a bound id needs its secret too, so the token alone cannot unlist it.
    pub fn remove(&self, id: &str, token: &str, secret: Option<&str>) -> bool {
        let mut s = self.lock();
        let Some(e) = s.engines.get(id) else {
            return true;
        };
        if e.token != token || s.bindings.check(id, secret) == Verdict::Refused {
            return false;
        }
        s.engines.shift_remove(id);
        true
    }

    /// `engineRole`: what `holder` is to engine `id`; an engine on this machine is every signed-in account's.
    pub fn role(&self, id: &str, holder: &str, server: bool) -> Role {
        let now = self.clock.now_ms();
        let mut s = self.lock();
        if !holder.is_empty() && s.get(id, now).is_some_and(|e| on_this_machine(e, now)) {
            return Role::Owner;
        }
        role_of(s.owners.get(id), holder, server)
    }

    pub fn owner_of(&self, id: &str) -> Option<Ownership> {
        self.lock().owners.get(id).cloned()
    }

    pub fn assign_owner(&self, id: &str, owner: &str) -> Assigned {
        let now = self.clock.now_ms();
        self.lock().owners.assign(id, owner, now)
    }

    pub fn share(&self, id: &str, emails: &[String], everyone: Option<bool>) -> bool {
        self.lock().owners.share(id, emails, everyone)
    }

    pub fn release_owner(&self, id: &str) -> bool {
        self.lock().owners.release(id)
    }

    /// Every ownership record, with its machine's hostname when listed.
    pub fn owners(&self) -> Vec<(String, Ownership, Option<String>)> {
        let now = self.clock.now_ms();
        let mut s = self.lock();
        let all: Vec<(String, Ownership)> = s
            .owners
            .all()
            .map(|(i, o)| (i.clone(), o.clone()))
            .collect();
        all.into_iter()
            .map(|(id, o)| {
                let host = s.get(&id, now).map(|e| e.hostname.clone());
                (id, o, host)
            })
            .collect()
    }

    /// The bindings' verdict for a secret, for the testbed.
    #[cfg(feature = "test-hooks")]
    pub fn binding(&self, id: &str, secret: Option<&str>) -> &'static str {
        match self.lock().bindings.check(id, secret) {
            Verdict::Bound => "bound",
            Verdict::Fresh => "fresh",
            Verdict::Legacy => "legacy",
            Verdict::Refused => "refused",
        }
    }
}

impl State {
    /// The entry when its heartbeat is recent; a lapsed one is dropped.
    fn get(&mut self, id: &str, now: i64) -> Option<&mut Entry> {
        if self
            .engines
            .get(id)
            .is_some_and(|e| now - e.seen >= REGISTRY_TTL_MS)
        {
            self.engines.shift_remove(id);
        }
        self.engines.get_mut(id)
    }

    /// Drops the lapsed entries, and counts the rest.
    fn live(&mut self, now: i64) -> usize {
        self.engines.retain(|_, e| now - e.seen < REGISTRY_TTL_MS);
        self.engines.len()
    }

    /// Whether `ip` may add one more id now; counted when so.
    fn admit(&mut self, ip: Option<&str>, now: i64) -> bool {
        let key = ip.unwrap_or("?").to_string();
        let fresh = self
            .arrivals
            .get(&key)
            .is_none_or(|(since, _)| now - since >= 60_000);
        if fresh {
            // Pruned whole when it grows: a minute-old window is worth nothing.
            if self.arrivals.len() > 10_000 {
                self.arrivals.clear();
            }
            self.arrivals.insert(key.clone(), (now, 0));
        }
        let a = self.arrivals.get_mut(&key).expect("just inserted");
        if a.1 >= self.per_ip {
            return false;
        }
        a.1 += 1;
        true
    }
}

#[cfg(test)]
mod tests;
