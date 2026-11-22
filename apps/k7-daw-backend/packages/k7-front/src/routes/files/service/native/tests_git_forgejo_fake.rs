//! tests_git_forgejo_fake.rs — a fake Forgejo 17 on 127.0.0.1 answering as the real one was measured (`forgejo-plan.md`):
//! PKCE required, the exact redirect path on literal 127.0.0.1 any port, single-use rotating refresh tokens whose reuse
//! kills the grant, the 422 key sentences, 409 on a taken name, pages clamped to 50 with `Link`, topics 204 or 403.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::Router;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::response::Response;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::git_forge_token::fingerprint;

pub(super) const LOGIN: &str = "fred";

/// What the fake holds and was asked.
#[derive(Default)]
pub(super) struct St {
    pub client_id: String,
    pub ttl: u64,
    pub deny: bool,
    /// A server that refuses a reused refresh token without revoking the grant.
    pub keep_on_reuse: bool,
    codes: HashMap<String, (String, String, String)>,
    access: HashMap<String, Instant>,
    pub refresh: Option<String>,
    used: HashSet<String>,
    scope: Option<String>,
    n: u32,
    pub keys: Vec<(String, String)>,
    pub elsewhere: Vec<String>,
    pub repos: Vec<String>,
    pub collab: Vec<String>,
    pub topics: HashMap<String, Vec<String>>,
    pub created: Vec<Value>,
    pub asked: Vec<String>,
}

pub(super) type Fake = Arc<Mutex<St>>;

fn reply(status: u16, body: Value) -> Response {
    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn to(location: &str) -> Response {
    Response::builder()
        .status(302)
        .header("location", location)
        .body(Body::empty())
        .unwrap()
}

/// A registered redirect: the app's scheme exactly, or the loopback path on literal 127.0.0.1 with any port.
fn registered(uri: &str) -> bool {
    if uri == "rocks.k7.daw://callback" {
        return true;
    }
    let Ok(u) = url::Url::parse(uri) else {
        return false;
    };
    u.scheme() == "http"
        && u.host_str() == Some("127.0.0.1")
        && u.path() == "/api/files/git/forge/callback"
        && u.query().is_none()
}

fn pair(st: &mut St) -> Value {
    st.n += 1;
    let (at, rt) = (format!("at-{}", st.n), format!("rt-{}", st.n));
    let ttl = Duration::from_secs(st.ttl);
    st.access.insert(at.clone(), Instant::now() + ttl);
    st.refresh = Some(rt.clone());
    json!({ "access_token": at, "token_type": "bearer", "expires_in": st.ttl, "refresh_token": rt })
}

fn authorize(st: &mut St, q: &HashMap<String, String>) -> Response {
    let get = |k: &str| q.get(k).cloned().unwrap_or_default();
    let (redirect, state) = (get("redirect_uri"), get("state"));
    if get("client_id") != st.client_id || !registered(&redirect) {
        return reply(400, json!({ "message": "Unregistered Redirect URI" }));
    }
    let back = |pairs: &[(&str, &str)]| {
        let mut s = url::form_urlencoded::Serializer::new(String::new());
        for (k, v) in pairs {
            s.append_pair(k, v);
        }
        to(&format!("{redirect}?{}&state={state}", s.finish()))
    };
    if get("code_challenge").is_empty() || get("code_challenge_method") != "S256" {
        let said = "PKCE is required for public clients";
        return back(&[("error", "invalid_request"), ("error_description", said)]);
    }
    let scope = get("scope");
    if st.scope.as_ref().is_some_and(|s| *s != scope) {
        let said = "a grant exists with different scope";
        return back(&[("error", "server_error"), ("error_description", said)]);
    }
    if st.deny {
        let said = "the request is denied";
        return back(&[("error", "access_denied"), ("error_description", said)]);
    }
    st.n += 1;
    let code = format!("code-{}", st.n);
    st.codes.insert(
        code.clone(),
        (get("code_challenge"), redirect.clone(), scope.clone()),
    );
    st.scope = Some(scope);
    back(&[("code", &code)])
}

fn token(st: &mut St, b: &Value) -> Response {
    let s = |k: &str| b[k].as_str().unwrap_or("").to_string();
    let no = |e: &str, d: &str| reply(400, json!({ "error": e, "error_description": d }));
    match s("grant_type").as_str() {
        "authorization_code" => {
            let Some((challenge, redirect, _)) = st.codes.remove(&s("code")) else {
                return no("invalid_grant", "client is not authorized");
            };
            let hashed = URL_SAFE_NO_PAD.encode(Sha256::digest(s("code_verifier").as_bytes()));
            if redirect != s("redirect_uri")
                || hashed != challenge
                || s("client_id") != st.client_id
            {
                return no("unauthorized_client", "failed PKCE code challenge");
            }
            reply(200, pair(st))
        }
        "refresh_token" => {
            let given = s("refresh_token");
            if st.refresh.as_deref() == Some(given.as_str()) {
                st.used.insert(given);
                return reply(200, pair(st));
            }
            if st.used.contains(&given) {
                if !st.keep_on_reuse {
                    st.refresh = None;
                    st.access.clear();
                }
                return no("unauthorized_client", "token was already used");
            }
            no("invalid_grant", "unable to parse refresh token")
        }
        _ => no(
            "unsupported_grant_type",
            "Only refresh_token or authorization_code grant type is supported",
        ),
    }
}

fn repo_json(full: &str, topics: Option<&Vec<String>>) -> Value {
    json!({ "full_name": full, "ssh_url": format!("git@127.0.0.1:{full}.git"), "private": true,
            "topics": topics, "updated_at": "2026-11-22T10:00:00Z" })
}

fn repos(st: &St, base: &str, q: &HashMap<String, String>) -> Response {
    let mut all: Vec<String> = st.repos.iter().map(|r| format!("{LOGIN}/{r}")).collect();
    all.extend(st.collab.iter().cloned());
    all.sort();
    let asked = q.get("limit").cloned().unwrap_or_else(|| "30".into());
    let limit = asked.parse::<usize>().unwrap_or(30).clamp(1, 50);
    let page = q
        .get("page")
        .and_then(|p| p.parse::<usize>().ok())
        .unwrap_or(1)
        .max(1);
    let rows: Vec<Value> = all
        .iter()
        .skip((page - 1) * limit)
        .take(limit)
        .map(|f| repo_json(f, st.topics.get(f)))
        .collect();
    let mut r = Response::builder().header("x-total-count", all.len().to_string());
    // The Link keeps the limit asked, not the one served, as Forgejo 17 does.
    if page * limit < all.len() {
        let next = format!(
            "<{base}/api/v1/user/repos?limit={asked}&page={}>; rel=\"next\"",
            page + 1
        );
        r = r.header("link", next);
    }
    r.body(Body::from(json!(rows).to_string())).unwrap()
}

fn api(
    st: &mut St,
    method: &str,
    path: &str,
    q: &HashMap<String, String>,
    b: &Value,
    base: &str,
) -> Response {
    match (method, path) {
        ("GET", "/user") => reply(200, json!({ "id": 1, "login": LOGIN })),
        ("GET", "/user/keys") => {
            let want = q.get("fingerprint");
            let rows: Vec<Value> = st
                .keys
                .iter()
                .filter(|(_, k)| want.is_none_or(|w| fingerprint(k).as_ref() == Some(w)))
                .map(|(t, k)| json!({ "title": t, "key": k, "fingerprint": fingerprint(k) }))
                .collect();
            reply(200, json!(rows))
        }
        ("POST", "/user/keys") => {
            let (title, key) = (
                b["title"].as_str().unwrap_or(""),
                b["key"].as_str().unwrap_or(""),
            );
            let blob = |k: &str| k.split_whitespace().nth(1).map(str::to_string);
            let used = st.keys.iter().map(|(_, k)| k).chain(st.elsewhere.iter());
            if used.into_iter().any(|k| blob(k) == blob(key)) {
                return reply(
                    422,
                    json!({ "message": "Key content has been used as non-deploy key" }),
                );
            }
            if st.keys.iter().any(|(t, _)| t == title) {
                return reply(422, json!({ "message": "Key title has been used" }));
            }
            st.keys.push((title.into(), key.into()));
            reply(
                201,
                json!({ "id": st.keys.len(), "title": title, "fingerprint": fingerprint(key) }),
            )
        }
        ("GET", "/user/repos") => repos(st, base, q),
        ("POST", "/user/repos") => {
            st.created.push(b.clone());
            let name = b["name"].as_str().unwrap_or("");
            if name.is_empty() || name.contains(' ') {
                return reply(422, json!({ "message": "[Name]: invalid name" }));
            }
            if st.repos.iter().any(|r| r == name) {
                return reply(
                    409,
                    json!({ "message": "The repository with the same name already exists." }),
                );
            }
            st.repos.push(name.into());
            let mut row = repo_json(&format!("{LOGIN}/{name}"), None);
            row["private"] = json!(b["private"].as_bool().unwrap_or(false));
            reply(201, row)
        }
        ("PUT", p) if p.starts_with("/repos/") && p.contains("/topics/") => {
            let (full, topic) = p
                .trim_start_matches("/repos/")
                .split_once("/topics/")
                .unwrap();
            if st.collab.iter().any(|c| c == full) {
                return reply(
                    403,
                    json!({ "message": "user should be an owner or a collaborator with admin write" }),
                );
            }
            let list = st.topics.entry(full.to_string()).or_default();
            if !list.iter().any(|t| t == topic) {
                list.push(topic.to_string());
            }
            Response::builder().status(204).body(Body::empty()).unwrap()
        }
        _ => reply(404, json!({ "message": "Not Found" })),
    }
}

async fn serve(State(f): State<Fake>, req: Request) -> Response {
    let (method, uri) = (req.method().to_string(), req.uri().clone());
    let host = req
        .headers()
        .get("host")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("")
        .to_string();
    let base = format!("http://{host}");
    let bearer = req
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .map(str::to_string);
    let bytes = req.into_body().collect().await.unwrap().to_bytes();
    let b: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    let q: HashMap<String, String> =
        url::form_urlencoded::parse(uri.query().unwrap_or("").as_bytes())
            .into_owned()
            .collect();
    let mut st = f.lock().unwrap();
    st.asked.push(format!("{method} {}", uri));
    match (method.as_str(), uri.path()) {
        ("GET", "/api/v1/version") => reply(200, json!({ "version": "17.0.2+gitea-1.22.0" })),
        ("GET", "/api/v1/settings/api") => reply(
            200,
            json!({ "max_response_items": 50, "default_paging_num": 30 }),
        ),
        ("GET", "/api/v1/settings/repository") => reply(
            200,
            json!({ "lfs_disabled": false, "http_git_disabled": false }),
        ),
        ("GET", "/.well-known/openid-configuration") => {
            reply(200, json!({ "issuer": format!("{base}/") }))
        }
        ("GET", "/login/oauth/authorize") => authorize(&mut st, &q),
        ("POST", "/login/oauth/access_token") => token(&mut st, &b),
        (m, p) if p.starts_with("/api/v1/") => {
            let live = bearer
                .and_then(|t| st.access.get(&t).copied())
                .is_some_and(|e| Instant::now() < e);
            if !live {
                return reply(
                    401,
                    json!({ "message": "invalid username, password or token" }),
                );
            }
            api(&mut st, m, p.trim_start_matches("/api/v1"), &q, &b, &base)
        }
        _ => reply(404, json!({ "message": "Not Found" })),
    }
}

/// A fake on its own port; its address is the server's, as the person would type it.
pub(super) async fn start() -> (Fake, String) {
    let f: Fake = Arc::new(Mutex::new(St {
        client_id: "cid".into(),
        ttl: 3600,
        ..St::default()
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let app = Router::new().fallback(serve).with_state(f.clone());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (f, url)
}

/// One GET that does not follow a redirect: the status and the `Location`.
pub(super) async fn visit(url: &str) -> (u16, Option<String>) {
    let req = hyper::Request::get(url)
        .body(Full::new(Bytes::new()))
        .unwrap();
    let res = crate::https::client::<Full<Bytes>>()
        .request(req)
        .await
        .unwrap();
    let at = res
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    (res.status().as_u16(), at)
}
