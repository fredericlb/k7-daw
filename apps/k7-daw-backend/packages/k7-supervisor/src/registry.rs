//! registry — presence at the studios this engine is connected to: a heartbeat every few seconds so a server
//! can list it, a deregister so Stop drops it at once, and the hello that proves an address IS a registry
//! before anything carrying the engine's token goes there. The hello's path and answer are
//! `REGISTRY_HELLO_PATH` and `REGISTRY_SERVICE` in `web-protocol/services.ts`; the frames are
//! `apps/k7-daw-backend/docs/engine/heartbeat.md`'s. What a reader gets wrong: a body's keys go out sorted,
//! since the wire is held byte for byte.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::Duration;

use serde_json::{Map, Value};

use crate::http::{self, Request, Response};
use crate::identity::Identity;
use crate::urls;

const HELLO_PATH: &str = "/api/registry/hello";
const SERVICE: &str = "k7-registry";

/// What a heartbeat carries beside the measure: `protocol`, `build`, `safeMode`, set by the part that knows them.
static EXTRAS: Mutex<BTreeMap<String, Value>> = Mutex::new(BTreeMap::new());

/// Say (or, with None, unsay) one more field of every heartbeat's presence.
pub fn set_presence_extra(key: &str, value: Option<Value>) {
    let mut extras = EXTRAS.lock().unwrap_or_else(|e| e.into_inner());
    match value {
        Some(v) => extras.insert(key.into(), v),
        None => extras.remove(key),
    };
}

/// The fields `set_presence_extra` holds, to lay over a presence.
pub fn presence_extras() -> BTreeMap<String, Value> {
    EXTRAS.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

fn as_registries(list: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for u in list {
        let r = urls::engine_registry_url(u);
        if r.is_empty() || out.contains(&r) {
            continue;
        }
        let why = urls::plaintext_refusal(&r);
        if !why.is_empty() {
            crate::log::line("registry", &why);
            continue;
        }
        out.push(r);
    }
    out
}

/// What the user added, else `$STUDIO_REGISTRY_URL`, else nothing: each as its family's registry, deduplicated.
pub fn resolve(configured: &[String]) -> Vec<String> {
    if !configured.is_empty() {
        return as_registries(configured);
    }
    let env = std::env::var("STUDIO_REGISTRY_URL").unwrap_or_default();
    if env.is_empty() {
        return Vec::new();
    }
    as_registries(&env.split(',').map(str::to_string).collect::<Vec<_>>())
}

#[derive(Debug, Clone, PartialEq)]
pub struct Probe {
    pub ok: bool,
    pub why: String,
}

fn no(why: String) -> Probe {
    Probe { ok: false, why }
}

/// Longer than a heartbeat's bound: somebody waits on the form, and a slow first handshake is not a "no".
const PROBE_BOUND: Duration = Duration::from_secs(5);
/// What a heartbeat or a deregister waits for each of connect and answer.
const BEAT_BOUND: Duration = Duration::from_secs(2);

/// Ask `base` whether it IS an engine registry; a redirect is refused, not followed.
pub async fn probe(base: &str) -> Probe {
    probe_within(base, PROBE_BOUND).await
}

/// `probe` with its own bound, for a test that waits on a hang.
pub async fn probe_within(base: &str, bound: Duration) -> Probe {
    let why = urls::plaintext_refusal(base);
    if !why.is_empty() {
        return no(why);
    }
    let mut r = Request::get(urls::join(base, HELLO_PATH));
    r.timeout = bound;
    let res = http::send(r).await;
    // The cause rides along: a refused certificate and a missing host must not read alike.
    if res.status == 0 {
        return no(format!("{base} did not answer ({})", res.error));
    }
    if (300..400).contains(&res.status) {
        return no(format!("{base} did not answer (it redirects)"));
    }
    if res.ok()
        && let Ok(body) = serde_json::from_str::<Value>(&res.body)
        && body.get("service").and_then(Value::as_str) == Some(SERVICE)
    {
        return Probe {
            ok: true,
            why: String::new(),
        };
    }
    no(format!("{base} is not an k7 instance"))
}

/// What one registry answered: its status (0 when nothing did) and, when it refused, why in one line.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    pub url: String,
    pub ok: bool,
    pub status: u16,
    pub detail: String,
}

/// Every registry's outcome; `ok()` only when they ALL answered, since the snapshot shows one light.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Beat {
    pub outcomes: Vec<Outcome>,
}

impl Beat {
    pub fn ok(&self) -> bool {
        !self.outcomes.is_empty() && self.outcomes.iter().all(|o| o.ok)
    }
}

/// What proves this machine to a studio that lists paired machines only (`docs/engine/heartbeat.md`).
#[derive(Debug, Clone, Default)]
pub struct Credentials {
    /// By registry URL, as the settings keep them.
    pub device_tokens: BTreeMap<String, String>,
    /// `$STUDIO_DEVICE_TOKEN`: sent only when the registries themselves come from the environment.
    pub env_token: String,
}

/// What one registry answered, cut to a line: a 400's body names the field it refused.
fn detail_of(res: &Response) -> String {
    if res.status == 0 {
        let why = if res.error.is_empty() {
            "timed out"
        } else {
            &res.error
        };
        return format!("no answer ({why})");
    }
    let body: String = res
        .body
        .chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .collect();
    let body = if body.chars().count() > 300 {
        format!("{}…", body.chars().take(300).collect::<String>())
    } else {
        body
    };
    let tail = if body.is_empty() {
        String::new()
    } else {
        format!(" {body}")
    };
    format!("HTTP {}{tail}", res.status)
}

/// The same value with every object's keys sorted, which is the order the wire is held to.
fn sorted(v: Value) -> Value {
    match v {
        Value::Object(m) => {
            let mut keys: Vec<String> = m.keys().cloned().collect();
            keys.sort();
            let mut out = Map::new();
            for k in keys {
                let item = sorted(m[&k].clone());
                out.insert(k, item);
            }
            Value::Object(out)
        }
        Value::Array(a) => Value::Array(a.into_iter().map(sorted).collect()),
        other => other,
    }
}

/// The headers one registry gets: ITS device token only, so a token for one studio never reaches another.
fn headers_for(registry: &str, cred: &Credentials) -> BTreeMap<String, String> {
    let mut token = String::new();
    for (u, t) in &cred.device_tokens {
        if u == registry || urls::engine_registry_url(u) == registry {
            token.clone_from(t);
        }
    }
    if token.is_empty() {
        token.clone_from(&cred.env_token);
    }
    let mut h = BTreeMap::new();
    if !token.is_empty() {
        h.insert("Authorization".into(), format!("Bearer {token}"));
    }
    h
}

async fn post(
    base: String,
    path: &str,
    body: String,
    headers: BTreeMap<String, String>,
    bound: Duration,
) -> Outcome {
    let mut o = Outcome {
        url: base.clone(),
        ok: false,
        status: 0,
        detail: String::new(),
    };
    // The body carries the token and the tunnel secret: never in clear past loopback and the allowlist.
    let why = urls::plaintext_refusal(&base);
    if !why.is_empty() {
        o.detail = why;
        return o;
    }
    let mut r = Request::post_json(urls::join(&base, path), body);
    r.headers = headers;
    r.timeout = bound;
    let res = http::send(r).await;
    o.status = res.status;
    o.ok = res.ok();
    if !o.ok {
        o.detail = detail_of(&res);
    }
    o
}

/// `post` to every url at once, each one's outcome kept: the snapshot shows one light, the log says which.
async fn all(
    list: &[String],
    path: &'static str,
    body: &Value,
    cred: &Credentials,
    bound: Duration,
) -> Beat {
    let text = sorted(body.clone()).to_string();
    let calls: Vec<_> = list
        .iter()
        .map(|u| {
            tokio::spawn(post(
                u.clone(),
                path,
                text.clone(),
                headers_for(u, cred),
                bound,
            ))
        })
        .collect();
    let mut beat = Beat::default();
    for (u, c) in list.iter().zip(calls) {
        beat.outcomes.push(c.await.unwrap_or_else(|e| Outcome {
            url: u.clone(),
            ok: false,
            status: 0,
            detail: format!("could not send: {e}"),
        }));
    }
    beat
}

fn identified(mut body: Value, id: &Identity) -> Value {
    body["id"] = id.id.clone().into();
    body["token"] = id.token.clone().into();
    if !id.tunnel_secret.is_empty() {
        body["tunnelSecret"] = id.tunnel_secret.clone().into();
    }
    body
}

/// Announce to every registry in parallel, with the id's token and tunnel secret; the registry binds the id to the address (`docs/engine/token.md`).
pub async fn heartbeat(
    list: &[String],
    id: &Identity,
    presence: Value,
    cred: &Credentials,
) -> Beat {
    heartbeat_within(list, id, presence, cred, BEAT_BOUND).await
}

/// `heartbeat` with its own bound, for a test that waits on a hang.
pub async fn heartbeat_within(
    list: &[String],
    id: &Identity,
    presence: Value,
    cred: &Credentials,
    bound: Duration,
) -> Beat {
    all(
        list,
        "/api/registry/heartbeat",
        &identified(presence, id),
        cred,
        bound,
    )
    .await
}

/// Leave every registry, in parallel; the caller bounds it, since it runs while somebody waits to quit.
pub async fn deregister(list: &[String], id: &Identity) -> Beat {
    let body = identified(Value::Object(Map::new()), id);
    all(
        list,
        "/api/registry/deregister",
        &body,
        &Credentials::default(),
        BEAT_BOUND,
    )
    .await
}
