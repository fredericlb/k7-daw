//! ticket.rs — what opens the camera socket: a short-lived HMAC binding one account, minted behind the
//! session, because an upgrade carries no body and a page cannot set its cookie's guard on it. The secret
//! is this process's own, so only this front's tickets open this front's hub (`docs/routes.md`).

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hmac::{Hmac, Mac};
use serde_json::{Value, json};
use sha2::Sha256;
use subtle::ConstantTimeEq;

/// Long enough for the handshake it was minted for, and no longer.
const TICKET_TTL_MS: f64 = 60_000.0;

pub struct Tickets {
    secret: [u8; 32],
}

pub fn now_ms() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as f64
}

/// `n` bytes from the system's generator, as hex; a generator that fails leaves the process unable to keep a secret.
pub fn random_hex(n: usize) -> String {
    let mut bytes = vec![0u8; n];
    getrandom::fill(&mut bytes).expect("the system's random generator");
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

impl Default for Tickets {
    fn default() -> Self {
        Self::new()
    }
}

impl Tickets {
    pub fn new() -> Self {
        let mut secret = [0u8; 32];
        getrandom::fill(&mut secret).expect("the system's random generator");
        Tickets { secret }
    }

    fn sign(&self, payload: &str) -> String {
        let mut mac = Hmac::<Sha256>::new_from_slice(&self.secret).expect("any key length");
        mac.update(payload.as_bytes());
        URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
    }

    pub fn issue(&self, account: &str) -> String {
        self.issue_until(account, now_ms() + TICKET_TTL_MS)
    }

    fn issue_until(&self, account: &str, expires: f64) -> String {
        let payload = URL_SAFE_NO_PAD.encode(json!({ "a": account, "e": expires }).to_string());
        format!("{payload}.{}", self.sign(&payload))
    }

    /// The account a ticket is good for, or None when it is forged, expired or malformed.
    pub fn verify(&self, ticket: &str) -> Option<String> {
        let mut parts = ticket.split('.');
        let (payload, sig) = (parts.next()?, parts.next()?);
        if payload.is_empty() || sig.is_empty() {
            return None;
        }
        if !bool::from(sig.as_bytes().ct_eq(self.sign(payload).as_bytes())) {
            return None;
        }
        let body: Value = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(payload).ok()?).ok()?;
        let account = body.get("a")?.as_str().filter(|a| !a.is_empty())?;
        (body.get("e")?.as_f64()? >= now_ms()).then(|| account.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binds_an_account_and_refuses_a_forgery() {
        let t = Tickets::new();
        assert_eq!(t.verify(&t.issue("ana")).as_deref(), Some("ana"));
        let payload = t.issue("ana").split('.').next().unwrap().to_string();
        assert_eq!(t.verify(&format!("{payload}.x")), None);
        assert_eq!(Tickets::new().verify(&t.issue("ana")), None);
        assert_eq!(t.verify(""), None);
        assert_eq!(t.verify(&t.issue("")), None);
    }

    #[test]
    fn an_expired_ticket_is_refused() {
        let t = Tickets::new();
        assert_eq!(t.verify(&t.issue_until("ana", now_ms() - 1.0)), None);
    }
}
