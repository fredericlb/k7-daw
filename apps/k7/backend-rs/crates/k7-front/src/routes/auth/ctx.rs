//! ctx.rs — a request's session as `setupAuth` leaves it to its routes: the cookie read by cookie-session's
//! rules, a user whose stamps fail dropped before any route reads it, and on a device any other account turned
//! into the local user. What a reader gets wrong: `/auth/{logout,me}` sit BEFORE that device
//! rewrite in `apps/k7/backend/src/server.ts`; `/auth/local` and the token routes sit after it.

use std::time::{SystemTime, UNIX_EPOCH};

use axum::response::Response;
use hyper::HeaderMap;
use hyper::header::{COOKIE, HeaderValue, SET_COOKIE};
use k7_auth::{CookieOptions, CookieSession, SESSION_MAX_AGE_MS};
use serde_json::{Map, Value};

use crate::AppState;
use crate::session::{COOKIE_NAME, https, local_user};

pub fn now_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as f64
}

/// The request's session, and what writing it back depends on.
pub struct Ctx {
    pub bag: CookieSession,
    https: bool,
}

/// The session's user when its stamps hold; cookie-session never reads `_expire`, so neither does this.
fn valid_user(data: &Map<String, Value>, state: &AppState) -> Option<Map<String, Value>> {
    let mut stamped = data.clone();
    stamped.shift_remove("_expire");
    k7_auth::valid_session_user(&stamped, now_ms(), k7_auth::GROUPS_MAX_AGE_MS, |sub| {
        state.revocations.not_before(sub)
    })
}

impl Ctx {
    /// The session middleware and the stamps check, as every `/auth` route sees them.
    pub fn open(headers: &HeaderMap, state: &AppState) -> Ctx {
        let header = headers
            .get_all(COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .collect::<Vec<_>>()
            .join("; ");
        let mut bag = CookieSession::read(&header, COOKIE_NAME, &state.settings.session_secret);
        let user = bag.data().get("user");
        if user.is_some_and(crate::js::truthy) && valid_user(bag.data(), state).is_none() {
            let data = bag.data_mut();
            for k in ["user", "iat", "exp"] {
                data.shift_remove(k);
            }
        }
        Ctx {
            bag,
            https: https(headers, &state.settings),
        }
    }

    /// `open`, then the device's rewrite of any valid session that is not the local user's.
    pub fn open_rewritten(headers: &HeaderMap, state: &AppState) -> Ctx {
        let mut ctx = Ctx::open(headers, state);
        if state.settings.local_user
            && let Some(user) = ctx.user(state)
            && user.get("local") != Some(&Value::Bool(true))
        {
            ctx.start(local_user());
        }
        ctx
    }

    /// `auth.sessionUser(req)`.
    pub fn user(&self, state: &AppState) -> Option<Map<String, Value>> {
        valid_user(self.bag.data(), state)
    }

    /// `startSession(req, user)`: stamped now, with no kept token.
    pub fn start(&mut self, user: Map<String, Value>) {
        let now = now_ms();
        let data = self.bag.data_mut();
        data.insert("user".into(), Value::Object(user));
        data.insert("iat".into(), k7_auth::js_number(now));
        data.insert("exp".into(), k7_auth::js_number(now + SESSION_MAX_AGE_MS));
        data.shift_remove("kt");
    }

    /// The answer with the session's `Set-Cookie` lines, as cookie-session writes them.
    pub fn finish(&self, mut response: Response, state: &AppState) -> Response {
        let options = CookieOptions {
            name: COOKIE_NAME,
            secret: &state.settings.session_secret,
            secure_request: self.https,
            max_age_ms: SESSION_MAX_AGE_MS,
        };
        for line in self.bag.set_cookie_headers(&options, now_ms()) {
            if let Ok(v) = HeaderValue::from_str(&line) {
                response.headers_mut().append(SET_COOKIE, v);
            }
        }
        response
    }
}
