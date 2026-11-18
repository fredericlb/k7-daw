//! session.rs — `requireAuth` for the routes the front owns: the `studio.sid` cookie through `k7-auth`, the
//! revocations both processes write (re-read when their file changes, since a logout everywhere may happen
//! in either), and a device's rewrite of any other session to its local user. A session refused for its
//! stamps answers 401 without the cleared cookie the backend would also send (`docs/front.md`).

use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::response::Response;
use hyper::HeaderMap;
use hyper::header::{COOKIE, HeaderValue, SET_COOKIE};
use k7_auth::Revocations;
use serde_json::{Map, Value, json};

use crate::settings::Settings;

pub const COOKIE_NAME: &str = "studio.sid";

/// A signed-in request's user, and the cookies to send back when the front started a session.
#[derive(Clone)]
pub struct Signed {
    pub user: Map<String, Value>,
    pub set_cookies: Vec<HeaderValue>,
}

/// The per-subject "not before" epochs, `session-revocations.json` under the auth folder.
pub fn revocations(auth_dir: &Path) -> Revocations {
    Revocations::new(auth_dir.join("session-revocations.json"))
}

pub(crate) fn now_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as f64
}

/// The user a request is signed in as, under the backend's rules; None answers 401.
pub fn signed_in(
    headers: &HeaderMap,
    settings: &Settings,
    revocations: &Revocations,
    https: bool,
) -> Option<Signed> {
    let header = headers
        .get_all(COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .collect::<Vec<_>>()
        .join("; ");
    let pairs = k7_auth::parse_cookie_header(&header);
    let get = |n: &str| pairs.iter().find(|(k, _)| k == n).map(|(_, v)| v.as_str());
    let session = k7_auth::open(
        COOKIE_NAME,
        get(COOKIE_NAME)?,
        get(&format!("{COOKIE_NAME}.sig"))?,
        &settings.session_secret,
    )?;
    let user =
        k7_auth::valid_session_user(&session, now_ms(), k7_auth::GROUPS_MAX_AGE_MS, |sub| {
            revocations.not_before(sub)
        })?;
    if settings.local_user && user.get("local") != Some(&Value::Bool(true)) {
        let local = local_user();
        let set_cookies = session_cookies(local.clone(), settings, https);
        return Some(Signed {
            user: local,
            set_cookies,
        });
    }
    Some(Signed {
        user,
        set_cookies: Vec::new(),
    })
}

/// The answer, carrying the session cookies the front started, if it did.
pub fn with_cookies(mut response: Response, signed: &Signed) -> Response {
    for c in &signed.set_cookies {
        response.headers_mut().append(SET_COOKIE, c.clone());
    }
    response
}

/// Whether a trusted edge says the page is on https, which a new cookie is then marked for.
pub fn https(headers: &HeaderMap, settings: &Settings) -> bool {
    settings.trust_hops > 0
        && headers
            .get("x-forwarded-proto")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|p| p.split(',').next() == Some("https"))
}

/// `isAdminRequest`: a group of the session in `$AUTH_ADMIN_GROUPS`, case aside; the dev stub holds them all when `dev_admin`.
pub fn is_admin(user: &Map<String, Value>, settings: &Settings) -> bool {
    let admin: Vec<String> = settings
        .admin_groups
        .iter()
        .map(|g| g.to_lowercase())
        .collect();
    if admin.is_empty() {
        return false;
    }
    if settings.dev_admin && user.get("sub").and_then(Value::as_str) == Some("dev-user") {
        return true;
    }
    user.get("groups")
        .and_then(Value::as_array)
        .is_some_and(|gs| {
            gs.iter()
                .filter_map(Value::as_str)
                .any(|g| admin.contains(&g.to_lowercase()))
        })
}

/// The signed-in user of a request, `signed_in` with the https the edge reports.
pub fn signed(
    headers: &HeaderMap,
    settings: &Settings,
    revocations: &Revocations,
) -> Option<Signed> {
    signed_in(headers, settings, revocations, https(headers, settings))
}

/// The stub email of a device's one account and of the dev sign-in: nobody's real address.
pub const LOCAL_EMAIL: &str = "dev@example.com";

/// A device's one account, as `apps/k7/backend/src/localUser.ts` names it.
pub fn local_user() -> Map<String, Value> {
    json!({ "sub": "dev-user", "name": "Local user", "email": LOCAL_EMAIL, "groups": [], "local": true })
        .as_object()
        .cloned()
        .unwrap_or_default()
}

/// The two `Set-Cookie` values cookie-session sends for a new session of `user`.
fn session_cookies(user: Map<String, Value>, settings: &Settings, https: bool) -> Vec<HeaderValue> {
    let header = k7_auth::cookie_header(user, COOKIE_NAME, &settings.session_secret, now_ms());
    let expires = httpdate::fmt_http_date(
        SystemTime::now() + Duration::from_millis(k7_auth::SESSION_MAX_AGE_MS as u64),
    );
    let attributes = if https {
        format!("; path=/; expires={expires}; samesite=lax; secure; httponly")
    } else {
        format!("; path=/; expires={expires}; samesite=lax; httponly")
    };
    header
        .split("; ")
        .filter_map(|pair| HeaderValue::from_str(&format!("{pair}{attributes}")).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn settings(local: bool) -> Settings {
        let mut env = BTreeMap::new();
        env.insert("SESSION_SECRET".to_string(), "s".to_string());
        if local {
            env.insert("STUDIO_LOCAL_USER".to_string(), "1".to_string());
        }
        Settings::new(&env, 0)
    }

    fn with_cookie(user: Value) -> HeaderMap {
        let header = k7_auth::cookie_header(
            user.as_object().unwrap().clone(),
            COOKIE_NAME,
            "s",
            now_ms(),
        );
        let mut h = HeaderMap::new();
        h.insert(COOKIE, HeaderValue::from_str(&header).unwrap());
        h
    }

    #[test]
    fn reads_a_session_and_refuses_none() {
        let dir = std::env::temp_dir().join("k7-front-session-test-none");
        let r = revocations(&dir);
        let s = settings(false);
        assert!(signed_in(&HeaderMap::new(), &s, &r, false).is_none());
        let signed = signed_in(
            &with_cookie(json!({ "sub": "a", "email": "a@b" })),
            &s,
            &r,
            false,
        )
        .unwrap();
        assert_eq!(signed.user["sub"], "a");
        assert!(signed.set_cookies.is_empty());
    }

    #[test]
    fn a_revocation_written_later_is_seen() {
        let dir = std::env::temp_dir().join(format!("k7-front-revoke-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let r = revocations(&dir);
        let s = settings(false);
        let h = with_cookie(json!({ "sub": "a" }));
        assert!(signed_in(&h, &s, &r, false).is_some());
        std::fs::write(
            dir.join("session-revocations.json"),
            format!("{{\"a\": {}}}", now_ms() + 10_000.0),
        )
        .unwrap();
        assert!(signed_in(&h, &s, &r, false).is_none());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_device_turns_any_session_into_its_local_user() {
        let r = revocations(&std::env::temp_dir().join("k7-front-session-test-local"));
        let signed = signed_in(
            &with_cookie(json!({ "sub": "someone" })),
            &settings(true),
            &r,
            false,
        )
        .unwrap();
        assert_eq!(signed.user["sub"], "dev-user");
        assert_eq!(signed.set_cookies.len(), 2);
        let first = signed.set_cookies[0].to_str().unwrap();
        assert!(
            first.starts_with("studio.sid=")
                && first.contains("samesite=lax; httponly")
                && !first.contains("secure")
        );
    }

    #[test]
    fn a_device_as_the_tray_starts_it_has_no_admin() {
        let r = settings(true);
        assert!(r.admin_groups.is_empty() && !r.dev_admin);
        assert!(!is_admin(&local_user(), &r));
    }
}
