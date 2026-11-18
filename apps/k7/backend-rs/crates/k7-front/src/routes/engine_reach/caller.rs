//! caller.rs — who a request is, as the backend's guards decide it: `bearerOrSession(requireAuth)` for
//! the routes a service may call and `requireAuth` for the session-only ones.
//! What a reader gets wrong: a bearer that does not verify is refused outright, never downgraded to the cookie.

use axum::extract::Request;
use axum::response::Response;
use hyper::StatusCode;
use hyper::header::AUTHORIZATION;
use serde_json::{Map, Value, json};

use super::seams_of;
use crate::AppState;
use crate::session::{Signed, signed};
use crate::web::error;

/// Who asked.
#[derive(Clone)]
pub enum Caller {
    /// A page's session.
    Session(Signed),
    /// A bearer's subject: a file service, a script.
    Bearer(String),
}

impl Caller {
    /// `req.user` as the backend's routes read it.
    pub fn user(&self) -> Option<Map<String, Value>> {
        match self {
            Caller::Session(s) => Some(s.user.clone()),
            Caller::Bearer(sub) => json!({ "sub": sub, "name": sub, "email": null })
                .as_object()
                .cloned(),
        }
    }

    /// The answer, with the cookies a rewrite to the local user started.
    pub fn finish(&self, response: Response) -> Response {
        match self {
            Caller::Session(s) => crate::session::with_cookies(response, s),
            _ => response,
        }
    }
}

/// The bearer of `Authorization: Bearer <token>`, as `/^Bearer\s+(.+)$/i` reads it.
pub(crate) fn bearer(req: &Request) -> Option<String> {
    let v = req.headers().get(AUTHORIZATION)?.to_str().ok()?;
    v.get(..6).filter(|p| p.eq_ignore_ascii_case("bearer"))?;
    let after = &v[6..];
    // `\s+(.+)` needs a blank, then any one character, a blank included, which the trim then drops.
    let blank = after.chars().next().is_some_and(char::is_whitespace);
    (blank && after.chars().count() >= 2).then(|| after.trim().to_string())
}

/// `bearerOrSession(requireAuth)`: a bearer is the backend's to verify, else the session; the future holds no request.
pub fn bearer_or_session<'a>(
    state: &'a AppState,
    req: &Request,
) -> impl Future<Output = Result<Caller, Box<Response>>> + Send + 'a {
    let decided = match bearer(req) {
        None => Err(session_only(state, req)),
        Some(token) => Ok(token),
    };
    async move {
        let token = match decided {
            Ok(t) => t,
            Err(answer) => return answer,
        };
        // A bearer this front minted for a file service (`routes/files/hub`), which the backend never saw.
        if let Some(sub) = state.files.as_ref().and_then(|f| {
            f.hub
                .verify_service_token(&state.settings.api_token_secret, &token)
        }) {
            return Ok(Caller::Bearer(sub));
        }
        match seams_of(state).seam.verify_bearer(&token).await {
            Ok(Some(sub)) => Ok(Caller::Bearer(sub)),
            Ok(None) => Err(Box::new(error(StatusCode::UNAUTHORIZED, "invalid token"))),
            Err(()) => Err(Box::new(error(
                StatusCode::BAD_GATEWAY,
                "k7's backend did not say whose this token is; try again",
            ))),
        }
    }
}

/// `requireAuth`: the session alone.
pub fn session_only(state: &AppState, req: &Request) -> Result<Caller, Box<Response>> {
    signed(req.headers(), &state.settings, &state.revocations)
        .map(Caller::Session)
        .ok_or_else(|| Box::new(error(StatusCode::UNAUTHORIZED, "unauthenticated")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;

    fn with(auth: &str) -> Request {
        Request::builder()
            .header(AUTHORIZATION, auth)
            .body(Body::empty())
            .unwrap()
    }

    #[test]
    fn reads_a_bearer_as_the_backend_does() {
        assert_eq!(bearer(&with("Bearer abc")).as_deref(), Some("abc"));
        assert_eq!(bearer(&with("bearer   abc ")).as_deref(), Some("abc"));
        assert_eq!(bearer(&with("BEARER\tabc")).as_deref(), Some("abc"));
        assert_eq!(bearer(&with("Bearerabc")), None);
        assert_eq!(bearer(&with("Bearer ")), None);
        assert_eq!(bearer(&with("Bearer  ")).as_deref(), Some(""));
        assert_eq!(bearer(&with("Basic abc")), None);
    }

    #[test]
    fn a_bearer_is_a_user_with_no_email() {
        let user = Caller::Bearer("service:k7".into()).user().unwrap();
        assert_eq!(
            Value::Object(user),
            json!({ "sub": "service:k7", "name": "service:k7", "email": null })
        );
    }
}
