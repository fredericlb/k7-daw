//! queries.rs — what the engine says of ONE plugin, asked on the bare session: its parameters, its window, a snapshot of its state (2340).
//! What a reader gets wrong: silence is an answer too, and each query reads it as the TypeScript does (503, 504, 503), never as one rule.

use serde_json::{Value, json};

use super::waiters::Want;
use crate::routes::files::service::native::engine::EngineLink;

/// The status and body a route sends.
pub type Reply = (u16, Value);

const NO_ANSWER: &str = "the engine did not answer — is it running?";

fn said(status: u16, message: &str) -> Reply {
    (status, json!({ "error": message }))
}

/// Wait for the engine's answer on `key`, registered before `queue` sends the frame; none when no engine is bound or it stays silent.
pub(super) async fn ask_with(
    waiters: &std::sync::Arc<super::waiters::Waiters>,
    want: Want,
    key: &str,
    queue: impl FnOnce() -> bool,
) -> Option<Value> {
    let waiting = waiters.wait(want, key);
    if !queue() {
        return None;
    }
    waiting.answer().await
}

async fn ask(link: &EngineLink, want: Want, key: &str, frame: Value) -> Option<Value> {
    ask_with(&link.0.waiters, want, key, || {
        link.session(None).frame(frame)
    })
    .await
}

/// `GET /plugin-params`: the plugin loaded and asked, three outcomes (found, not found, silence).
pub async fn params(link: &EngineLink, format: &str, id: &str) -> Reply {
    let frame = json!({ "type": "list-plugin-params", "format": format, "id": id });
    match ask(link, Want::Params, id, frame).await {
        None => said(503, NO_ANSWER),
        // An engine too old to send the flag loaded something to answer at all, so absent reads as true.
        Some(f) => (
            200,
            json!({
                "found": f.get("found") != Some(&Value::Bool(false)),
                "params": f.get("params").filter(|p| !p.is_null()).cloned().unwrap_or_else(|| json!([])),
            }),
        ),
    }
}

/// `GET /plugin-native-ui`: the plugin's own window on an orphan instance.
pub async fn window(link: &EngineLink, format: &str, id: &str) -> Reply {
    let frame = json!({ "type": "show-native-plugin-ui-of", "format": format, "id": id });
    match ask(link, Want::Window, id, frame).await {
        None => said(
            504,
            "the engine never said it opened one — it may be busy loading the plugin, or its offline engine may be wedged (the parameter scan hangs the same way)",
        ),
        Some(f) if f.get("ok") != Some(&Value::Bool(true)) => said(
            502,
            &format!("the engine could not open a window for {id} — see its log"),
        ),
        Some(_) => (200, json!({ "opened": id })),
    }
}

/// `POST /module-base/snapshot`'s frame: the orphan's state written to `abs`, answered as `path` when it landed.
pub async fn base(link: &EngineLink, format: &str, id: &str, abs: &str, path: &str) -> Reply {
    let frame = json!({ "type": "snapshot-plugin-base", "format": format, "id": id, "path": abs });
    match ask(link, Want::Base, abs, frame).await {
        None => said(503, NO_ANSWER),
        Some(f) if f.get("ok") != Some(&Value::Bool(true)) => said(
            409,
            "nothing to snapshot — open the plugin's window with « Native UI » and set it up first",
        ),
        Some(_) => (200, json!({ "path": path })),
    }
}
