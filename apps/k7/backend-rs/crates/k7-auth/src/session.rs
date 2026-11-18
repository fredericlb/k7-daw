//! session.rs — the session payload: base64 of `JSON.stringify({user, iat, exp, _expire?})`, and the
//! rules that make a signed one still valid. A signature alone proves nothing about time (`apps/k7/backend-rs/docs/sessions.md`).

use base64::Engine;
use base64::alphabet::STANDARD;
use base64::engine::DecodePaddingMode;
use base64::engine::general_purpose::{GeneralPurpose, GeneralPurposeConfig};
use serde_json::{Map, Value};

/// How long a session lives from its sign-in, whatever it carries.
pub const SESSION_MAX_AGE_MS: f64 = 7.0 * 24.0 * 60.0 * 60.0 * 1000.0;
/// How long a session that carries groups lives.
pub const GROUPS_MAX_AGE_MS: f64 = 12.0 * 60.0 * 60.0 * 1000.0;
const SKEW_MS: f64 = 60_000.0;

/// Node's base64 decoder takes either alphabet and either padding; this one follows it.
pub(crate) const LENIENT: GeneralPurpose = GeneralPurpose::new(
    &STANDARD,
    GeneralPurposeConfig::new()
        .with_decode_padding_mode(DecodePaddingMode::Indifferent)
        .with_decode_allow_trailing_bits(true),
);

/// A decoded session: the JSON object the cookie carries, keys in the order they were written.
pub type Session = Map<String, Value>;

/// The cookie value for `session`: what cookie-session's encode gives for the same object.
pub fn seal(session: &Session) -> String {
    let json = Value::Object(session.clone()).to_string();
    LENIENT.encode(json.as_bytes())
}

/// The session in a cookie value whose signature holds, or None.
pub fn open(cookie_name: &str, value: &str, signature: &str, secret: &str) -> Option<Session> {
    if !crate::verify(&format!("{cookie_name}={value}"), signature, secret) {
        return None;
    }
    let normalized: String = value
        .chars()
        .map(|c| match c {
            '-' => '+',
            '_' => '/',
            c => c,
        })
        .collect();
    let bytes = LENIENT.decode(normalized.trim()).ok()?;
    match serde_json::from_slice(&bytes).ok()? {
        Value::Object(map) => Some(map),
        _ => None,
    }
}

/// The session's user when its stamps are all good at `now` (ms), else None; `not_before` is the revocation epoch.
pub fn valid_session_user(
    session: &Session,
    now: f64,
    groups_max_age_ms: f64,
    not_before: impl Fn(&str) -> Option<f64>,
) -> Option<Map<String, Value>> {
    if let Some(expire) = session.get("_expire").and_then(Value::as_f64)
        && now > expire
    {
        return None;
    }
    let Some(Value::Object(user)) = session.get("user") else {
        return None;
    };
    let iat = session.get("iat").and_then(Value::as_f64)?;
    let exp = session.get("exp").and_then(Value::as_f64)?;
    if now >= exp || iat > now + SKEW_MS {
        return None;
    }
    if let Some(Value::Array(groups)) = user.get("groups")
        && !groups.is_empty()
        && now >= iat + groups_max_age_ms
    {
        return None;
    }
    if let Some(Value::String(sub)) = user.get("sub")
        && let Some(epoch) = not_before(sub)
        && iat < epoch
    {
        return None;
    }
    Some(user.clone())
}

/// The Cookie header pairs for `user` signed in at `iat`, as `sessionCookieHeader` writes them.
pub fn cookie_header(
    user: Map<String, Value>,
    cookie_name: &str,
    secret: &str,
    iat: f64,
) -> String {
    let mut session = Session::new();
    session.insert("user".into(), Value::Object(user));
    session.insert("iat".into(), crate::js_number(iat));
    session.insert("exp".into(), crate::js_number(iat + SESSION_MAX_AGE_MS));
    let value = seal(&session);
    let sig = crate::sign(&format!("{cookie_name}={value}"), secret);
    format!("{cookie_name}={value}; {cookie_name}.sig={sig}")
}

/// A Cookie header's pairs, the first of each name winning, values percent-decoded when they decode.
pub fn parse_cookie_header(header: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for part in header.split(';') {
        let Some(eq) = part.find('=') else { continue };
        let name = part[..eq].trim();
        if name.is_empty() || out.iter().any(|(n, _)| n == name) {
            continue;
        }
        let raw = part[eq + 1..].trim();
        out.push((
            name.to_string(),
            percent_decode(raw).unwrap_or_else(|| raw.to_string()),
        ));
    }
    out
}

/// decodeURIComponent: None where it would throw (a malformed escape or bytes that are not UTF-8).
fn percent_decode(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = s.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}
