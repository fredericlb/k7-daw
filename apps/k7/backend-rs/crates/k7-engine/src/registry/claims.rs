//! claims.rs — taking an engine and keeping it: an OPEN (no secret) always
//! wins and displaces under the rules; a RENEWAL (its secret) holds while that secret is registered, and
//! says which of three ways it went otherwise (`apps/k7/backend-rs/docs/engine/claim.md`).

use super::owners::{Role, role_of};
use super::{ENGINE_ASSIGN_TTL_MS, Entry, Holder, REGISTRY_TTL_MS, Registry, State};

/// An open or a renewal.
pub struct ClaimRequest {
    /// `<repo>/<projet>`, or None for a surface that opens no project.
    pub project: Option<String>,
    pub holder: String,
    /// Absent on an open, present on every renewal.
    pub secret: Option<String>,
}

/// Why a renewal was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lost {
    Project,
    Engine,
    Lapsed,
}

impl Lost {
    pub fn as_str(self) -> &'static str {
        match self {
            Lost::Project => "project",
            Lost::Engine => "engine",
            Lost::Lapsed => "lapsed",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ClaimResult {
    Ok {
        secret: String,
        /// Whom this open displaced, and from which project.
        took_over: Option<(String, Option<String>)>,
    },
    Lost {
        lost: Lost,
        held_by: String,
        /// For `engine`, the project that took the machine; `Some(None)` is a holder with no project.
        took_for: Option<Option<String>>,
    },
}

/// `claimSlot`: the bench has a session of its own beside the song's.
fn is_bench(project: &str) -> bool {
    project.starts_with("@banc/")
}

/// JavaScript's truthiness of an optional project.
fn truthy(p: &Option<String>) -> bool {
    p.as_deref().is_some_and(|p| !p.is_empty())
}

/// Forgets the holders whose renewal lapsed.
fn prune(e: &mut Entry, now: i64) {
    e.holders.retain(|_, h| now - h.seen < ENGINE_ASSIGN_TTL_MS);
}

/// A holder on `e` whose project is not `project` and would share its session; rule 2's.
fn other_project_on(e: &Entry, project: &str, holder: &str) -> Option<String> {
    e.holders.iter().find_map(|(secret, h)| {
        let p = h.project.as_deref()?;
        let other_session = h.holder == holder && is_bench(p) != is_bench(project);
        (p != project && !other_session).then(|| secret.clone())
    })
}

fn random_secret() -> String {
    let mut b = [0u8; 18];
    getrandom::fill(&mut b).expect("the system's random source");
    b.iter().map(|x| format!("{x:02x}")).collect()
}

impl State {
    /// Every live engine's id, pruned of its lapsed holders.
    pub(super) fn live_ids(&mut self, now: i64) -> Vec<String> {
        let ids: Vec<String> = self
            .engines
            .values()
            .filter(|e| now - e.seen < REGISTRY_TTL_MS)
            .map(|e| e.id.clone())
            .collect();
        for id in &ids {
            if let Some(e) = self.engines.get_mut(id) {
                prune(e, now);
            }
        }
        ids
    }
}

impl Registry {
    /// Takes the engine, or renews a hold already taken.
    pub fn claim(&self, id: &str, req: ClaimRequest) -> ClaimResult {
        let now = self.clock.now_ms();
        let mut s = self.lock();
        let lapsed = ClaimResult::Lost {
            lost: Lost::Lapsed,
            held_by: String::new(),
            took_for: None,
        };
        let Some(e) = s.get(id, now) else {
            return lapsed;
        };
        prune(e, now);
        // The same instance: this engine, this account — one account on one machine being one session.
        let same = |eid: &str, h: &Holder| eid == id && h.holder == req.holder;

        if let Some(secret) = &req.secret {
            if let Some(mine) = e.holders.get_mut(secret) {
                mine.seen = now;
                mine.project = req.project.clone();
                return ClaimResult::Ok {
                    secret: secret.clone(),
                    took_over: None,
                };
            }
            if truthy(&req.project) {
                let want = req.project.as_deref();
                for eid in s.live_ids(now) {
                    let x = &s.engines[&eid];
                    if let Some(h) = x
                        .holders
                        .values()
                        .find(|h| h.project.as_deref() == want && !same(&eid, h))
                    {
                        return ClaimResult::Lost {
                            lost: Lost::Project,
                            held_by: h.holder.clone(),
                            took_for: None,
                        };
                    }
                }
                let e = &s.engines[id];
                if let Some(k) = other_project_on(e, want.unwrap_or(""), &req.holder) {
                    let h = &e.holders[&k];
                    return ClaimResult::Lost {
                        lost: Lost::Engine,
                        held_by: h.holder.clone(),
                        took_for: Some(h.project.clone()),
                    };
                }
            }
            return lapsed;
        }

        let mut took_over = None;
        if let Some(project) = &req.project {
            // Rule 1: the same project wherever it is open, except our own other screen on this very engine.
            for eid in s.live_ids(now) {
                let x = s.engines.get_mut(&eid).expect("live");
                let gone: Vec<String> = x
                    .holders
                    .iter()
                    .filter(|(_, h)| h.project.as_deref() == Some(project) && !same(&eid, h))
                    .map(|(k, _)| k.clone())
                    .collect();
                for k in gone {
                    if let Some(h) = x.holders.shift_remove(&k) {
                        took_over = Some((h.holder, h.project));
                    }
                }
            }
            // Rule 2: a DIFFERENT project already on this engine.
            let e = s.engines.get_mut(id).expect("live");
            if let Some(k) = other_project_on(e, project, &req.holder)
                && let Some(h) = e.holders.shift_remove(&k)
            {
                took_over = Some((h.holder, h.project));
            }
        }
        let secret = random_secret();
        let e = s.engines.get_mut(id).expect("live");
        e.holders.insert(
            secret.clone(),
            Holder {
                project: req.project,
                holder: req.holder,
                seen: now,
            },
        );
        ClaimResult::Ok { secret, took_over }
    }

    /// Lets go, by the claim's own secret, so a goodbye drops only the hold it has.
    pub fn release(&self, id: &str, secret: &str) {
        let now = self.clock.now_ms();
        if let Some(e) = self.lock().get(id, now) {
            e.holders.shift_remove(secret);
        }
    }

    /// Whether account `holder` holds a live claim on engine `id`.
    pub fn holds(&self, id: &str, holder: &str) -> bool {
        let now = self.clock.now_ms();
        let mut s = self.lock();
        let Some(e) = s.get(id, now) else {
            return false;
        };
        if holder.is_empty() {
            return false;
        }
        prune(e, now);
        e.holders.values().any(|h| h.holder == holder)
    }

    /// Drops every claim on `id` whose account `keep` refuses.
    pub fn evict(&self, id: &str, keep: impl Fn(&str) -> bool) {
        let now = self.clock.now_ms();
        if let Some(e) = self.lock().get(id, now) {
            e.holders.retain(|_, h| keep(&h.holder));
        }
    }

    /// Drops every claim on `id` of an account that may no longer use it: an owner moved.
    pub fn evict_unusable(&self, id: &str, server: bool) {
        let now = self.clock.now_ms();
        let mut s = self.lock();
        let owner = s.owners.get(id).cloned();
        let Some(e) = s.get(id, now) else { return };
        let local = super::on_this_machine(e, now);
        e.holders.retain(|_, h| {
            let role = if !h.holder.is_empty() && local {
                Role::Owner
            } else {
                role_of(owner.as_ref(), &h.holder, server)
            };
            role.may_use()
        });
    }

    /// Drops every claim on `id` of an account its owner no longer shares it with.
    pub fn evict_unshared(&self, id: &str) {
        let now = self.clock.now_ms();
        let mut s = self.lock();
        let owner = s.owners.get(id).cloned();
        if let Some(e) = s.get(id, now) {
            e.holders
                .retain(|_, h| role_of(owner.as_ref(), &h.holder, false) != Role::None);
        }
    }

    /// Which engine drives this project now.
    pub fn engine_for_project(&self, project: &str) -> Option<String> {
        let now = self.clock.now_ms();
        let mut s = self.lock();
        s.live_ids(now).into_iter().find(|id| {
            s.engines[id]
                .holders
                .values()
                .any(|h| h.project.as_deref() == Some(project))
        })
    }
}
