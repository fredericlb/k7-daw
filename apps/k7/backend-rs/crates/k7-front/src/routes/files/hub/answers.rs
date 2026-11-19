//! answers.rs — what the hub answers itself: the hello that turns a session into a ticket, the runtimes' callbacks on
//! `/api/files-hub/internal` (their own secret is the gate), and the seam's `cold-run`, `slot` and `hub`.
//! What a reader gets wrong: an internal call whose secret is no running service's is a 401 whatever else it carries.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Request, State};
use axum::response::{IntoResponse, Response};
use hyper::{Method, StatusCode};
use serde_json::{Value, json};

use super::super::files_of;
use super::super::service;
use super::proxy::{email_of, libraries, session_of};
use crate::AppState;
use crate::web::{error, read_json_within};

/// `GET /api/files-hub`, behind the session: the ticket naming the account, and whether it is an admin.
pub async fn hello(State(state): State<Arc<AppState>>, req: Request) -> Response {
    let hub = &files_of(&state).hub;
    // No gate: a ticket names an account, and the page asks for it to be able to ask for anything at all.
    let Some(signed) = session_of(&state, &req) else {
        return error(StatusCode::UNAUTHORIZED, "unauthenticated");
    };
    let user = email_of(&signed);
    let response = if user.is_empty() {
        error(StatusCode::UNAUTHORIZED, "no user on the session")
    } else {
        let admin = crate::session::is_admin(&signed.user, &state.settings);
        let ticket = k7_hub::ticket::mint(
            &hub.ticket_key,
            &user,
            admin,
            crate::session::now_ms(),
            k7_hub::ticket::TICKET_TTL_MS,
        );
        Json(json!({ "path": "/api/files", "ticket": ticket, "admin": admin })).into_response()
    };
    crate::session::with_cookies(response, &signed)
}

/// `/api/files-hub/internal/*`: only this hub's own account runtimes, by their secret.
pub async fn internal(State(state): State<Arc<AppState>>, req: Request) -> Response {
    let files = files_of(&state);
    let secret = req
        .headers()
        .get("x-studio-hub-secret")
        .map(|v| String::from_utf8_lossy(v.as_bytes()).into_owned())
        .unwrap_or_default();
    if files.hub.account_of_secret(&secret).await.is_none() {
        return error(
            StatusCode::UNAUTHORIZED,
            "not one of this hub's file services",
        );
    }
    let path = req
        .uri()
        .path()
        .strip_prefix("/api/files-hub/internal")
        .unwrap_or("");
    let path = if path.is_empty() {
        "/".to_string()
    } else {
        path.to_string()
    };
    let method = req.method().clone();
    if method == Method::POST && path == "/libraries/sync" {
        return libraries(&state, &path, false, req).await;
    }
    error(StatusCode::NOT_FOUND, &format!("no {method} {path}"))
}

/// The seam's body, as `express.json({ limit: "8kb" })` reads it.
#[allow(clippy::result_large_err, reason = "the refusal is the answer itself")]
async fn body(req: Request) -> Result<Value, Response> {
    read_json_within(req, 8 * 1024).await.map(|(_, v)| v)
}

fn field(v: &Value, k: &str) -> Option<String> {
    v.get(k)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn refused(message: &str) -> Response {
    error(StatusCode::UNPROCESSABLE_ENTITY, message)
}

/// What a cut shares with the machine: an account's folders on a device, the libraries' alone on a server.
enum Roots {
    Account(std::path::PathBuf, String),
    Libraries,
}

/// `POST /api/internal/files/cold-run`: what a cut of this account's runs, and what it shares with the machine.
pub async fn cold_run(State(state): State<Arc<AppState>>, req: Request) -> Response {
    let hub = &files_of(&state).hub;
    let b = match body(req).await {
        Ok(b) => b,
        Err(r) => return r,
    };
    let (Some(user), Some(want)) = (field(&b, "user"), field(&b, "want")) else {
        return error(StatusCode::BAD_REQUEST, "user and want are required");
    };
    // An EMPTY chain is the account's own setting, as `resolveColdRun` reads it: only the user and the want are required.
    let chain = field(&b, "chain").unwrap_or_default();
    let files = files_of(&state);
    if let Some(why) = files.service.refusal(&user) {
        return refused(why);
    }
    let (roots_of, asked): (Roots, _) = if hub.is_device() {
        match files.service.held(&user, &hub.names).await {
            Ok(h) => {
                let (home, slug) = (h.account.home.clone(), h.account.slug.clone());
                let asked = service::internal::cold_run(&h, &chain, &want).await;
                (Roots::Account(home, slug), asked)
            }
            Err(e) => return refused(&e),
        }
    } else {
        match files.service.cut(&hub.cut).await {
            Ok(c) => (
                Roots::Libraries,
                service::internal::cut_run(&c, &chain, &want).await,
            ),
            Err(e) => return refused(&e),
        }
    };
    let (status, r) = match asked {
        Ok(r) => r,
        Err(e) => return refused(&e),
    };
    let Some(model) = field(&r, "model").filter(|_| (200..300).contains(&status)) else {
        return refused(
            &field(&r, "error").unwrap_or(format!("could not resolve the chain ({status})")),
        );
    };
    let outputs = r
        .get("outputs")
        .cloned()
        .filter(|o| !o.is_null())
        .unwrap_or(json!([]));
    let roots = match roots_of {
        Roots::Account(home, slug) => {
            k7_hub::binding::cut_roots(&home, &slug, &hub.mediatek_root, &model)
        }
        Roots::Libraries => {
            k7_hub::binding::library_cut_roots(&hub.data_root, &hub.mediatek_root, &model)
        }
    };
    Json(json!({ "model": model, "outputs": outputs, "roots": roots })).into_response()
}

/// `POST /api/internal/files/slot`: a chain compiled into a slot, and what its machine must see.
pub async fn slot(State(state): State<Arc<AppState>>, req: Request) -> Response {
    let hub = &files_of(&state).hub;
    let b = match body(req).await {
        Ok(b) => b,
        Err(r) => return r,
    };
    let (Some(user), Some(r#ref)) = (field(&b, "user"), field(&b, "ref")) else {
        return error(StatusCode::BAD_REQUEST, "user and ref are required");
    };
    let files = files_of(&state);
    if let Some(why) = files.service.refusal(&user) {
        return refused(why);
    }
    let (roots_of, asked) = if hub.is_device() {
        match files.service.held(&user, &hub.names).await {
            Ok(h) => (
                Roots::Account(h.account.home.clone(), h.account.slug.clone()),
                service::internal::slot(&h, &r#ref).await,
            ),
            Err(e) => return refused(&e),
        }
    } else {
        match files.service.cut(&hub.cut).await {
            Ok(c) => (
                Roots::Libraries,
                service::internal::cut_slot(&c, &r#ref).await,
            ),
            Err(e) => return refused(&e),
        }
    };
    let (status, r) = match asked {
        Ok(r) => r,
        Err(e) => return refused(&e),
    };
    if !(200..300).contains(&status) || field(&r, "id").is_none() {
        return refused(
            &field(&r, "error").unwrap_or(format!("could not compile the slot ({status})")),
        );
    }
    let roots = match roots_of {
        Roots::Account(home, slug) => {
            let mut roots = k7_hub::binding::roots_for(&home, &slug, Some(&hub.mediatek_root));
            // The account's own library too, read-only: an account chain's models are there, not in a workspace.
            roots.push(json!({ "remote": home.join("library").display().to_string(), "name": format!("lib-{slug}"), "writable": false }));
            roots
        }
        Roots::Libraries => k7_hub::binding::library_slot_roots(&hub.data_root, &hub.mediatek_root),
    };
    Json(json!({ "slot": r, "roots": roots })).into_response()
}
