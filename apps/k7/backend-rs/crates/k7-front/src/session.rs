//! session.rs — the one proof a page gives: the per-install token, as the `k7.sid` cookie. It keeps any other
//! website from calling the loopback API; it names no one, a device has one user. Choices: `docs/sessions.md`.

use std::path::Path;

use hyper::HeaderMap;
use hyper::header::{COOKIE, HeaderValue};
use serde_json::{Value, json};
use subtle::ConstantTimeEq;

use crate::settings::Settings;

pub const COOKIE_NAME: &str = "k7.sid";
/// The token's file under the auth folder.
const TOKEN_FILE: &str = "session-token";

/// The token this install answers to: the file's, else a new one written with the owner alone allowed to read it.
pub fn load_token(dir: &Path) -> String {
    let file = dir.join(TOKEN_FILE);
    if let Some(t) = std::fs::read_to_string(&file)
        .ok()
        .map(|t| t.trim().to_string())
        .filter(|t| t.len() >= 32)
    {
        return t;
    }
    let mut raw = [0u8; 32];
    getrandom::fill(&mut raw).expect("no OS randomness");
    let token: String = raw.iter().map(|b| format!("{b:02x}")).collect();
    if let Err(e) = std::fs::create_dir_all(dir).and_then(|()| write_private(&file, &token)) {
        // Without a file the token lasts as long as this process: the page is granted a new one on its next start.
        eprintln!("k7-front: the session token was not kept: {e}");
    }
    token
}

#[cfg(unix)]
fn write_private(file: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(file)?;
    f.write_all(text.as_bytes())
}

#[cfg(not(unix))]
fn write_private(file: &Path, text: &str) -> std::io::Result<()> {
    std::fs::write(file, text)
}

/// Whether a request's cookies carry the token, compared in constant time; false answers 401.
pub fn holds(headers: &HeaderMap, settings: &Settings) -> bool {
    let held = headers
        .get_all(COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|h| h.split(';'))
        .filter_map(|p| p.trim().split_once('='))
        .any(|(k, v)| {
            k == COOKIE_NAME && bool::from(v.as_bytes().ct_eq(settings.session_token.as_bytes()))
        });
    held && !settings.session_token.is_empty()
}

/// The `Set-Cookie` value that hands a page the token: no script reads it, no other site sends it.
pub fn grant(settings: &Settings) -> HeaderValue {
    HeaderValue::from_str(&format!(
        "{COOKIE_NAME}={}; Path=/; HttpOnly; SameSite=Strict; Max-Age=31536000",
        settings.session_token
    ))
    .expect("a hex token is a header value")
}

/// The stub email of a device's one account: nobody's real address, and the name its files folder is derived from.
pub const LOCAL_EMAIL: &str = "dev@example.com";
/// The subject its preferences file is named after.
pub const LOCAL_SUB: &str = "dev-user";

/// What `/auth/me` says of the device's one account.
pub fn local_user() -> Value {
    json!({ "sub": LOCAL_SUB, "name": "Local user", "email": LOCAL_EMAIL, "groups": [], "local": true })
}

pub(crate) fn now_ms() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn settings(token: &str) -> Settings {
        let mut env = BTreeMap::new();
        env.insert("K7_SESSION_TOKEN".to_string(), token.to_string());
        Settings::new(&env, 0)
    }

    fn with_cookie(value: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(COOKIE, HeaderValue::from_str(value).unwrap());
        h
    }

    #[test]
    fn only_the_installs_token_signs_in() {
        let s = settings(&"a".repeat(40));
        let token = &s.session_token;
        assert!(!holds(&HeaderMap::new(), &s));
        assert!(holds(
            &with_cookie(&format!("x=1; {COOKIE_NAME}={token}")),
            &s
        ));
        assert!(!holds(&with_cookie(&format!("{COOKIE_NAME}={token}b")), &s));
        assert!(!holds(&with_cookie(&format!("other={token}")), &s));
        let granted = grant(&s);
        let line = granted.to_str().unwrap();
        assert!(line.contains("HttpOnly") && line.contains("SameSite=Strict"));
        assert!(
            !line.contains("Secure"),
            "a device's page is http on loopback"
        );
    }

    #[test]
    fn a_token_is_kept_across_starts_and_private() {
        let dir = std::env::temp_dir().join(format!("k7-front-token-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let first = load_token(&dir);
        assert_eq!(first.len(), 64);
        assert_eq!(load_token(&dir), first, "the second start reads the file");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(dir.join(TOKEN_FILE))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o077, 0, "the owner alone reads it");
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
