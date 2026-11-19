//! ticket.rs — who is calling `/api/files`, as the hub signs it: base64url JSON `{u, exp}`,
//! a dot, its HMAC-SHA256 under the hub's key; the engine the caller drives rides after a `~`, unsigned on purpose.
//! What a reader gets wrong: `split("~", 2)` keeps the first two pieces, so a second `~` cuts the engine's id.

use base64::Engine;
use base64::engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use serde_json::Value;
use sha2::Sha256;
use subtle::ConstantTimeEq;

/// Long enough for a working session; a page that outlives it asks for another.
pub const TICKET_TTL_MS: f64 = 12.0 * 60.0 * 60_000.0;

/// What a ticket names.
#[derive(Clone, Debug, PartialEq)]
pub struct Ticket {
    pub user: String,
    pub engine_id: Option<String>,
}

fn sign(key: &[u8], payload: &str) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC takes any key");
    mac.update(payload.as_bytes());
    URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
}

/// A ticket naming `user` until `now + ttl_ms`.
pub fn mint(key: &[u8], user: &str, now: f64, ttl_ms: f64) -> String {
    let user = serde_json::to_string(user).unwrap_or_default();
    let exp = k7_js::number::to_string(now + ttl_ms);
    let payload = URL_SAFE_NO_PAD.encode(format!("{{\"u\":{user},\"exp\":{exp}}}"));
    let sig = sign(key, &payload);
    format!("{payload}.{sig}")
}

/// The user and engine a presented token names, or None when it is forged, expired or malformed.
pub fn read(key: &[u8], presented: &str, now: f64) -> Option<Ticket> {
    let mut pieces = presented.split('~');
    let ticket = pieces.next().unwrap_or("");
    let engine_id = pieces.next().filter(|e| !e.is_empty()).map(str::to_string);
    let mut parts = ticket.split('.');
    let (payload, sig) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
    if payload.is_empty() || sig.is_empty() {
        return None;
    }
    let want = sign(key, payload);
    if want.len() != sig.len() || !bool::from(want.as_bytes().ct_eq(sig.as_bytes())) {
        return None;
    }
    let bytes = URL_SAFE_NO_PAD
        .decode(payload)
        .or_else(|_| URL_SAFE.decode(payload))
        .ok()?;
    let body: Value = serde_json::from_str(&String::from_utf8_lossy(&bytes)).ok()?;
    let user = body.get("u")?.as_str().filter(|u| !u.is_empty())?;
    let exp = body.get("exp")?.as_f64()?;
    if exp < now {
        return None;
    }
    Some(Ticket {
        user: user.to_string(),
        engine_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_what_it_mints_until_it_expires() {
        let t = mint(b"k", "ann@x", 1000.0, 50.0);
        let read_at = |now| read(b"k", &t, now);
        assert_eq!(
            read_at(1050.0),
            Some(Ticket {
                user: "ann@x".into(),
                engine_id: None
            })
        );
        assert_eq!(read_at(1051.0), None);
        assert_eq!(read(b"other", &t, 1000.0), None);
        let with_engine = read(b"k", &format!("{t}~eng~more"), 1000.0).unwrap();
        assert_eq!(with_engine.engine_id.as_deref(), Some("eng"));
        assert_eq!(
            read(b"k", &format!("{t}~"), 1000.0).unwrap().engine_id,
            None
        );
        assert_eq!(read(b"k", t.split('.').next().unwrap(), 1000.0), None);
        assert_eq!(read(b"k", "not-a-ticket", 1000.0), None);
    }

    #[test]
    fn refuses_a_signed_body_with_no_user() {
        for body in [
            r#"{"u":"","exp":9e15}"#,
            r#"{"exp":9e15}"#,
            r#"{"u":"a"}"#,
            "nope",
        ] {
            let payload = URL_SAFE_NO_PAD.encode(body);
            let t = format!("{payload}.{}", sign(b"k", &payload));
            assert_eq!(read(b"k", &t, 0.0), None, "{body}");
        }
    }
}
