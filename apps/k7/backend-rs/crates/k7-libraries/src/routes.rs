//! routes.rs — the libraries' service behind the hub's secret: `/account/libraries` (read only), `/libraries/sync` and `/status`,
//! answered as `libraryCore.ts` and `librariesRoutes.ts` do.
//! What a reader gets wrong: the caller checks who asks; the secret gate only keeps a request that is not the hub's out.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::{Value, json};
use tokio::sync::OnceCell;

use crate::list::Note;
use crate::reply::Reply;
use crate::store::Store;

/// The most a body read here may weigh: the file service's own JSON limit, 8 MB.
pub const JSON_BODY_MAX: usize = 8 * 1024 * 1024;
const ACCOUNT: &str = "/account/libraries";

/// One request on the service's paths.
pub struct Request<'a> {
    pub method: &'a str,
    /// The path under the hub, without its query.
    pub path: &'a str,
    pub query: &'a str,
    /// The hub's `x-studio-hub-secret`, as the request carried it.
    pub secret: Option<&'a str>,
    pub body: &'a [u8],
}

/// The service: the store and whether its first `ensure` settled.
pub struct Libraries {
    pub store: Arc<Store>,
    secret: String,
    first: OnceCell<Vec<Note>>,
    settled: AtomicBool,
}

fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0
}

/// The body read as `jsonBody` reads it: `{}` when empty or not JSON.
fn json_body(body: &[u8]) -> Value {
    if body.is_empty() {
        return json!({});
    }
    serde_json::from_str(&String::from_utf8_lossy(body)).unwrap_or_else(|_| json!({}))
}

fn error(status: u16, message: impl Into<String>) -> Reply {
    Reply::json(status, json!({ "error": message.into() }))
}

impl Libraries {
    /// The service over `store`.
    pub fn new(store: Arc<Store>, secret: String) -> Arc<Libraries> {
        Arc::new(Libraries {
            store,
            secret,
            first: OnceCell::new(),
            settled: AtomicBool::new(false),
        })
    }

    /// The first `ensure`, run once and shared.
    pub async fn first_ensure(self: &Arc<Self>) -> &Vec<Note> {
        self.first
            .get_or_init(|| async {
                let store = self.store.clone();
                let notes = tokio::task::spawn_blocking(move || {
                    store.rename_legacy_default();
                    store.ensure(None, false)
                })
                .await
                .unwrap_or_default();
                self.settled.store(true, Ordering::Release);
                notes
            })
            .await
    }

    /// Whether the first `ensure` has settled.
    pub fn settled(&self) -> bool {
        self.settled.load(Ordering::Acquire)
    }

    /// `/status`.
    pub fn status(&self) -> Value {
        json!({ "settled": self.settled() })
    }

    async fn blocking<T: Send + 'static>(
        &self,
        f: impl FnOnce(Arc<Store>) -> T + Send + 'static,
    ) -> Result<T, String> {
        let store = self.store.clone();
        tokio::task::spawn_blocking(move || f(store))
            .await
            .map_err(|e| e.to_string())
    }

    /// One request, answered; a path that is none of the service's is a JSON 404.
    pub async fn answer(self: &Arc<Self>, req: &Request<'_>) -> Reply {
        if self.secret.is_empty() || !same(req.secret.unwrap_or(""), &self.secret) {
            return error(401, "not from this service's file hub");
        }
        match self.route(req).await {
            Ok(r) => r,
            Err(e) => error(500, e),
        }
    }

    async fn route(self: &Arc<Self>, req: &Request<'_>) -> Result<Reply, String> {
        let (method, path) = (req.method, req.path);
        if method == "GET" && path == "/hub/ready" {
            return Ok(Reply::json(200, json!({ "ok": true })));
        }
        if method == "GET" && path == "/status" {
            return Ok(Reply::json(200, self.status()));
        }
        if method == "POST" && path == "/libraries/sync" {
            return self.sync(req).await;
        }
        if path != ACCOUNT && !path.starts_with(&format!("{ACCOUNT}/")) {
            return Ok(error(404, format!("no {method} {path}")));
        }
        self.account(req).await
    }

    /// `/account/libraries`: the list, which everyone reads.
    async fn account(self: &Arc<Self>, req: &Request<'_>) -> Result<Reply, String> {
        let sub = match &req.path[ACCOUNT.len()..] {
            "" => "/",
            s => s,
        };
        let method = req.method;
        if method == "GET" && sub == "/" {
            let view = self
                .blocking(move |s| {
                    let notes = s.ensure(None, false);
                    s.view(&notes)
                })
                .await?;
            return Ok(Reply::json(200, view));
        }
        Ok(error(404, format!("no {method} {}", req.path)))
    }

    /// `POST /libraries/sync`: the folders made sure of, an admin's draft too.
    async fn sync(self: &Arc<Self>, req: &Request<'_>) -> Result<Reply, String> {
        let body = match json_body_of(req) {
            Ok(b) => b,
            Err(r) => return Ok(r),
        };
        let name = body.get("name").and_then(Value::as_str).map(str::to_string);
        let draft = body.get("draft") == Some(&Value::Bool(true));
        let notes = self
            .blocking(move |s| s.ensure(name.as_deref(), draft))
            .await?;
        Ok(Reply::json(
            200,
            json!({ "notes": notes.iter().map(Note::json).collect::<Vec<_>>() }),
        ))
    }
}

/// The request's body as `jsonBody` reads it; past the limit, the 413 itself.
fn json_body_of(req: &Request<'_>) -> Result<Value, Reply> {
    if req.body.len() > JSON_BODY_MAX {
        return Err(error(413, format!("body over {JSON_BODY_MAX} bytes")));
    }
    Ok(json_body(req.body))
}
