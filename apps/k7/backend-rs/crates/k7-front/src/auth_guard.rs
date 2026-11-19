//! auth_guard.rs — a host that signs no one in as a local user, and is no dev session, must hold a session secret
//! nobody can guess; it is refused at start otherwise. Checked by every host's start (`Config::from_env`,
//! `embedded::start`). What a reader gets wrong: no sign-in provider exists, so a secret is the only thing checked.

use std::collections::BTreeMap;

/// The public development default a session secret must never be.
pub const DEFAULT_SESSION_SECRET: &str = "dev-insecure-secret-change-me";
/// Shorter is guessable; the tray's and the phones' generated secret is 64 hex characters.
const MIN_SESSION_SECRET: usize = 32;

/// The line printed after the reasons, before the start is refused.
pub const REFUSING: &str = "refusing to start; set STUDIO_DEV_AUTH=1 only for a local dev session";

/// A device's own backend and an explicit dev session are the two that may run the stub.
pub fn dev_stub_allowed(env: &BTreeMap<String, String>) -> bool {
    env.get("STUDIO_LOCAL_USER").is_some_and(|v| v == "1")
        || env.get("STUDIO_DEV_AUTH").is_some_and(|v| v == "1")
}

/// Why this environment must not start, one line per reason; empty means it may.
pub fn auth_refusals(env: &BTreeMap<String, String>) -> Vec<String> {
    if dev_stub_allowed(env) {
        return Vec::new();
    }
    let mut reasons = Vec::new();
    let secret = env.get("SESSION_SECRET").map_or("", String::as_str);
    if secret == DEFAULT_SESSION_SECRET {
        reasons.push("SESSION_SECRET is the public development default".to_string());
    } else if secret.encode_utf16().count() < MIN_SESSION_SECRET {
        reasons.push(format!(
            "SESSION_SECRET must be at least {MIN_SESSION_SECRET} characters (it is {})",
            secret.encode_utf16().count()
        ));
    }
    reasons
}

/// The refusal as printed, `FATAL: ` before each line; None when the environment may start.
pub fn refusal(env: &BTreeMap<String, String>) -> Option<String> {
    let reasons = auth_refusals(env);
    if reasons.is_empty() {
        return None;
    }
    let mut lines: Vec<String> = reasons.iter().map(|r| format!("FATAL: {r}")).collect();
    lines.push(format!("FATAL: {REFUSING}"));
    Some(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn refuses_a_guessable_secret_unless_a_device_or_a_dev_session() {
        let good = "0123456789abcdef0123456789abcdef";
        assert!(auth_refusals(&env(&[("SESSION_SECRET", good)])).is_empty());
        assert!(auth_refusals(&env(&[("STUDIO_DEV_AUTH", "1")])).is_empty());
        assert!(auth_refusals(&env(&[("STUDIO_LOCAL_USER", "1")])).is_empty());
        assert_eq!(
            auth_refusals(&env(&[])),
            ["SESSION_SECRET must be at least 32 characters (it is 0)"]
        );
        assert_eq!(
            auth_refusals(&env(&[("SESSION_SECRET", DEFAULT_SESSION_SECRET)])),
            ["SESSION_SECRET is the public development default"]
        );
        assert_eq!(auth_refusals(&env(&[("STUDIO_DEV_AUTH", "0")])).len(), 1);
        let r = refusal(&env(&[])).unwrap();
        assert!(r.ends_with(
            "FATAL: refusing to start; set STUDIO_DEV_AUTH=1 only for a local dev session"
        ));
    }
}
