//! tests_git_github.rs — the GitHub sign-in routes against a fake GitHub on 127.0.0.1: each device-flow state, the key (present,
//! uploaded, refused), the pinned host key, the repository pages, the scope refusal, creation, the topic and the sign-out.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::response::Response;
use http_body_util::BodyExt;
use serde_json::{Value, json};

use super::git::Ctx;
use super::git_forge::{Forge, repo_of};
use super::git_forge_routes::{self as fr, Verb};
use super::git_github::GitHub;
use super::tests_git::device;

/// What the fake GitHub is told to answer, and what it was asked.
#[derive(Default)]
struct Fake {
    polls: Mutex<VecDeque<Value>>,
    scopes: Mutex<String>,
    keys: Mutex<Vec<String>>,
    key_status: Mutex<u16>,
    topics: Mutex<HashMap<String, Vec<String>>>,
    asked: Mutex<Vec<String>>,
}

fn reply(status: u16, body: Value) -> Response {
    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

async fn serve(State(f): State<Arc<Fake>>, req: Request) -> Response {
    let (method, uri) = (req.method().to_string(), req.uri().clone());
    let path = uri.path().to_string();
    let host = req
        .headers()
        .get("host")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("")
        .to_string();
    let base = format!("http://{host}");
    let bytes = req.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    f.asked.lock().unwrap().push(format!("{method} {path}"));
    let page2 = uri.query().is_some_and(|q| q.contains("page=2"));
    match (method.as_str(), path.as_str()) {
        ("POST", "/login/device/code") => reply(
            200,
            json!({ "device_code": "dc", "user_code": "ABCD-1234", "verification_uri": format!("{base}/login/device"), "expires_in": 900, "interval": 5 }),
        ),
        ("POST", "/login/oauth/access_token") => {
            let next = f.polls.lock().unwrap().pop_front();
            reply(
                200,
                next.unwrap_or(json!({ "error": "authorization_pending" })),
            )
        }
        ("GET", "/user") => Response::builder()
            .header("x-oauth-scopes", f.scopes.lock().unwrap().clone())
            .body(Body::from(json!({ "login": "octo" }).to_string()))
            .unwrap(),
        ("GET", "/user/keys") => reply(
            200,
            json!(
                f.keys
                    .lock()
                    .unwrap()
                    .iter()
                    .map(|k| json!({ "key": k }))
                    .collect::<Vec<_>>()
            ),
        ),
        ("POST", "/user/keys") => {
            let status = *f.key_status.lock().unwrap();
            if status == 201 {
                f.keys
                    .lock()
                    .unwrap()
                    .push(body["key"].as_str().unwrap().to_string());
            }
            reply(status, json!({ "message": "key is already in use" }))
        }
        ("GET", "/meta") => reply(
            200,
            json!({ "ssh_keys": ["ecdsa-sha2-nistp256 AAAA", "ssh-ed25519 aGVsbG8="] }),
        ),
        ("GET", "/user/repos") if !page2 => Response::builder()
            .header(
                "link",
                format!("<{base}/user/repos?sort=pushed&per_page=100&page=2>; rel=\"next\""),
            )
            .body(Body::from(
                json!([
                    repo("octo/plain", &[]),
                    repo("octo/song", &["k7-workspace"])
                ])
                .to_string(),
            ))
            .unwrap(),
        ("GET", "/user/repos") => reply(200, json!([repo("octo/old", &["rust"])])),
        ("POST", "/user/repos") if body["name"] == "taken" => reply(
            422,
            json!({ "message": "Repository creation failed.", "errors": [{ "message": "name already exists on this account" }] }),
        ),
        ("POST", "/user/repos") => {
            let name = body["name"].as_str().unwrap();
            reply(
                201,
                json!({ "full_name": format!("octo/{name}"), "ssh_url": format!("git@github.com:octo/{name}.git") }),
            )
        }
        (m, p) if p.starts_with("/repos/") && p.ends_with("/topics") => {
            let repo = p
                .trim_start_matches("/repos/")
                .trim_end_matches("/topics")
                .to_string();
            if m == "PUT" {
                let names = body["names"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|n| n.as_str().unwrap().to_string())
                    .collect();
                f.topics.lock().unwrap().insert(repo, names);
                return reply(200, json!({}));
            }
            let names = f
                .topics
                .lock()
                .unwrap()
                .get(&repo)
                .cloned()
                .unwrap_or_default();
            reply(200, json!({ "names": names }))
        }
        _ => reply(404, json!({ "message": "Not Found" })),
    }
}

fn repo(full: &str, topics: &[&str]) -> Value {
    json!({ "full_name": full, "ssh_url": format!("git@github.com:{full}.git"), "private": true, "pushed_at": "2026-11-01T00:00:00Z", "topics": topics })
}

async fn fake() -> (Arc<Fake>, Forge) {
    let f = Arc::new(Fake::default());
    *f.key_status.lock().unwrap() = 201;
    *f.scopes.lock().unwrap() = "read:user, write:public_key".into();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let app = Router::new().fallback(serve).with_state(f.clone());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (
        f,
        Forge::GitHub(GitHub::new(Some("cid".into()), url.clone(), url)),
    )
}

async fn json_of(r: Response) -> (u16, Value) {
    let status = r.status().as_u16();
    let bytes = r.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn start(ctx: &Ctx, hub: &Forge, scopes: &[&str]) -> Value {
    let (s, v) = json_of(hub.start(ctx, &json!({ "scopes": scopes })).await).await;
    assert_eq!(s, 200);
    v
}

async fn signed_in(ctx: &Ctx, hub: &Forge, f: &Fake) {
    f.polls
        .lock()
        .unwrap()
        .push_back(json!({ "access_token": "tok" }));
    start(ctx, hub, &["read:user", "write:public_key"]).await;
    assert_eq!(json_of(fr::poll(ctx, hub).await).await.1["state"], "ok");
}

#[tokio::test(flavor = "multi_thread")]
async fn github_is_not_offered_without_a_client_id() {
    let d = device("gh-none");
    let none = Forge::GitHub(GitHub::new(
        None,
        "http://127.0.0.1:1".into(),
        "http://127.0.0.1:1".into(),
    ));
    let (s, v) = json_of(fr::status(&d.ctx, &none)).await;
    assert_eq!((s, &v["available"]), (200, &json!(false)));
    let started = fr::answer(&d.ctx, &none, Verb::Start, &json!({ "scopes": ["repo"] })).await;
    let (s, v) = json_of(started).await;
    assert_eq!((s, &v["code"]), (404, &json!("github-unavailable")));
}

#[tokio::test(flavor = "multi_thread")]
async fn github_device_flow_answers_each_state_and_ends_signed_in() {
    let (f, hub) = fake().await;
    let d = device("gh-flow");
    let began = start(&d.ctx, &hub, &["read:user", "write:public_key"]).await;
    assert_eq!(began["userCode"], "ABCD-1234");
    assert!(
        began["verificationUri"]
            .as_str()
            .unwrap()
            .ends_with("/login/device")
    );
    assert!(
        !began.to_string().contains("\"dc\""),
        "the device code stays server side"
    );
    let mut states = Vec::new();
    for answer in [
        json!({ "error": "authorization_pending" }),
        json!({ "error": "slow_down" }),
    ] {
        f.polls.lock().unwrap().push_back(answer);
        let (_, v) = json_of(fr::poll(&d.ctx, &hub).await).await;
        states.push((
            v["state"].as_str().unwrap().to_string(),
            v["interval"].as_u64().unwrap(),
        ));
    }
    assert_eq!(
        states,
        [("pending".to_string(), 5), ("slow_down".to_string(), 10)]
    );
    f.polls
        .lock()
        .unwrap()
        .push_back(json!({ "access_token": "tok" }));
    let (_, ok) = json_of(fr::poll(&d.ctx, &hub).await).await;
    assert_eq!(
        (ok["state"].clone(), ok["login"].clone()),
        (json!("ok"), json!("octo"))
    );
    assert!(
        !ok.to_string().contains("tok\""),
        "the token is never answered"
    );
    let (_, now) = json_of(fr::status(&d.ctx, &hub)).await;
    assert_eq!(now["login"], "octo");
    assert_eq!(now["scopes"], json!(["read:user", "write:public_key"]));
}

#[tokio::test(flavor = "multi_thread")]
async fn github_device_flow_ends_on_a_denial_or_an_expiry_and_a_poll_without_start_is_expired() {
    let (f, hub) = fake().await;
    let d = device("gh-end");
    assert_eq!(
        json_of(fr::poll(&d.ctx, &hub).await).await.1["state"],
        "expired"
    );
    for (said, state) in [("access_denied", "denied"), ("expired_token", "expired")] {
        start(&d.ctx, &hub, &["repo"]).await;
        f.polls.lock().unwrap().push_back(json!({ "error": said }));
        assert_eq!(
            json_of(fr::poll(&d.ctx, &hub).await).await.1["state"],
            state
        );
    }
    assert!(
        json_of(fr::status(&d.ctx, &hub))
            .await
            .1
            .get("login")
            .is_none()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn github_sign_in_uploads_the_key_once_and_pins_the_host_key() {
    let (f, hub) = fake().await;
    let d = device("gh-key");
    signed_in(&d.ctx, &hub, &f).await;
    assert_eq!(f.keys.lock().unwrap().len(), 1);
    assert_eq!(
        d.ctx.hosts.get("github.com", 22).unwrap().as_deref(),
        Some("SHA256:LPJNul+wow4m6DsqxbninhsWHlwfp0JecwQzYpOLmCQ")
    );
    signed_in(&d.ctx, &hub, &f).await;
    let posts = f
        .asked
        .lock()
        .unwrap()
        .iter()
        .filter(|a| *a == "POST /user/keys")
        .count();
    assert_eq!(posts, 1, "a key already on the account is not posted again");
}

#[tokio::test(flavor = "multi_thread")]
async fn github_key_in_use_elsewhere_is_a_sentence_and_the_sign_in_stands() {
    let (f, hub) = fake().await;
    *f.key_status.lock().unwrap() = 422;
    let d = device("gh-422");
    f.polls
        .lock()
        .unwrap()
        .push_back(json!({ "access_token": "tok" }));
    start(&d.ctx, &hub, &["read:user", "write:public_key"]).await;
    let (_, v) = json_of(fr::poll(&d.ctx, &hub).await).await;
    assert_eq!(v["state"], "ok");
    assert!(
        v["keyNote"]
            .as_str()
            .unwrap()
            .contains("another GitHub account")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn github_repos_follow_the_pages_flag_the_topic_and_need_the_repo_scope() {
    let (f, hub) = fake().await;
    let d = device("gh-repos");
    assert_eq!(json_of(fr::repos(&d.ctx, &hub).await).await.0, 401);
    signed_in(&d.ctx, &hub, &f).await;
    let (s, v) = json_of(fr::repos(&d.ctx, &hub).await).await;
    assert_eq!((s, v["code"].clone()), (403, json!("scope")));
    *f.scopes.lock().unwrap() = "read:user, write:public_key, repo".into();
    f.polls
        .lock()
        .unwrap()
        .push_back(json!({ "access_token": "tok2" }));
    start(&d.ctx, &hub, &["read:user", "write:public_key", "repo"]).await;
    json_of(fr::poll(&d.ctx, &hub).await).await;
    let (s, v) = json_of(fr::repos(&d.ctx, &hub).await).await;
    let names: Vec<_> = v["repos"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["fullName"].as_str().unwrap(), r["k7"].as_bool().unwrap()))
        .collect();
    assert_eq!(s, 200);
    assert_eq!(
        names,
        [
            ("octo/plain", false),
            ("octo/song", true),
            ("octo/old", false)
        ]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn github_creates_a_repository_marked_as_a_workspace_and_says_a_taken_name() {
    let (f, hub) = fake().await;
    let d = device("gh-create");
    *f.scopes.lock().unwrap() = "repo".into();
    signed_in(&d.ctx, &hub, &f).await;
    let (s, v) =
        json_of(fr::create(&d.ctx, &hub, &json!({ "name": "mine", "private": true })).await).await;
    assert_eq!(
        (s, v["sshUrl"].clone()),
        (200, json!("git@github.com:octo/mine.git"))
    );
    assert_eq!(
        f.topics.lock().unwrap().get("octo/mine").unwrap(),
        &["k7-workspace"]
    );
    let (s, v) = json_of(fr::create(&d.ctx, &hub, &json!({ "name": "taken" })).await).await;
    assert_eq!(s, 422);
    assert!(v["error"].as_str().unwrap().contains("already exists"));
}

#[tokio::test(flavor = "multi_thread")]
async fn github_topic_is_added_to_an_existing_repository_and_a_failure_is_silent() {
    let (f, hub) = fake().await;
    let d = device("gh-topic");
    *f.scopes.lock().unwrap() = "repo".into();
    signed_in(&d.ctx, &hub, &f).await;
    f.topics
        .lock()
        .unwrap()
        .insert("octo/song".into(), vec!["rust".into()]);
    fr::tag_topic(&d.ctx, &hub, "git@github.com:octo/song.git").await;
    assert_eq!(
        f.topics.lock().unwrap().get("octo/song").unwrap(),
        &["rust", "k7-workspace"]
    );
    fr::tag_topic(&d.ctx, &hub, "git@gitlab.com:octo/other.git").await;
    assert!(f.topics.lock().unwrap().get("octo/other").is_none());
    let Forge::GitHub(mut g) = hub;
    g.profile.api.base = "http://127.0.0.1:1".into();
    let dead = Forge::GitHub(g);
    fr::tag_topic(&d.ctx, &dead, "git@github.com:octo/song.git").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn github_sign_out_drops_the_token_and_keeps_the_key() {
    let (f, hub) = fake().await;
    let d = device("gh-out");
    signed_in(&d.ctx, &hub, &f).await;
    assert_eq!(json_of(fr::signout(&d.ctx, &hub)).await.0, 200);
    assert!(
        json_of(fr::status(&d.ctx, &hub))
            .await
            .1
            .get("login")
            .is_none()
    );
    assert_eq!(f.keys.lock().unwrap().len(), 1);
    assert!(k7_git::auth::public_key(&*d.ctx.secrets).is_ok());
}

#[test]
fn github_repo_is_read_from_an_ssh_address_of_github_only() {
    assert_eq!(
        repo_of("git@github.com:o/r.git", "github.com").as_deref(),
        Some("o/r")
    );
    assert_eq!(
        repo_of("ssh://git@github.com/o/r", "github.com").as_deref(),
        Some("o/r")
    );
    assert_eq!(repo_of("git@gitlab.com:o/r.git", "github.com"), None);
    assert_eq!(repo_of("git@github.com:o/r/x.git", "github.com"), None);
}
