//! git_keys.rs — the routes that touch no repository's history: the focus heartbeat, the device's identity, its ssh keys
//! and the trusted hosts (`docs/files/git.md`). What a reader gets wrong: `focus` is repeated by the page and expires.
use std::path::Path;

use axum::response::Response;
use k7_git::auth;
use k7_git::{Error, Identity};
use serde_json::{Value, json};

use crate::routes::files::json_answer;

use super::git::{Answer, Ctx, answer_error, bad, field};
use super::git_sync::prefilled;

pub(super) async fn focus(ctx: &Ctx, root: &Path, b: &Value) -> Answer {
    let cadence = ctx.fetchers.of(&root.display().to_string());
    {
        let mut c = cadence.lock().unwrap_or_else(|p| p.into_inner());
        if b.get("opened").and_then(Value::as_bool) == Some(true) {
            c.opened();
        }
        let front = b.get("front").and_then(Value::as_bool).unwrap_or(true);
        c.focus(front, super::git_fetch::now());
    }
    super::git_task::ensure(ctx, root, cadence);
    Ok(json_answer(200, &json!({ "ok": true })))
}

pub(super) fn identity(ctx: &Ctx) -> Response {
    match prefilled(ctx) {
        Ok(id) => json_answer(200, &json!({ "name": id.name, "email": id.email })),
        Err(e) => answer_error(&e),
    }
}

pub(super) fn set_identity(ctx: &Ctx, b: &Value) -> Response {
    let id = Identity {
        name: field(b, "name").unwrap_or("").to_string(),
        email: field(b, "email").unwrap_or("").to_string(),
    };
    match ctx.identity.save(&id) {
        Ok(()) => json_answer(200, &json!({ "ok": true })),
        Err(e) => answer_error(&e),
    }
}

/// `GET /git/key`: the device's public SSH key, made on first use; the private half never leaves the keychain.
pub(super) fn key(ctx: &Ctx) -> Response {
    match auth::public_key(&*ctx.secrets) {
        Ok(public) => json_answer(
            200,
            &key_answer(&public, auth::key_is_synced(&*ctx.secrets)),
        ),
        Err(e) => answer_error(&e),
    }
}

/// The page's answer: the public line and whether the key is shared; nothing of the private half.
fn key_answer(public: &str, synced: bool) -> Value {
    json!({ "publicKey": public, "synced": synced })
}

/// `POST /git/forget-host`: takes the server off the trusted list, so the next connection meets it as a first contact.
pub(super) fn forget_host(ctx: &Ctx, b: &Value) -> Response {
    let Some(host) = field(b, "host") else {
        return bad("host is required");
    };
    let port = b
        .get("port")
        .and_then(Value::as_u64)
        .and_then(|p| u16::try_from(p).ok())
        .unwrap_or(22);
    match ctx.hosts.forget(host, port) {
        Ok(was) => json_answer(200, &json!({ "ok": true, "forgotten": was })),
        Err(e) => answer_error(&Error::Io(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use k7_git::auth::MemorySecrets;

    #[test]
    fn the_key_answer_holds_the_public_line_and_never_the_private_text() {
        let store = MemorySecrets::default();
        let public = auth::public_key(&store).unwrap();
        let kept = store.get_for("device-ssh-key", "k7").unwrap();
        let answer = key_answer(&public, auth::key_is_synced(&store)).to_string();
        assert!(answer.contains("ssh-ed25519 "));
        assert!(answer.contains("\"synced\":false"));
        assert!(!answer.contains("PRIVATE") && !answer.contains(&kept));
    }
}
