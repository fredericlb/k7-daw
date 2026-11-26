//! plugins.rs — the engine's plugin catalogue: the cached list, and a rescan that joins one already running (3f, M9).
//! What a reader gets wrong: the answer is the engine's next `plugins` frame, so silence is `[]`, not an error, as the TypeScript's four seconds are.

use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock, PoisonError};

use axum::response::Response;
use k7_file_service::verb_host::Error;
use serde_json::{Value, json};

use super::account::{QUERY_WAIT, ready};
use super::{Checked, failure};
use crate::routes::files::json_answer;
use crate::routes::files::service::body;
use crate::routes::files::service::native::engine::{EngineLink, Session};
use crate::routes::files::service::native::jobs::waiters::Want;

/// The longest filter the route takes.
const FILTER_MAX: usize = 256;

fn plugins_of(frame: Option<Value>) -> Value {
    match frame.and_then(|f| f.get("plugins").cloned()) {
        Some(p @ Value::Array(_)) => p,
        _ => json!([]),
    }
}

/// One list ask at a time per account: the engine's `plugins` frame does not say which filter it answers.
fn one_list(account: &str) -> std::sync::Arc<tokio::sync::Mutex<()>> {
    static LOCKS: OnceLock<Mutex<HashMap<String, std::sync::Arc<tokio::sync::Mutex<()>>>>> =
        OnceLock::new();
    LOCKS
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .entry(account.to_owned())
        .or_default()
        .clone()
}

/// The engine's cached list under `filter`, `[]` when the frame did not go or no answer came.
async fn listed(link: &EngineLink, account: &str, session: &Session, filter: &str) -> Value {
    let lock = one_list(account);
    let _one = lock.lock().await;
    let waiting = link.0.waiters.wait(Want::Plugins, "");
    let mut frame = json!({ "type": "list-plugins" });
    if !filter.is_empty() {
        frame["filter"] = json!(filter);
    }
    if !session.frame(frame) {
        return json!([]);
    }
    plugins_of(waiting.answer().await)
}

/// The engine's plugin names, lowercase; none when no engine answers, which reads as "unknown" and skips the check of names.
pub async fn names(link: &EngineLink, root: &str, account: &str) -> Option<HashSet<String>> {
    let session = ready(link, root, QUERY_WAIT).await?;
    let list = listed(link, account, &session, "").await;
    let names: HashSet<String> = list
        .as_array()?
        .iter()
        .filter_map(|p| p.get("name").and_then(Value::as_str))
        .filter(|n| !n.is_empty())
        .map(str::to_lowercase)
        .collect();
    (!names.is_empty()).then_some(names)
}

/// `GET /verb-host/plugins?filter=`: the engine's cached list, `[]` with no engine.
pub async fn list(req: &Checked, link: &EngineLink) -> Response {
    let filter = body::query(&req.query, "filter");
    if filter.len() > FILTER_MAX {
        return failure(&Error::new(400, "filter is too long"));
    }
    let Some(session) = ready(link, &req.ctx.root, QUERY_WAIT).await else {
        return json_answer(200, &json!({ "plugins": [] }));
    };
    let plugins = listed(link, &req.ctx.account, &session, &filter).await;
    json_answer(200, &json!({ "plugins": plugins }))
}

/// The accounts whose rescan is running: a second asks nothing of the engine and hears the first's list.
fn running() -> &'static Mutex<HashSet<String>> {
    static SET: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    SET.get_or_init(Mutex::default)
}

struct Scanning(String);

impl Drop for Scanning {
    fn drop(&mut self) {
        running()
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&self.0);
    }
}

/// `POST /verb-host/plugins/rescan {force?}`: an incremental scan and the fresh list.
pub async fn rescan(req: &Checked, link: &EngineLink) -> Response {
    let force = match req.body.get("force") {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(_) => return failure(&Error::new(400, "force must be a boolean")),
    };
    let Some(session) = ready(link, &req.ctx.root, QUERY_WAIT).await else {
        return json_answer(200, &json!({ "plugins": [] }));
    };
    let waiting = link.0.waiters.wait(Want::ScanEnded, "");
    let key = req.ctx.account.clone();
    let first = running()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(key.clone());
    let held = first.then(|| Scanning(key));
    if first {
        let mut frame = json!({ "type": "rescan-plugins" });
        if force {
            frame["force"] = json!(true);
        }
        if !session.frame(frame) {
            return json_answer(200, &json!({ "plugins": [] }));
        }
    }
    // The scan ends on the engine's status, not on any list frame; the list it left is then read, by the joiners too.
    let ended = waiting.answer().await;
    drop(held);
    if ended.is_none() {
        return json_answer(200, &json!({ "plugins": [] }));
    }
    let plugins = listed(link, &req.ctx.account, &session, "").await;
    json_answer(200, &json!({ "plugins": plugins }))
}
