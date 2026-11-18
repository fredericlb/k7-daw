//! seam.rs — the hub's side of the engine seam (`packages/engine-registry/seamClient.ts`): the registry's view of an
//! engine, the tunnel hub's state, and loopback bindings, each an HTTP call to `/api/internal/engine` at the front.
//! What a reader gets wrong: a KEPT binding is re-posted under its own key when the tunnel hub's epoch moves on.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use hyper::Method;
use serde_json::{Map, Value};

use crate::Log;

/// How soon a hub that restarted gets the bindings back.
const HUB_WATCH: Duration = Duration::from_secs(2);
/// How long a channel's upgrade may take before it counts as no channel.
pub const OPEN: Duration = Duration::from_secs(10);
const CALL: Duration = Duration::from_secs(15);

/// `encodeURIComponent`.
pub fn encode_uri_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[derive(Clone)]
struct Held {
    engine_id: Option<String>,
    roots: Value,
    kind: String,
    synced: Option<bool>,
    epoch: Option<String>,
}

/// A WebSocket to an engine through a one-shot binding.
pub type Channel =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

pub struct Seam {
    base: String,
    secret: String,
    held: Mutex<HashMap<String, Held>>,
    watching: AtomicBool,
    log: Log,
}

fn key_of(path: &str) -> String {
    path.rsplit('/').next().unwrap_or("").to_string()
}

impl Seam {
    /// The seam at `http://<addr>`, proved by `secret`.
    pub fn new(addr: &str, secret: &str, log: Log) -> Arc<Seam> {
        Arc::new(Seam {
            base: format!("http://{addr}"),
            secret: secret.to_string(),
            held: Mutex::new(HashMap::new()),
            watching: AtomicBool::new(false),
            log,
        })
    }

    fn held(&self) -> std::sync::MutexGuard<'_, HashMap<String, Held>> {
        self.held.lock().unwrap_or_else(|p| p.into_inner())
    }

    async fn call(
        &self,
        method: Method,
        path: &str,
        body: Option<&Value>,
    ) -> Result<(u16, Value), String> {
        let url = format!("{}/api/internal/engine{path}", self.base);
        crate::http::call(method, &url, &[("x-k7-internal", &self.secret)], body, CALL)
            .await
            .map_err(|e| format!("the engine seam did not answer: {e}"))
    }

    fn json(what: &str, (status, v): (u16, Value)) -> Result<Value, String> {
        if (200..300).contains(&status) {
            return Ok(v);
        }
        let said = v.get("error").and_then(Value::as_str).unwrap_or("");
        Err(format!("the engine seam refused {what} ({status}): {said}"))
    }

    /// Engine `id` as `holder` sees it, or None when no engine has that id.
    pub async fn engine(&self, id: &str, holder: &str) -> Result<Option<Value>, String> {
        let path = format!(
            "/engines/{}?holder={}",
            encode_uri_component(id),
            encode_uri_component(holder)
        );
        let r = self.call(Method::GET, &path, None).await?;
        if r.0 == 404 {
            return Ok(None);
        }
        Self::json("an engine", r).map(Some)
    }

    /// What the tunnel hub says of the render machine, and its epoch.
    pub async fn hub(&self) -> Result<Value, String> {
        Self::json("the hub", self.call(Method::GET, "/hub", None).await?)
    }

    async fn post(&self, b: &Held, key: Option<&str>) -> Result<(String, Option<String>), String> {
        let mut body = Map::new();
        body.insert(
            "engineId".into(),
            b.engine_id.clone().map_or(Value::Null, Value::String),
        );
        body.insert("roots".into(), b.roots.clone());
        body.insert("kind".into(), b.kind.clone().into());
        if let Some(s) = b.synced {
            body.insert("synced".into(), s.into());
        }
        if let Some(k) = key {
            body.insert("key".into(), k.into());
        }
        let v = Self::json(
            "a binding",
            self.call(Method::POST, "/bindings", Some(&Value::Object(body)))
                .await?,
        )?;
        let path = v
            .get("path")
            .and_then(Value::as_str)
            .ok_or("the engine seam answered no path")?;
        Ok((
            path.to_string(),
            v.get("epoch").and_then(Value::as_str).map(str::to_string),
        ))
    }

    /// A loopback path reaching engine `engine_id`, or the render machine when None; kept until `unbind`.
    pub async fn bind(
        self: &Arc<Self>,
        engine_id: Option<&str>,
        roots: &Value,
        kind: &str,
        synced: Option<bool>,
    ) -> Result<String, String> {
        let mut b = Held {
            engine_id: engine_id.map(str::to_string),
            roots: roots.clone(),
            kind: kind.to_string(),
            synced,
            epoch: None,
        };
        let (path, epoch) = self.post(&b, None).await?;
        b.epoch = epoch;
        self.held().insert(key_of(&path), b);
        self.watch_hub();
        Ok(path)
    }

    /// Drop a path `bind` answered; dropping one twice, or one never bound, is no error.
    pub async fn unbind(&self, path: &str) -> Result<(), String> {
        let key = key_of(path);
        self.held().remove(&key);
        self.drop_key(&key).await
    }

    async fn drop_key(&self, key: &str) -> Result<(), String> {
        let (status, _) = self
            .call(
                Method::DELETE,
                &format!("/bindings/{}", encode_uri_component(key)),
                None,
            )
            .await?;
        if (200..300).contains(&status) || status == 404 {
            Ok(())
        } else {
            Err(format!("the engine seam refused an unbind ({status})"))
        }
    }

    fn watch_hub(self: &Arc<Self>) {
        if self.watching.swap(true, Ordering::Relaxed) {
            return;
        }
        let me = Arc::downgrade(self);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(HUB_WATCH).await;
                let Some(seam) = me.upgrade() else {
                    return;
                };
                if seam.held().is_empty() {
                    seam.watching.store(false, Ordering::Relaxed);
                    return;
                }
                // A hub that does not answer is restarting: the next tick asks again.
                let _ = seam.rebind().await;
            }
        });
    }

    /// Re-establish, under its own key, every kept binding a restarted hub forgot; how many were.
    pub async fn rebind(&self) -> Result<usize, String> {
        let held: Vec<(String, Held)> = self
            .held()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        if held.is_empty() {
            return Ok(0);
        }
        let epoch = self
            .hub()
            .await?
            .get("epoch")
            .and_then(Value::as_str)
            .map(str::to_string);
        let mut n = 0;
        for (key, b) in held {
            if b.epoch == epoch {
                continue;
            }
            let (_, again) = self.post(&b, Some(&key)).await?;
            let kept = match self.held().get_mut(&key) {
                Some(h) => {
                    h.epoch = again;
                    true
                }
                None => false,
            };
            // Dropped while it was re-posted: the hub must not keep it.
            if kept {
                n += 1;
            } else {
                let _ = self.drop_key(&key).await;
            }
        }
        if n > 0 {
            (self.log)(&format!(
                "the engine hub restarted: {n} loopback binding(s) re-established"
            ));
        }
        Ok(n)
    }

    /// The `ws://` address of a bound path, on the process the seam is reached at.
    pub fn ws_url(&self, path: &str) -> String {
        format!("{}{path}", self.base.replacen("http", "ws", 1))
    }

    /// A socket to engine `engine_id` through a one-shot binding, or None when it has no tunnel or the upgrade fails.
    pub async fn open(
        &self,
        engine_id: Option<&str>,
        roots: &Value,
        kind: &str,
        timeout: Duration,
    ) -> Option<Channel> {
        let b = Held {
            engine_id: engine_id.map(str::to_string),
            roots: roots.clone(),
            kind: kind.to_string(),
            synced: None,
            epoch: None,
        };
        let (path, _) = self.post(&b, None).await.ok()?;
        let ws = tokio::time::timeout(
            timeout,
            tokio_tungstenite::connect_async(self.ws_url(&path)),
        )
        .await;
        // The hub read the binding at the upgrade; it serves nobody after.
        let _ = self.drop_key(&key_of(&path)).await;
        ws.ok()?.ok().map(|(s, _)| s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_as_javascript_does() {
        assert_eq!(encode_uri_component("a b/é~*'()!"), "a%20b%2F%C3%A9~*'()!");
    }
}
