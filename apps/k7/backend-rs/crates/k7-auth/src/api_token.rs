//! api_token.rs — the personal access tokens of `packages/oidc/apiToken.ts`: `payload.signature`, an
//! HMAC-SHA256 over `{v: 2, jti, sub, iat, exp}`, listed by id in a JSON file the TypeScript backend shares.
//! What a reader gets wrong: the signature is not the whole check; a token counts only while its id is listed
//! for its subject, and every write re-reads the file first (`packages/oidc/docs/apiTokens.md`).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use base64::Engine;
use base64::engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use serde_json::{Map, Value, json};
use sha2::Sha256;
use subtle::ConstantTimeEq;

use crate::json_file::{SharedJsonFile, js_number};

/// A token's default lifetime: 90 days.
pub const API_TOKEN_TTL_MS: f64 = 90.0 * 24.0 * 60.0 * 60.0 * 1000.0;

/// The token signature of `payload` (its base64url text) under `secret`.
pub fn sign(secret: &str, payload: &str) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC takes any key");
    mac.update(payload.as_bytes());
    URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
}

/// A new token for `sub`, and the record its store lists: `jti` is the caller's 16 random bytes, base64url.
pub fn mint(
    secret: &str,
    sub: &str,
    label: &str,
    jti: &str,
    now: f64,
    ttl_ms: f64,
) -> (String, Map<String, Value>) {
    let exp = now + ttl_ms;
    let body =
        json!({ "v": 2, "jti": jti, "sub": sub, "iat": js_number(now), "exp": js_number(exp) });
    let payload = URL_SAFE_NO_PAD.encode(body.to_string());
    let token = format!("{payload}.{}", sign(secret, &payload));
    let record = json!({
        "id": jti, "sub": sub, "label": label, "created": js_number(now), "expires": js_number(exp)
    });
    (token, record.as_object().cloned().unwrap_or_default())
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

/// `library-tokens.json`: the records, in the file's order, each id once with its last record.
pub struct TokenFile {
    file: SharedJsonFile,
    /// Last uses not written yet, laid over the file, and when the first of them was noted.
    used: Mutex<(HashMap<String, f64>, Option<f64>)>,
    /// Held across a read-modify-write, so two threads of this process never lose one another's change.
    writing: Mutex<()>,
}

fn str_field<'a>(r: &'a Map<String, Value>, k: &str) -> Option<&'a str> {
    r.get(k).and_then(Value::as_str)
}

fn expires(r: &Map<String, Value>) -> Option<f64> {
    r.get("expires").and_then(Value::as_f64)
}

impl TokenFile {
    pub fn new(file: PathBuf) -> Self {
        TokenFile {
            file: SharedJsonFile::new(file),
            used: Mutex::new((HashMap::new(), None)),
            writing: Mutex::new(()),
        }
    }

    fn used(&self) -> MutexGuard<'_, (HashMap<String, f64>, Option<f64>)> {
        self.used.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// The records as `fileTokenStore` maps them, the last uses not written yet laid over them.
    pub fn records(&self) -> Vec<Map<String, Value>> {
        let mut out = self.listed();
        let used = self.used();
        for r in &mut out {
            let at = str_field(r, "id").and_then(|id| used.0.get(id)).copied();
            let before = r.get("lastUsed").and_then(Value::as_f64).unwrap_or(0.0);
            if let Some(at) = at.filter(|at| before < *at) {
                r.insert("lastUsed".into(), js_number(at));
            }
        }
        out
    }

    fn listed(&self) -> Vec<Map<String, Value>> {
        let mut out: Vec<Map<String, Value>> = Vec::new();
        if let Value::Array(list) = &*self.file.read() {
            for r in list.iter().filter_map(Value::as_object) {
                let Some(id) = str_field(r, "id") else {
                    continue;
                };
                match out.iter_mut().find(|o| str_field(o, "id") == Some(id)) {
                    Some(slot) => *slot = r.clone(),
                    None => out.push(r.clone()),
                }
            }
        }
        out
    }

    pub fn get(&self, id: &str) -> Option<Map<String, Value>> {
        self.records()
            .into_iter()
            .find(|r| str_field(r, "id") == Some(id))
    }

    /// `sub`'s records that have not expired at `now`.
    pub fn list(&self, sub: &str, now: f64) -> Vec<Map<String, Value>> {
        self.records()
            .into_iter()
            .filter(|r| str_field(r, "sub") == Some(sub) && expires(r).is_some_and(|e| e > now))
            .collect()
    }

    /// Re-reads the file, applies one change, drops what expired, and writes it back.
    fn change(
        &self,
        now: f64,
        apply: impl FnOnce(&mut Vec<Map<String, Value>>),
    ) -> std::io::Result<()> {
        let _turn = self.writing.lock().unwrap_or_else(|p| p.into_inner());
        let mut all = self.records();
        apply(&mut all);
        let kept: Vec<Value> = all
            .into_iter()
            .filter(|r| expires(r).is_some_and(|e| e > now))
            .map(Value::Object)
            .collect();
        let written = self.file.write(&Value::Array(kept));
        *self.used() = (HashMap::new(), None);
        written
    }

    /// Note a token's use at `at`, written with the next change or by `flush_due`.
    pub fn touch(&self, id: &str, at: f64) {
        if self.get(id).is_none() {
            return;
        }
        let mut used = self.used();
        used.0.insert(id.to_string(), at);
        used.1.get_or_insert(at);
    }

    /// Write the noted uses once the first of them is `after_ms` old, as `fileTokenStore`'s timer does.
    pub fn flush_due(&self, now: f64, after_ms: f64) -> std::io::Result<()> {
        let due = self.used().1.is_some_and(|first| now - first >= after_ms);
        if due {
            self.change(now, |_| {})
        } else {
            Ok(())
        }
    }

    /// `add`, in the same write dropping `sub`'s other records labelled `label`.
    pub fn add_replacing(&self, record: Map<String, Value>, now: f64) -> std::io::Result<()> {
        let (sub, label) = (str_field(&record, "sub"), str_field(&record, "label"));
        let (sub, label) = (sub.map(str::to_string), label.map(str::to_string));
        self.change(now, |all| {
            all.retain(|o| {
                str_field(o, "sub") != sub.as_deref() || str_field(o, "label") != label.as_deref()
            });
            all.push(record);
        })
    }

    pub fn add(&self, record: Map<String, Value>, now: f64) -> std::io::Result<()> {
        self.change(now, |all| {
            let id = str_field(&record, "id").map(str::to_string);
            match all
                .iter_mut()
                .find(|o| str_field(o, "id").map(str::to_string) == id)
            {
                Some(slot) => *slot = record,
                None => all.push(record),
            }
        })
    }

    /// Revokes `sub`'s token `id`; false when it is not listed or not theirs.
    pub fn revoke(&self, sub: &str, id: &str, now: f64) -> std::io::Result<bool> {
        match self.get(id) {
            Some(r) if str_field(&r, "sub") == Some(sub) => {
                self.change(now, |all| all.retain(|o| str_field(o, "id") != Some(id)))?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    /// The subject of a listed, unexpired token signed with `secret`; an ephemeral token is another process's.
    pub fn verify(&self, secret: &str, token: &str, now: f64) -> Option<String> {
        let c = claims(secret, token)?;
        if c.exp == 0.0 || c.exp <= now {
            return None;
        }
        let r = self.get(&c.jti)?;
        (str_field(&r, "sub") == Some(c.sub.as_str()) && expires(&r).is_some_and(|e| e > now))
            .then_some(c.sub)
    }

    /// `verify`, then the token's use noted, as @studio/oidc's `verify` touches its store.
    pub fn verify_and_touch(&self, secret: &str, token: &str, now: f64) -> Option<String> {
        let sub = self.verify(secret, token, now)?;
        if let Some(c) = claims(secret, token) {
            self.touch(&c.jti, now);
        }
        Some(sub)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mints_lists_verifies_and_revokes() {
        let dir = std::env::temp_dir().join(format!("k7-auth-tok-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let file = TokenFile::new(dir.join("library-tokens.json"));
        let (token, record) = mint("k", "ann", "cli", "abc", 1000.0, 50.0);
        assert!(file.verify("k", &token, 1001.0).is_none(), "not listed yet");
        file.add(record, 1000.0).unwrap();
        assert_eq!(file.verify("k", &token, 1001.0).as_deref(), Some("ann"));
        assert!(file.verify("other", &token, 1001.0).is_none());
        assert!(file.verify("k", &token, 1050.0).is_none(), "expired");
        assert_eq!(file.list("ann", 1001.0).len(), 1);
        assert!(!file.revoke("bob", "abc", 1001.0).unwrap());
        assert!(file.revoke("ann", "abc", 1001.0).unwrap());
        assert!(file.verify("k", &token, 1001.0).is_none());
        assert!(claims("k", &format!("{token}.x")).is_none());
        let (token, record) = mint("k", "ann", "cli", "def", 1000.0, 50.0);
        file.add(record, 1000.0).unwrap();
        assert_eq!(
            file.verify_and_touch("k", &token, 1010.0).as_deref(),
            Some("ann")
        );
        assert_eq!(file.get("def").unwrap()["lastUsed"], json!(1010));
        file.flush_due(1020.0, 60.0).unwrap();
        assert!(
            !std::fs::read_to_string(dir.join("library-tokens.json"))
                .unwrap()
                .contains("lastUsed")
        );
        file.flush_due(1040.0, 30.0).unwrap();
        assert!(
            std::fs::read_to_string(dir.join("library-tokens.json"))
                .unwrap()
                .contains("\"lastUsed\": 1010")
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn concurrent_adds_lose_none() {
        let dir = std::env::temp_dir().join(format!("k7-auth-race-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let file = std::sync::Arc::new(TokenFile::new(dir.join("api-tokens.json")));
        let threads: Vec<_> = (0..8)
            .map(|t| {
                let file = file.clone();
                std::thread::spawn(move || {
                    for i in 0..10 {
                        let (_, r) = mint(
                            "k",
                            &format!("u{t}"),
                            "svc",
                            &format!("{t}-{i}"),
                            1000.0,
                            5e6,
                        );
                        file.add_replacing(r, 1000.0).unwrap();
                    }
                })
            })
            .collect();
        for t in threads {
            t.join().unwrap();
        }
        for t in 0..8 {
            assert_eq!(
                file.list(&format!("u{t}"), 1001.0).len(),
                1,
                "u{t} keeps its last token"
            );
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
