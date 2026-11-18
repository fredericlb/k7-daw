//! auth_guard.rs — `apps/k7/backend/src/authGuard.ts`'s refusal, ported word for word: a host whose sign-in
//! would fall back on the dev stub, which signs ANY visitor in as one shared "Dev User", or whose session
//! secret is guessable, must not start. Checked by every host's start (`Config::from_env`, `embedded::start`).

use std::collections::BTreeMap;

/// The literal @studio/oidc's `setupAuth` signs with when `SESSION_SECRET` is unset.
pub const DEFAULT_SESSION_SECRET: &str = "dev-insecure-secret-change-me";
/// Shorter is guessable; the tray's and the phones' generated secret is 64 hex characters.
const MIN_SESSION_SECRET: usize = 32;

/// The line node prints after the reasons, before it exits.
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
    let set = |k: &str| env.get(k).is_some_and(|v| !v.is_empty());
    let mut reasons = Vec::new();
    if !set("OIDC_ISSUER") || !set("OIDC_CLIENT_ID") {
        reasons.push("OIDC_ISSUER and OIDC_CLIENT_ID must both be set, or every visitor is signed in as the dev user".to_string());
    }
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

/// The refusal as node prints it, `FATAL: ` before each line; None when the environment may start.
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

    /// The cases of `authGuard.test.ts`.
    #[test]
    fn refuses_as_node_does() {
        let good = "0123456789abcdef0123456789abcdef";
        assert!(
            auth_refusals(&env(&[
                ("OIDC_ISSUER", "https://id"),
                ("OIDC_CLIENT_ID", "k7"),
                ("SESSION_SECRET", good)
            ]))
            .is_empty()
        );
        assert!(auth_refusals(&env(&[("STUDIO_DEV_AUTH", "1")])).is_empty());
        assert!(auth_refusals(&env(&[("STUDIO_LOCAL_USER", "1")])).is_empty());
        assert_eq!(
            auth_refusals(&env(&[])),
            [
                "OIDC_ISSUER and OIDC_CLIENT_ID must both be set, or every visitor is signed in as the dev user",
                "SESSION_SECRET must be at least 32 characters (it is 0)",
            ]
        );
        assert_eq!(
            auth_refusals(&env(&[
                ("OIDC_ISSUER", "https://id"),
                ("OIDC_CLIENT_ID", "k7"),
                ("SESSION_SECRET", DEFAULT_SESSION_SECRET)
            ])),
            ["SESSION_SECRET is the public development default"]
        );
        assert_eq!(
            auth_refusals(&env(&[
                ("OIDC_ISSUER", "https://id"),
                ("SESSION_SECRET", good)
            ]))
            .len(),
            1
        );
        assert!(auth_refusals(&env(&[("STUDIO_DEV_AUTH", "0")])).len() == 2);
        let r = refusal(&env(&[])).unwrap();
        assert!(r.ends_with(
            "FATAL: refusing to start; set STUDIO_DEV_AUTH=1 only for a local dev session"
        ));
    }
}
