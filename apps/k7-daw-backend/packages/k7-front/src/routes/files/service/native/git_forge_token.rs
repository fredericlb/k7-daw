//! git_forge_token.rs — a forge token's life (`docs/files/git.md`, "The forge seam"): a sign-in's end (the token kept,
//! the device's key uploaded, the host trusted) and the refresh before a call. What a reader gets wrong: a rotating
//! refresh token is single-use, so the new one is in the keychain before the new access token is used, and two calls
//! never refresh at once.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::response::Response;
use base64::Engine;
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD};
use k7_git::auth;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::git::Ctx;
use super::git_forge::{Forge, Saved, Tokens, drop_token, fail, keep, saved, sign_in_again};
use super::git_forgejo::Refresh;

/// An access token this close to its expiry is refreshed first, so a call never starts with one about to die.
const MARGIN_S: u64 = 60;

pub(super) fn now_s() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// What a finished sign-in gives the page.
pub(super) struct Done {
    pub login: String,
    pub scopes: Vec<String>,
    pub note: Option<String>,
}

/// Why a sign-in could not end: the status, the code and the sentence.
pub(super) type Refusal = (u16, String, String);

/// The token is kept, then the key is uploaded and the host trusted; the key's trouble is a sentence, not a failure.
pub(super) async fn finish(ctx: &Ctx, f: &Forge, t: Tokens) -> Result<Done, Refusal> {
    let p = f.profile();
    let me = match p.api.get(&t.access, "/user").await {
        Ok(r) if r.ok() => r,
        Ok(r) => return Err((502, p.code.to_string(), r.message())),
        Err(e) => return Err((502, "offline".into(), e)),
    };
    let login = me
        .body
        .get("login")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let s = Saved {
        token: t.access.clone(),
        login: login.clone(),
        scopes: f.granted(&me),
        refresh: t.refresh,
        expiry: t.expiry,
    };
    if let Err(e) = keep(ctx, f, &s) {
        let said = format!("the device's keychain: {e}");
        return Err((500, "keychain".into(), said));
    }
    let note = upload_key(ctx, f, &t.access).await;
    f.trust_host(ctx).await;
    Ok(Done {
        login,
        scopes: s.scopes,
        note,
    })
}

/// `SHA256:` and the unpadded base64 of the hash of the key's blob, as the SSH client names a key.
pub(super) fn fingerprint(line: &str) -> Option<String> {
    let blob = STANDARD.decode(line.split_whitespace().nth(1)?).ok()?;
    Some(format!(
        "SHA256:{}",
        STANDARD_NO_PAD.encode(Sha256::digest(blob))
    ))
}

/// Puts the device's public key on the account unless it is there; a sentence when it cannot be.
async fn upload_key(ctx: &Ctx, f: &Forge, token: &str) -> Option<String> {
    let p = f.profile();
    let public = auth::public_key(&*ctx.secrets).ok()?;
    let ours = fingerprint(&public)?;
    let listed = match p.api.get(token, &f.keys_path(&ours)).await {
        Ok(r) if r.ok() => r,
        Ok(r) => return Some(r.message()),
        Err(e) => return Some(e),
    };
    let there = listed.body.as_array().is_some_and(|a| {
        a.iter().any(|k| {
            k.get("key")
                .and_then(Value::as_str)
                .and_then(fingerprint)
                .is_some_and(|k| k == ours)
        })
    });
    if there {
        return None;
    }
    let title = if auth::key_is_synced(&*ctx.secrets) {
        "k7".to_string()
    } else {
        format!("k7 — {}", ctx.device)
    };
    let post = |title: String| {
        let sent = json!({ "title": title, "key": public });
        async move { p.api.send("POST", token, "/user/keys", sent).await }
    };
    let mut sent = post(title.clone()).await;
    // A title is unique per account on Forgejo: another device of the same name gets the key's own short name beside it.
    if let Ok(r) = &sent
        && r.status == 422
        && r.message().contains("title has been used")
    {
        let short: String = ours.trim_start_matches("SHA256:").chars().take(8).collect();
        sent = post(format!("{title} ({short})")).await;
    }
    match sent {
        Ok(r) if r.ok() => None,
        Ok(r) if r.status == 422 && !r.message().contains("title has been used") => {
            Some(f.key_elsewhere())
        }
        Ok(r) => Some(r.message()),
        Err(e) => Some(e),
    }
}

fn locks() -> &'static Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>> {
    static L: OnceLock<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>> = OnceLock::new();
    L.get_or_init(Default::default)
}

fn lock_of(ctx: &Ctx, f: &Forge) -> Arc<tokio::sync::Mutex<()>> {
    let key = format!("{}\n{}", ctx.account_home, f.profile().id);
    let mut map = locks().lock().unwrap_or_else(|p| p.into_inner());
    map.entry(key).or_default().clone()
}

fn stale(s: &Saved) -> bool {
    s.refresh.is_some() && s.expiry.is_some_and(|e| now_s() + MARGIN_S >= e)
}

/// The kept token, refreshed first when it is about to expire; a refused refresh drops it and asks to sign in again.
#[allow(
    clippy::result_large_err,
    reason = "the refusal is the answer to give, built once"
)]
pub(super) async fn fresh(ctx: &Ctx, f: &Forge, s: Saved) -> Result<Saved, Response> {
    if !stale(&s) {
        return Ok(s);
    }
    let lock = lock_of(ctx, f);
    let _held = lock.lock().await;
    // Read again under the lock: the call that held it may have refreshed already.
    let Some(s) = saved(ctx, f) else {
        return Err(sign_in_again(f));
    };
    let Some(refresh) = s.refresh.clone().filter(|_| stale(&s)) else {
        return Ok(s);
    };
    match f.refresh(&refresh).await {
        Ok(t) => {
            let next = Saved {
                token: t.access,
                login: s.login,
                scopes: s.scopes,
                refresh: t.refresh,
                expiry: t.expiry,
            };
            keep(ctx, f, &next)
                .map_err(|e| fail(500, "keychain", &format!("the device's keychain: {e}")))?;
            Ok(next)
        }
        Err(Refresh::Offline(e)) => Err(fail(502, "offline", &e)),
        Err(Refresh::Refused) => {
            let _ = drop_token(ctx, f);
            Err(sign_in_again(f))
        }
    }
}
