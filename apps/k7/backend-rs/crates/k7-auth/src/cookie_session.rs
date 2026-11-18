//! cookie_session.rs — a request's session as cookie-session 2 and the `cookies` library hold it: found by the
//! same pattern in the raw Cookie header, rewritten only when it changed, cleared as an empty cookie dated 1970.
//! What a reader gets wrong: the `Set-Cookie` lines are part of the contract too, since a client keeps whatever
//! they say; `apps/k7/backend-rs/docs/sessions.md` lists the rules copied.

use std::time::{Duration, UNIX_EPOCH};

use base64::Engine;

use serde_json::{Map, Value};

use crate::session::{LENIENT, Session, seal};

/// What the next `Set-Cookie` lines depend on besides the session.
pub struct CookieOptions<'a> {
    pub name: &'a str,
    pub secret: &'a str,
    /// Whether the page is on https, as the backend's `req.protocol` says.
    pub secure_request: bool,
    pub max_age_ms: f64,
}

#[derive(Debug)]
enum Origin {
    /// No readable session came with the request.
    New,
    /// Read from the cookie whose value was this text.
    Read(String),
}

#[derive(Debug)]
pub struct CookieSession {
    data: Session,
    origin: Origin,
    nulled: bool,
    /// The signature did not match: `cookies` clears the `.sig` cookie.
    clear_sig: bool,
}

/// The value of cookie `name` as `cookies` finds it: the first `(?:^|;) *name=([^;]*)`, quotes dropped.
fn find(header: &str, name: &str) -> Option<String> {
    for part in header.split(';') {
        if let Some(value) = part.trim_start_matches(' ').strip_prefix(name)
            && let Some(value) = value.strip_prefix('=')
        {
            return Some(match value.strip_prefix('"') {
                Some(rest) => rest
                    .get(..rest.len().saturating_sub(1))
                    .unwrap_or("")
                    .to_string(),
                None => value.to_string(),
            });
        }
    }
    None
}

/// cookie-session's decode: node's base64, either alphabet, then JSON; anything else is no session.
fn decode(value: &str) -> Option<Session> {
    let normalized: String = value
        .chars()
        .map(|c| match c {
            '-' => '+',
            '_' => '/',
            c => c,
        })
        .collect();
    let bytes = LENIENT.decode(normalized.trim()).ok()?;
    match serde_json::from_str(&String::from_utf8_lossy(&bytes)).ok()? {
        Value::Object(map) => Some(map),
        _ => Some(Map::new()),
    }
}

impl CookieSession {
    /// The session in `header` (every Cookie header joined with "; "), checked against `secret`.
    pub fn read(header: &str, name: &str, secret: &str) -> Self {
        let mut session = CookieSession {
            data: Map::new(),
            origin: Origin::New,
            nulled: false,
            clear_sig: false,
        };
        let Some(value) = find(header, name) else {
            return session;
        };
        let Some(sig) = find(header, &format!("{name}.sig")).filter(|s| !s.is_empty()) else {
            return session;
        };
        if !crate::verify(&format!("{name}={value}"), &sig, secret) {
            session.clear_sig = true;
            return session;
        }
        if !value.is_empty()
            && let Some(data) = decode(&value)
        {
            session.data = data;
            session.origin = Origin::Read(value);
        }
        session
    }

    pub fn data(&self) -> &Session {
        &self.data
    }

    /// The session to change; after a `clear` this is a new, empty one, as `req.session ??= {}` makes it.
    pub fn data_mut(&mut self) -> &mut Session {
        if self.nulled {
            self.nulled = false;
            self.data = Map::new();
            self.origin = Origin::New;
        }
        &mut self.data
    }

    /// `req.session = null`: the cookie is cleared on the way out.
    pub fn clear(&mut self) {
        self.nulled = true;
    }

    /// The `Set-Cookie` lines cookie-session sends for this session at `now_ms`, in its order.
    pub fn set_cookie_headers(&self, o: &CookieOptions, now_ms: f64) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        if self.clear_sig {
            let secure = if o.secure_request { "; secure" } else { "" };
            out.push(format!(
                "{}.sig=; path=/; expires=Thu, 01 Jan 1970 00:00:00 GMT{secure}; httponly",
                o.name
            ));
        }
        let value = if self.nulled {
            String::new()
        } else {
            let sealed = seal(&self.data);
            let changed = match &self.origin {
                Origin::New => !self.data.is_empty(),
                Origin::Read(raw) => *raw != sealed,
            };
            if !changed {
                return out;
            }
            sealed
        };
        let expires = if self.nulled {
            "Thu, 01 Jan 1970 00:00:00 GMT".to_string()
        } else {
            httpdate::fmt_http_date(
                UNIX_EPOCH + Duration::from_millis((now_ms + o.max_age_ms) as u64),
            )
        };
        let attributes = if o.secure_request {
            format!("; path=/; expires={expires}; samesite=lax; secure; httponly")
        } else {
            format!("; path=/; expires={expires}; samesite=lax; httponly")
        };
        let pair = format!("{}={value}", o.name);
        let sig = crate::sign(&pair, o.secret);
        // Overwritten, not added to: a new `.sig` replaces the clearing one.
        out.clear();
        out.push(format!("{pair}{attributes}"));
        out.push(format!("{}.sig={sig}{attributes}", o.name));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn opts(secure: bool) -> CookieOptions<'static> {
        CookieOptions {
            name: "studio.sid",
            secret: "s",
            secure_request: secure,
            max_age_ms: 1000.0,
        }
    }

    #[test]
    fn rewrites_only_what_changed() {
        let header = crate::cookie_header(
            json!({ "sub": "a" }).as_object().unwrap().clone(),
            "studio.sid",
            "s",
            5.0,
        );
        let mut s = CookieSession::read(&format!("x=1; {header}"), "studio.sid", "s");
        assert_eq!(s.data()["user"]["sub"], "a");
        assert!(s.set_cookie_headers(&opts(false), 0.0).is_empty());
        s.data_mut().insert("note".into(), json!({ "state": "x" }));
        let set = s.set_cookie_headers(&opts(false), 0.0);
        assert_eq!(set.len(), 2);
        assert!(
            set[0].ends_with(
                "; path=/; expires=Thu, 01 Jan 1970 00:00:01 GMT; samesite=lax; httponly"
            )
        );
        assert!(set[1].starts_with("studio.sid.sig="));
        s.clear();
        let cleared = s.set_cookie_headers(&opts(true), 0.0);
        assert_eq!(
            cleared[0],
            "studio.sid=; path=/; expires=Thu, 01 Jan 1970 00:00:00 GMT; samesite=lax; secure; httponly"
        );
    }

    #[test]
    fn a_wrong_signature_clears_the_sig_cookie() {
        let s = CookieSession::read("studio.sid=e30=; studio.sid.sig=nope", "studio.sid", "s");
        assert!(s.data().is_empty());
        assert_eq!(
            s.set_cookie_headers(&opts(false), 0.0),
            vec!["studio.sid.sig=; path=/; expires=Thu, 01 Jan 1970 00:00:00 GMT; httponly"]
        );
    }
}
