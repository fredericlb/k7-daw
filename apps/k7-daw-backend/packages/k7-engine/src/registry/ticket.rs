//! ticket.rs — the relay ticket, as `/engines` mints it: `<b64url payload>.<b64url hmac>`, the payload
//! `{"id","h","exp"}`, signed by a secret drawn per PROCESS and never configured. A ticket names one engine
//! and one account for a minute (`apps/k7-daw-backend/docs/engine/proxy.md`, "The ticket").

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hmac::{Hmac, Mac};
use serde_json::{Value, json};
use sha2::Sha256;
use subtle::ConstantTimeEq;

/// A ticket only needs to survive the handshake it was minted for.
pub const TICKET_TTL_MS: i64 = 60_000;

/// What a verified ticket is good for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ticket {
    pub id: String,
    pub holder: String,
}

pub struct Tickets {
    secret: [u8; 32],
}

impl Default for Tickets {
    fn default() -> Self {
        let mut secret = [0u8; 32];
        getrandom::fill(&mut secret).expect("the system's random source");
        Tickets { secret }
    }
}

impl Tickets {
    fn sign(&self, payload: &str) -> String {
        let mut mac = Hmac::<Sha256>::new_from_slice(&self.secret).expect("any key length");
        mac.update(payload.as_bytes());
        URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
    }

    /// A ticket for engine `id` and account `holder`, good until `now_ms` plus a minute.
    pub fn issue(&self, id: &str, holder: &str, now_ms: i64) -> String {
        let body = json!({ "id": id, "h": holder, "exp": now_ms + TICKET_TTL_MS });
        let payload = URL_SAFE_NO_PAD.encode(body.to_string());
        let sig = self.sign(&payload);
        format!("{payload}.{sig}")
    }

    /// The engine and account a ticket is good for at `now_ms`, or None when forged, expired or malformed.
    pub fn verify(&self, ticket: &str, now_ms: i64) -> Option<Ticket> {
        // `split(".")` read as its first two parts, as the TypeScript reads it.
        let mut parts = ticket.split('.');
        let (payload, sig) = (parts.next()?, parts.next()?);
        if payload.is_empty() || sig.is_empty() {
            return None;
        }
        let want = self.sign(payload);
        if sig.len() != want.len() || !bool::from(sig.as_bytes().ct_eq(want.as_bytes())) {
            return None;
        }
        let body: Value = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(payload).ok()?).ok()?;
        let id = match body.get("id")? {
            Value::String(s) if !s.is_empty() => s.clone(),
            _ => return None,
        };
        let exp = body.get("exp")?.as_f64()?;
        if exp < now_ms as f64 {
            return None;
        }
        let holder = body
            .get("h")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        Some(Ticket { id, holder })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holds_for_its_minute_to_the_millisecond() {
        let t = Tickets::default();
        let ticket = t.issue("e1", "ann@x", 1_000_000);
        let ok = Some(Ticket {
            id: "e1".into(),
            holder: "ann@x".into(),
        });
        assert_eq!(t.verify(&ticket, 1_000_000 + TICKET_TTL_MS), ok);
        assert_eq!(t.verify(&ticket, 1_000_000 + TICKET_TTL_MS + 1), None);
        assert_eq!(Tickets::default().verify(&ticket, 1_000_000), None);
    }

    #[test]
    fn a_tampered_or_malformed_one_is_nothing() {
        let t = Tickets::default();
        let ticket = t.issue("e1", "alice@x", 0);
        let (payload, sig) = ticket.split_once('.').unwrap();
        let forged = URL_SAFE_NO_PAD.encode(r#"{"id":"e1","h":"bob@x","exp":99999999999999}"#);
        for bad in [
            "",
            ".",
            "abc",
            &format!("{payload}."),
            &format!(".{sig}"),
            &format!("{sig}.{payload}"),
            &format!("{forged}.{sig}"),
            &format!("{payload}.x{sig}"),
        ] {
            assert_eq!(t.verify(bad, 0), None, "{bad}");
        }
        assert!(t.verify(&format!("{ticket}.extra"), 0).is_some());
    }
}
