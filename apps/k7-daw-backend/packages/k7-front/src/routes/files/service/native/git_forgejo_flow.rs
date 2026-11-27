//! git_forgejo_flow.rs — a Forgejo sign-in in the browser (`docs/files/git.md`, "Forgejo"): `start` answers the page to
//! open, `GET /git/forge/callback` takes the code back, `poll` tells the page how it ended. What a reader gets wrong: the
//! callback carries no k7 cookie (the browser comes from the server), so the `state` it brings is what proves k7 started
//! this sign-in, and the PKCE verifier never leaves the front.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::response::Response;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use git_rules::{ForgeStart, PollAnswer, PollState};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::git::{Ctx, typed};
use super::git_forge::{Forge, fail, find};
use super::git_forge_token::finish;
use super::git_forgejo::{APP_REDIRECT, CALLBACK, Forgejo};

/// How long a started sign-in waits for the browser to come back.
const WAIT: Duration = Duration::from_secs(600);

struct Pending {
    slot: String,
    id: String,
    verifier: String,
    redirect: String,
    at: Instant,
}

/// The sign-ins started, by their `state`, and how the ended ones ended, by account and server.
#[derive(Default)]
struct Flows {
    pending: HashMap<String, Pending>,
    ended: HashMap<String, PollAnswer>,
}

fn flows() -> std::sync::MutexGuard<'static, Flows> {
    static F: OnceLock<Mutex<Flows>> = OnceLock::new();
    let m = F.get_or_init(Default::default);
    let mut g = m.lock().unwrap_or_else(|p| p.into_inner());
    g.pending.retain(|_, p| p.at.elapsed() < WAIT);
    g
}

/// One sign-in at a time per account folder and server.
fn slot(ctx: &Ctx, id: &str) -> String {
    format!("{}\n{id}", ctx.account_home)
}

fn random() -> String {
    let mut b = [0u8; 32];
    getrandom::fill(&mut b).expect("the system's random generator");
    URL_SAFE_NO_PAD.encode(b)
}

/// `POST /git/forges/<id>/start { redirect? }`: the page to open; `redirect: "app"` returns through the app's scheme.
pub(super) fn start(ctx: &Ctx, f: &Forgejo, b: &Value) -> Response {
    // The built-in client accepts a loopback root alone, so the app's scheme is for a client the person registered.
    let builtin = f.builtin();
    let app = !builtin && b.get("redirect").and_then(Value::as_str) == Some("app");
    if !app && ctx.port == 0 {
        return fail(
            503,
            "no-port",
            "k7's backend does not know its own port yet",
        );
    }
    let redirect = if app {
        APP_REDIRECT.to_string()
    } else if builtin {
        format!("http://127.0.0.1:{}/", ctx.port)
    } else {
        format!("http://127.0.0.1:{}{CALLBACK}", ctx.port)
    };
    let (verifier, state) = (random(), random());
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let url = f.authorize_url(&redirect, &state, &challenge);
    let here = slot(ctx, &f.server.id);
    let mut g = flows();
    g.pending.retain(|_, p| p.slot != here);
    g.ended.remove(&here);
    let p = Pending {
        slot: here,
        id: f.server.id.clone(),
        verifier,
        redirect: redirect.clone(),
        at: Instant::now(),
    };
    g.pending.insert(state, p);
    typed(
        200,
        &ForgeStart {
            authorize_url: url,
            redirect,
            expires_in: WAIT.as_secs(),
        },
    )
}

/// `POST /git/forges/<id>/poll`: `pending` while the browser is out, then how it ended, said once.
pub(super) fn poll(ctx: &Ctx, f: &Forgejo) -> Response {
    let here = slot(ctx, &f.server.id);
    let mut g = flows();
    if let Some(v) = g.ended.remove(&here) {
        return typed(200, &v);
    }
    let waiting = g.pending.values().any(|p| p.slot == here);
    let state = if waiting {
        PollState::Pending
    } else {
        PollState::Expired
    };
    typed(200, &PollAnswer::state(state))
}

fn arg(query: &str, name: &str) -> Option<String> {
    url::form_urlencoded::parse(query.as_bytes())
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.into_owned())
        .filter(|v| !v.is_empty())
}

/// `GET /git/forge/callback?code&state` (or `error`): the code exchanged, the sign-in ended, a page for the tab.
pub(super) async fn callback(ctx: &Ctx, query: &str) -> Response {
    let taken = arg(query, "state").and_then(|s| flows().pending.remove(&s));
    let Some(p) = taken.filter(|p| p.slot == slot(ctx, &p.id)) else {
        return page(
            400,
            "This sign-in has expired",
            "Start it again from k7's Settings.",
        );
    };
    let found = find(ctx, &p.id);
    let Some(f @ Forge::Forgejo(g)) = found.as_ref() else {
        return page(
            404,
            "This server was removed",
            "Add it again from k7's Settings.",
        );
    };
    let outcome = match (arg(query, "error"), arg(query, "code")) {
        (Some(e), _) => {
            let state = if e == "access_denied" {
                PollState::Denied
            } else {
                PollState::Error
            };
            let said = arg(query, "error_description").unwrap_or(e);
            PollAnswer {
                error: Some(said),
                ..PollAnswer::state(state)
            }
        }
        (None, Some(code)) => match g.exchange(&code, &p.redirect, &p.verifier).await {
            Ok(t) => match finish(ctx, f, t).await {
                Ok(d) => PollAnswer {
                    login: Some(d.login),
                    scopes: Some(d.scopes),
                    key_note: d.note,
                    ..PollAnswer::state(PollState::Ok)
                },
                Err((_, code, said)) => PollAnswer {
                    code: Some(code),
                    error: Some(said),
                    ..PollAnswer::state(PollState::Error)
                },
            },
            Err(said) => PollAnswer {
                error: Some(said),
                ..PollAnswer::state(PollState::Error)
            },
        },
        (None, None) => PollAnswer {
            error: Some("Forgejo sent no code back".to_string()),
            ..PollAnswer::state(PollState::Error)
        },
    };
    let ok = outcome.state == PollState::Ok;
    let said = outcome.error.clone().unwrap_or_default();
    let login = outcome.login.clone().unwrap_or_default();
    flows().ended.insert(p.slot, outcome);
    if ok {
        let line = format!("Signed in to {} as {login}: you can close this tab.", p.id);
        page(200, "Signed in", &line)
    } else {
        page(400, "Not signed in", &said)
    }
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The small page the browser tab shows once Forgejo sent it back.
fn page(status: u16, title: &str, said: &str) -> Response {
    let html = format!(
        "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width\"><title>k7</title><body style=\"font:16px system-ui;margin:3em auto;max-width:32em;padding:0 1em\"><h1 style=\"font-size:1.3em\">{}</h1><p>{}</p></body></html>",
        escape(title),
        escape(said)
    );
    Response::builder()
        .status(status)
        .header("content-type", "text/html; charset=utf-8")
        .header("cache-control", "no-store")
        .body(Body::from(html))
        .unwrap_or_default()
}
