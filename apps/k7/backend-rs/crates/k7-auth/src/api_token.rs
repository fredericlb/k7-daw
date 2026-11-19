//! api_token.rs — the signed bearers the hub hands its services: `payload.signature`, an HMAC-SHA256 over
//! `{v: 2, jti, sub, iat, exp}`. What a reader gets wrong: the signature is not the whole check; the hub
//! accepts a bearer only while it holds its id for its subject (`hub.rs`'s `verify_service_token`).

use base64::Engine;
use base64::engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use serde_json::Value;
use sha2::Sha256;
use subtle::ConstantTimeEq;

/// The token signature of `payload` (its base64url text) under `secret`.
pub fn sign(secret: &str, payload: &str) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC takes any key");
    mac.update(payload.as_bytes());
    URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
}

/// What a well-signed token claims, before any store or clock is asked.
#[derive(Debug, PartialEq)]
pub struct Claims {
    pub jti: String,
    pub sub: String,
    pub exp: f64,
}

/// The claims of a token signed with `secret` in the current format, or None.
pub fn claims(secret: &str, token: &str) -> Option<Claims> {
    let mut parts = token.split('.');
    let (payload, sig) = (parts.next()?, parts.next()?);
    if payload.is_empty() || sig.is_empty() || parts.next().is_some() {
        return None;
    }
    let want = sign(secret, payload);
    if want.len() != sig.len() || !bool::from(want.as_bytes().ct_eq(sig.as_bytes())) {
        return None;
    }
    let bytes = URL_SAFE
        .decode(payload.trim_end_matches('='))
        .or_else(|_| URL_SAFE_NO_PAD.decode(payload))
        .ok()?;
    let body: Value = serde_json::from_slice(&bytes).ok()?;
    if body.get("v").and_then(Value::as_f64) != Some(2.0) {
        return None;
    }
    Some(Claims {
        jti: body.get("jti")?.as_str()?.to_string(),
        sub: body.get("sub")?.as_str()?.to_string(),
        exp: body.get("exp")?.as_f64()?,
    })
}
