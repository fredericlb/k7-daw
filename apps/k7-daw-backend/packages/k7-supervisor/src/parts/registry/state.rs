//! state.rs — the registry part's own state and what it does with it: the heartbeat, its log policy, the
//! presence it carries, the snapshot slice, and the leaving. The commands that change the list are in `commands.rs`.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use tokio::sync::mpsc;

use super::{Done, Reply, bounded};
use crate::addresses;
use crate::core::Ctx;
use crate::protocol::{Instance, InstanceCheck};
use crate::registry::{self, Beat, Credentials, Outcome};
use crate::urls;

/// The library level a heartbeat advertises.
const LIBRARY_LEVEL: i64 = 2;

/// Per registry: how many heartbeats in a row it refused and why, what `.studio.log` says and when.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BeatTrack {
    pub failures: u32,
    pub last_detail: String,
    pub ever_ok: bool,
}

pub struct State {
    pub cx: Ctx,
    /// The registries as resolved: the user's list, else the environment's.
    pub urls: Vec<String>,
    /// The settings' list and tokens as this part last wrote them, which the core's copy follows.
    pub pinned: Vec<String>,
    pub tokens: BTreeMap<String, String>,
    pub performance: Option<Value>,
    pub safe: bool,
    pub light: bool,
    pub check: Option<InstanceCheck>,
    pub beating: bool,
    pub said_no_registry: bool,
    pub tracks: BTreeMap<String, BeatTrack>,
}

/// A studio's family key, which pl8's registry shares.
pub fn instance_key(u: &str) -> String {
    let k = urls::studio_family(u);
    if k.is_empty() { u.to_string() } else { k }
}

impl State {
    pub fn new(cx: Ctx) -> State {
        State {
            cx,
            urls: Vec::new(),
            pinned: Vec::new(),
            tokens: BTreeMap::new(),
            performance: None,
            safe: false,
            light: false,
            check: None,
            beating: false,
            said_no_registry: false,
            tracks: BTreeMap::new(),
        }
    }

    /// The settings read once, the paired studios listed once, the registries resolved and their origins let in.
    pub fn load(&mut self) {
        let mut s = self.cx.settings.borrow().clone();
        if list_paired_studios(&mut s) {
            let (pinned, listed) = (s.registry_urls.clone(), s.pairings_listed);
            self.cx.update_settings(move |c| {
                c.registry_urls = pinned;
                c.pairings_listed = listed;
            });
        }
        self.pinned = s.registry_urls;
        self.tokens = s.device_tokens;
        self.urls = registry::resolve(&self.pinned);
        self.cx
            .set_origins(crate::engine::allowed_origins(&self.urls));
        self.publish();
    }

    pub fn up(&self) -> bool {
        self.cx.engine.borrow().up
    }

    /// A row per studio family; one from `$STUDIO_REGISTRY_URL` is not removable, the environment not being ours.
    fn instances(&self) -> Vec<Instance> {
        let pinned: Vec<String> = self.pinned.iter().map(|u| instance_key(u)).collect();
        let mut rows: Vec<Instance> = Vec::new();
        for u in &self.urls {
            let key = instance_key(u);
            if rows.iter().any(|r| r.url == key) {
                continue;
            }
            rows.push(Instance {
                removable: pinned.contains(&key),
                url: key,
            });
        }
        rows
    }

    /// This part's slice of the snapshot, as it is now.
    pub fn publish(&self) {
        let (joined, rows) = (self.urls.join(", "), self.instances());
        let (check, light) = (self.check.clone(), self.light);
        self.cx.patch(false, move |s| {
            s.registry_url.clone_from(&joined);
            s.instances.clone_from(&rows);
            s.instance_check.clone_from(&check);
            s.last_heartbeat_ok = light;
        });
    }

    /// What every heartbeat carries.
    pub fn presence(&self) -> Value {
        let engine = *self.cx.engine.borrow();
        let port = if engine.port > 0 {
            engine.port
        } else {
            self.cx.settings.borrow().port
        };
        let mut p = addresses::presence(port);
        if let Some(perf) = &self.performance {
            p["performance"] = perf.clone();
        }
        p["libraryLevel"] = json!(LIBRARY_LEVEL);
        for (k, v) in registry::presence_extras() {
            p[k] = v;
        }
        if self.safe {
            p["safeMode"] = json!(true);
        }
        // This machine's own chain per `want`: what a cut uses when its caller names none (stemSplit.md).
        let s = self.cx.settings.borrow();
        let mut chains = serde_json::Map::new();
        if !s.stems_chain.is_empty() {
            chains.insert("stems".into(), json!(s.stems_chain));
        }
        if !s.instrumental_chain.is_empty() {
            chains.insert("instrumental".into(), json!(s.instrumental_chain));
        }
        if !chains.is_empty() {
            p["chains"] = Value::Object(chains);
        }
        p
    }

    fn credentials(&self) -> Credentials {
        let env = std::env::var("STUDIO_DEVICE_TOKEN").unwrap_or_default();
        Credentials {
            device_tokens: self.tokens.clone(),
            // The environment's token goes only to the environment's registries: a studio added by hand has its own.
            env_token: if self.pinned.is_empty() {
                env
            } else {
                String::new()
            },
        }
    }

    /// One heartbeat, unless the engine is down, no studio is listed or one is already in flight; `reply` goes out after it, or at once.
    pub fn beat(&mut self, dtx: &mpsc::UnboundedSender<Done>, reply: Option<Reply>) {
        let skip = |st: &mut State, reply: Option<Reply>| {
            if let Some(r) = reply {
                st.cx.reply(r.id, r.ok, &r.message);
            }
        };
        if !self.up() {
            self.light = false;
            self.publish();
            return skip(self, reply);
        }
        if self.urls.is_empty() {
            // Said once: an engine no studio lists is a setup question, and `registryUrls` is the field for it.
            if !self.said_no_registry {
                crate::log::host("no registry configured — this engine is listed at no studio");
            }
            self.said_no_registry = true;
            self.light = false;
            self.publish();
            return skip(self, reply);
        }
        self.said_no_registry = false;
        // One in flight at a time: a dead network stacked a beat every 5 s, each holding its sockets, until none were left.
        if self.beating {
            return skip(self, reply);
        }
        self.beating = true;
        let (list, id, presence, cred) = (
            self.urls.clone(),
            self.cx.identity.clone(),
            self.presence(),
            self.credentials(),
        );
        let dtx = dtx.clone();
        tokio::spawn(async move {
            let beat = registry::heartbeat(&list, &id, presence, &cred).await;
            let _ = dtx.send(Done::Beat(beat, reply));
        });
    }

    pub fn beat_done(&mut self, beat: Beat, reply: Option<Reply>) {
        self.beating = false;
        for o in &beat.outcomes {
            self.note_beat(o);
        }
        // An engine stopped while it flew is not listed by a late answer.
        self.light = beat.ok() && self.up();
        self.publish();
        if let Some(r) = reply {
            self.cx.reply(r.id, r.ok, &r.message);
        }
    }

    /// One registry's answer, into `.studio.log`: its first refusal, each NEW reason, a reminder a minute, the recovery.
    fn note_beat(&mut self, o: &Outcome) {
        let t = self.tracks.entry(o.url.clone()).or_default();
        if o.ok {
            if t.failures > 0 {
                crate::log::line(
                    "registry",
                    &format!(
                        "heartbeat to {} accepted again after {} failure(s)",
                        o.url, t.failures
                    ),
                );
            } else if !t.ever_ok {
                crate::log::line(
                    "registry",
                    &format!("listed at {} (heartbeat accepted)", o.url),
                );
            }
            *t = BeatTrack {
                failures: 0,
                last_detail: String::new(),
                ever_ok: true,
            };
            return;
        }
        t.failures += 1;
        // A minute of heartbeats between reminders: a registry down for an hour is sixty lines, not seven hundred.
        let per_minute = (60_000 / super::HEARTBEAT.as_millis()) as u32;
        if t.failures == 1 || o.detail != t.last_detail {
            crate::log::line(
                "registry",
                &format!(
                    "heartbeat to {} failed: {} — until it is accepted, browsers cannot select this engine",
                    o.url, o.detail
                ),
            );
        } else if t.failures.is_multiple_of(per_minute) {
            crate::log::line(
                "registry",
                &format!(
                    "heartbeat to {} still failing ({} in a row): {}",
                    o.url, t.failures, o.detail
                ),
            );
        }
        t.last_detail.clone_from(&o.detail);
    }

    /// Leave `list`, logging each that could not be left.
    pub async fn leave(&mut self, list: Vec<String>) {
        let beat = registry::deregister(&list, &self.cx.identity).await;
        for o in &beat.outcomes {
            self.tracks.remove(&o.url);
            if !o.ok {
                crate::log::line(
                    "registry",
                    &format!(
                        "could not leave {}: {} — it drops this engine once its heartbeats stop",
                        o.url, o.detail
                    ),
                );
            }
        }
    }

    /// Every registry, bounded: what a stop and a quit wait on.
    pub async fn leave_all(&mut self) {
        let list = self.urls.clone();
        bounded(self.leave(list)).await;
    }
}

/// Once: a studio this machine holds a token for, from before a pairing listed the engine, is listed now.
pub fn list_paired_studios(s: &mut crate::settings::Settings) -> bool {
    if s.pairings_listed {
        return false;
    }
    s.pairings_listed = true;
    // An empty list is `$STUDIO_REGISTRY_URL`'s: one entry added would replace that environment's studio.
    if s.registry_urls.is_empty() {
        return true;
    }
    for studio in s.device_tokens.keys().cloned().collect::<Vec<_>>() {
        let reg = urls::engine_registry_url(&studio);
        if reg.is_empty() {
            continue;
        }
        let family = urls::studio_family(&reg);
        if !s
            .registry_urls
            .iter()
            .any(|u| urls::studio_family(u) == family)
        {
            s.registry_urls.push(reg);
        }
    }
    true
}
