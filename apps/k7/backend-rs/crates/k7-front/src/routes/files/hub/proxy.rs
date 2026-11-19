//! proxy.rs — `/api/files/*` on a device, answered by its user's runtime in the front (`docs/files/hub.md`): the ticket
//! or the session says who, the engine a request names is bound to the account. What a reader gets wrong: the libraries' paths never reach the account's runtime.

use std::sync::Arc;

use axum::extract::{Request, State};
use axum::response::Response;
use hyper::header::HeaderValue;
use hyper::{HeaderMap, Method, StatusCode};

use super::super::files_of;
use super::super::service::{self, Pass};
use crate::AppState;
use crate::session::Signed;
use crate::web::error;

/// Who the session says is calling.
pub(super) fn session_of(state: &AppState, req: &Request) -> Option<Signed> {
    crate::session::signed(req.headers(), &state.settings, &state.revocations)
}

fn is_libraries(path: &str) -> bool {
    path == "/account/libraries" || path.starts_with("/account/libraries/")
}

/// The session's email, `emailOf(req) ?? ""`.
pub(super) fn email_of(s: &Signed) -> String {
    s.user
        .get("email")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

/// A standalone's files wait for its libraries, and only where a write would need them: a workspace made
/// before the bundled library is installed has none (`docs/files.md`). Reads answer at once — the page's
/// own list of workspaces resolves no library, and a phone's first screen asks for it before they settle.
pub(super) fn starting() -> Response {
    let mut r = error(
        StatusCode::SERVICE_UNAVAILABLE,
        "k7 is starting: its libraries are being installed",
    );
    r.headers_mut()
        .insert("retry-after", HeaderValue::from_static("1"));
    r
}

/// Whether this request could CREATE something that needs the libraries, which is the one thing the gate holds back.
fn writes(req: &Request) -> bool {
    !matches!(*req.method(), Method::GET | Method::HEAD | Method::OPTIONS)
}

/// The `?token=` of a query, as Express's `String(req.query.token)` reads a repeated one.
fn query_token(query: &str) -> String {
    url::form_urlencoded::parse(query.as_bytes())
        .filter(|(k, _)| k == "token")
        .map(|(_, v)| v.into_owned())
        .collect::<Vec<_>>()
        .join(",")
}

fn header(h: &HeaderMap, name: &str) -> Option<String> {
    h.get(name)
        .map(|v| String::from_utf8_lossy(v.as_bytes()).into_owned())
}

/// `/api/files[/*]`: the ticket or the session says who, the libraries' paths go to their service, the rest to the account's.
pub async fn files(State(state): State<Arc<AppState>>, req: Request) -> Response {
    let hub = &files_of(&state).hub;
    if !hub.ready() && writes(&req) {
        return starting();
    }
    let presented = header(req.headers(), "x-studio-engine-token")
        .unwrap_or_else(|| query_token(req.uri().query().unwrap_or("")));
    let ticket = (!presented.is_empty())
        .then(|| k7_hub::ticket::read(&hub.ticket_key, &presented, crate::session::now_ms()))
        .flatten();
    let session = if ticket.is_none() {
        session_of(&state, &req)
    } else {
        None
    };
    let user = match (&ticket, &session) {
        (Some(t), _) => t.user.clone(),
        (None, Some(s)) => email_of(s),
        _ => String::new(),
    };
    if user.is_empty() {
        return error(
            StatusCode::UNAUTHORIZED,
            "no valid files ticket, and no session",
        );
    }
    let path = req.uri().path().strip_prefix("/api/files").unwrap_or("");
    let path = if path.is_empty() {
        "/".to_string()
    } else {
        path.to_string()
    };
    let target = match req.uri().query() {
        Some(q) => format!("{path}?{q}"),
        None => path.clone(),
    };
    let response = if is_libraries(&path) {
        libraries(&state, &target, req).await
    } else {
        let engine = header(req.headers(), "x-studio-engine-id")
            .or_else(|| ticket.and_then(|t| t.engine_id));
        forward(&state, &user, engine, &target, req).await
    };
    match &session {
        Some(s) => crate::session::with_cookies(response, s),
        None => response,
    }
}

/// The account's engine: let go once its claim moved on, bound when the request names one it holds.
async fn bind(state: &AppState, user: &str, engine: Option<&str>) {
    let hub = &files_of(state).hub;
    if engine.is_none() && hub.bindings.get(user).is_none() {
        return;
    }
    let (slug, home) = hub.names.name_of(user);
    let acct = hub.bindings.of(user, &slug, &home);
    // An engine is one account's at a time: whoever claimed it since, this account's link goes first.
    if let Err(e) = k7_hub::binding::release_if_unclaimed(&hub.seam, &acct, &hub.log).await {
        (hub.log)(&format!("[{slug}] could not let go of its engine: {e}"));
    }
    // A binding that fails leaves the files reachable; only the engine is not, and the log says why.
    if let Some(id) = engine
        && let Err(e) =
            k7_hub::binding::bind_engine(&hub.seam, &acct, id, Some(&hub.mediatek_root), &hub.log)
                .await
    {
        (hub.log)(&format!("[{slug}] could not bind engine {id}: {e}"));
    }
}

async fn forward(
    state: &AppState,
    user: &str,
    engine: Option<String>,
    target: &str,
    req: Request,
) -> Response {
    // The runtime's `/front/*` is no client's to reach, whatever its account.
    if service::names_front(target.split('?').next().unwrap_or(target)) {
        return error(StatusCode::NOT_FOUND, "Not Found");
    }
    let files = files_of(state);
    if let Some(why) = files.service.refusal(user) {
        return error(StatusCode::SERVICE_UNAVAILABLE, why);
    }
    let engine = engine.filter(|e| !e.is_empty());
    bind(state, user, engine.as_deref()).await;
    match service::answer(files, &files.hub.names, user, target, req).await {
        Ok(r) => r,
        // The front takes every account that has a code root, and one without was refused above.
        Err(Pass) => error(StatusCode::SERVICE_UNAVAILABLE, "file service unavailable"),
    }
}

/// A request for the libraries, under `target`; 503 while the front's own are not made.
pub(super) async fn libraries(state: &AppState, target: &str, req: Request) -> Response {
    let hub = &files_of(state).hub;
    match hub.library_native.get() {
        Some(native) => crate::routes::files::service::libraries::answer(native, target, req).await,
        None => error(
            StatusCode::SERVICE_UNAVAILABLE,
            files_of(state).service.libraries_refusal(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use axum::body::Body;

    use super::*;

    fn asking(method: &str) -> Request {
        Request::builder()
            .method(method)
            .uri("/api/files/workspaces/projects")
            .body(Body::empty())
            .unwrap()
    }

    #[test]
    fn only_a_write_waits_for_the_libraries() {
        assert!(!writes(&asking("GET")) && !writes(&asking("HEAD")));
        assert!(writes(&asking("POST")) && writes(&asking("PUT")) && writes(&asking("DELETE")));
    }

    #[test]
    fn reads_a_repeated_token_as_express_does() {
        assert_eq!(query_token("a=1&token=x%2By"), "x+y");
        assert_eq!(query_token("token=a&token=b"), "a,b");
        assert_eq!(query_token(""), "");
    }
}
