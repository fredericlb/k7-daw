//! landing.rs — `POST /changes` that creates a project folder (P9's import): the folder is checked free, the set lands as any
//! other, and a set that does not land (or a `discard`) takes the folder away with the samples uploaded into it.
//! What a reader gets wrong: a folder that holds a project is never removed, whatever the set came to.

use axum::response::Response;
use k7_file_service::changes::{Refusal, landing};
use k7_file_service::revisions::Revisions;
use k7_file_service::workspaces::EXAMPLES;
use serde_json::json;

use super::entries::blocking;
use super::{Ctx, changes, locks};
use crate::routes::files::json_answer;
use crate::routes::files::service::routes::Call;

/// The sentence, with the flag a client reads to tell a taken name from a stale set.
fn taken(r: &Refusal) -> Response {
    let body = json!({ "error": r.to_string(), "taken": true, "stale": [], "project": false });
    json_answer(r.status(), &body)
}

/// The set that creates `body.create`, answered; a refusal other than "taken" and a failed landing leave no new folder.
pub async fn answer(c: &Call, ctx: &Ctx) -> Response {
    let at = c.body.get("at").and_then(|v| v.as_str()).unwrap_or("");
    let Some(name) = c.body.get("create").and_then(|v| v.as_str()) else {
        return changes::answer_set(c, ctx).await;
    };
    let name = match landing::folder_name(at, name) {
        Ok(n) => n,
        Err(r) => return json_answer(r.status(), &json!({ "error": r.to_string() })),
    };
    // The bundled demos are mirrored on every start: a project the bundle does not hold would sit among them.
    if landing::is_examples(&ctx.root) {
        let why = format!(
            "{EXAMPLES} holds the bundled demos and takes no new project — make it in a workspace of your own"
        );
        return json_answer(403, &json!({ "error": why }));
    }
    // A client whose uploads failed before its set asks for the folder to be taken away: no set follows.
    if c.body.get("discard").and_then(|v| v.as_bool()) == Some(true) {
        discard(ctx, &name).await;
        return json_answer(200, &json!({ "discarded": true }));
    }
    let root = ctx.root.clone();
    let held = {
        let (root, name) = (root.clone(), name.clone());
        blocking(move || {
            landing::holds_project(&k7_file_service::paths::join(&[
                &k7_file_service::paths::resolve_all(&[&root]),
                &name,
            ]))
        })
        .await
    };
    if held != Some(false) {
        return taken(&landing::taken(&name));
    }
    let r = changes::answer_set(c, ctx).await;
    if r.status().as_u16() != 200 {
        discard(ctx, &name).await;
    }
    r
}

/// The folder taken away under the account's write lock, where a project that landed meanwhile is seen and kept.
async fn discard(ctx: &Ctx, name: &str) {
    let _lock = locks::write(&ctx.home.studio).await;
    let (root, name) = (ctx.root.clone(), name.to_string());
    let gone = blocking(move || landing::discard(&root, &name)).await;
    if gone == Some(true) {
        Revisions::global().bump(&ctx.root);
    }
}
