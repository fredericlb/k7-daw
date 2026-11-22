//! tests_git.rs — the git routes through their hook over real workspaces and a bare repository (`file://`): connect, commit,
//! push, clone, pull with and without uncommitted changes, a merge that waits (and refuses writes), and no secret in an answer.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::body::Body;
use axum::extract::Request;
use http_body_util::BodyExt;
use k7_git::auth::{KnownHosts, MemorySecrets};
use k7_git::{IdentityStore, Net};
use serde_json::{Value, json};

use super::git::{Ctx, Kind, hook, merge_refusal};

pub(super) struct Dev {
    pub(super) ctx: Ctx,
    pub(super) dir: PathBuf,
}

pub(super) fn bare(at: &Path) {
    let mut o = git2::RepositoryInitOptions::new();
    o.bare(true).initial_head("main").mkpath(true);
    git2::Repository::init_opts(at, &o).unwrap();
}

pub(super) fn device(name: &str) -> Dev {
    let dir = super::tests::tmp_root().join(format!("k7-front-git-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("workspace")).unwrap();
    let secrets = Arc::new(MemorySecrets::default());
    let hosts = Arc::new(KnownHosts::memory());
    k7_git::init();
    let net = Net::new(
        tokio::runtime::Handle::current(),
        crate::trust::client_config(),
        secrets.clone(),
        hosts.clone(),
    )
    .with_file(true);
    let ctx = Ctx {
        net,
        secrets,
        hosts,
        identity: IdentityStore::in_folder(&dir),
        workspaces: super::tests::slashed(&dir.join("workspace")),
        account_home: super::tests::slashed(&dir),
        device: "Test device".into(),
        port: 0,
        land: None,
        fetchers: Arc::default(),
        account: None,
    };
    Dev { ctx, dir }
}

fn req(method: &str, uri: &str, body: Option<Value>) -> Request {
    let mut b = Request::builder().method(method).uri(uri);
    if body.is_some() {
        b = b.header("content-type", "application/json");
    }
    b.body(Body::from(body.map(|v| v.to_string()).unwrap_or_default()))
        .unwrap()
}

pub(super) async fn ask(
    d: &Dev,
    kind: Kind,
    method: &str,
    uri: &str,
    body: Option<Value>,
) -> (u16, Value, String) {
    let res = hook(&d.ctx, kind, req(method, uri, body)).await;
    let status = res.status().as_u16();
    let text =
        String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();
    (
        status,
        serde_json::from_str(&text).unwrap_or(Value::Null),
        text,
    )
}

pub(super) async fn post(d: &Dev, kind: Kind, uri: &str, body: Value) -> (u16, Value, String) {
    ask(d, kind, "POST", uri, Some(body)).await
}

pub(super) fn ws(d: &Dev, name: &str) -> PathBuf {
    let p = d.dir.join("workspace").join(name);
    std::fs::create_dir_all(&p).unwrap();
    p
}

pub(super) async fn who(d: &Dev) {
    let (s, ..) = post(
        d,
        Kind::SetIdentity,
        "/git/identity",
        json!({ "name": "Ann", "email": "ann@example.org" }),
    )
    .await;
    assert_eq!(s, 200);
}

pub(super) fn url(remote: &Path) -> String {
    format!("file://{}", remote.display())
}

#[tokio::test(flavor = "multi_thread")]
async fn a_workspace_goes_from_none_to_connected_and_a_commit_needs_an_identity() {
    let (a, remote) = (device("a"), device("remote-a").dir.join("bare.git"));
    bare(&remote);
    let root = ws(&a, "band");
    std::fs::write(root.join("song.k7x"), "a: 1\n").unwrap();
    let (s, v, _) = ask(&a, Kind::State, "GET", "/git/state?workspace=band", None).await;
    assert_eq!((s, v["mode"].as_str()), (200, Some("none")));
    let (s, v, _) = post(
        &a,
        Kind::Connect,
        "/git/connect",
        json!({ "workspace": "band", "url": url(&remote) }),
    )
    .await;
    assert_eq!((s, v["ok"].as_bool()), (200, Some(true)), "{v}");
    let (_, v, _) = ask(&a, Kind::State, "GET", "/git/state?workspace=band", None).await;
    assert_eq!(
        (v["mode"].as_str(), v["remote"].as_str()),
        (Some("connected"), Some(url(&remote).as_str()))
    );
    let (s, v, _) = post(
        &a,
        Kind::Commit,
        "/git/commit",
        json!({ "workspace": "band", "message": "first" }),
    )
    .await;
    assert_eq!((s, v["code"].as_str()), (400, Some("no-identity")));
    who(&a).await;
    let (s, v, _) = post(
        &a,
        Kind::Commit,
        "/git/commit",
        json!({ "workspace": "band", "message": "first" }),
    )
    .await;
    assert_eq!(s, 200, "{v}");
    let (_, v, _) = ask(&a, Kind::Status, "GET", "/git/status?workspace=band", None).await;
    assert_eq!(v["entries"].as_array().map(Vec::len), Some(0));
    let (s, ..) = post(&a, Kind::Push, "/git/push", json!({ "workspace": "band" })).await;
    assert_eq!(s, 200);
    let (_, v, _) = ask(&a, Kind::Log, "GET", "/git/log?workspace=band", None).await;
    assert_eq!(v["commits"][0]["subject"], "first");
    assert_eq!(v["commits"][0]["device"], "Test device");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_clone_makes_a_workspace_and_a_pull_with_uncommitted_changes_asks_for_a_message() {
    let (a, b) = (device("a2"), device("b2"));
    let remote = a.dir.join("bare.git");
    bare(&remote);
    let root = ws(&a, "band");
    std::fs::write(root.join("song.k7x"), "a: 1\n").unwrap();
    who(&a).await;
    post(
        &a,
        Kind::Connect,
        "/git/connect",
        json!({ "workspace": "band", "url": url(&remote) }),
    )
    .await;
    post(
        &a,
        Kind::Commit,
        "/git/commit",
        json!({ "workspace": "band", "message": "first" }),
    )
    .await;
    post(&a, Kind::Push, "/git/push", json!({ "workspace": "band" })).await;

    let (s, v, _) = post(
        &b,
        Kind::Clone,
        "/git/clone",
        json!({ "url": url(&remote), "name": "copy" }),
    )
    .await;
    assert_eq!((s, v["name"].as_str()), (200, Some("copy")), "{v}");
    assert_eq!(
        std::fs::read_to_string(b.dir.join("workspace/copy/song.k7x")).unwrap(),
        "a: 1\n"
    );
    let (s, v, _) = post(
        &b,
        Kind::Clone,
        "/git/clone",
        json!({ "url": url(&remote), "name": "copy" }),
    )
    .await;
    assert_eq!((s, v["code"].as_str()), (409, Some("exists")));

    std::fs::write(root.join("song.k7x"), "a: 2\n").unwrap();
    post(
        &a,
        Kind::Commit,
        "/git/commit",
        json!({ "workspace": "band", "message": "second" }),
    )
    .await;
    post(&a, Kind::Push, "/git/push", json!({ "workspace": "band" })).await;
    std::fs::write(b.dir.join("workspace/copy/notes.md"), "mine\n").unwrap();
    let (s, v, _) = post(&b, Kind::Pull, "/git/pull", json!({ "workspace": "copy" })).await;
    assert_eq!((s, v["code"].as_str()), (409, Some("uncommitted")), "{v}");
    who(&b).await;
    let (s, v, _) = post(
        &b,
        Kind::Pull,
        "/git/pull",
        json!({ "workspace": "copy", "message": "my notes" }),
    )
    .await;
    assert_eq!(s, 200, "{v}");
    assert_eq!(v["state"], "merged");
    assert_eq!(
        std::fs::read_to_string(b.dir.join("workspace/copy/song.k7x")).unwrap(),
        "a: 2\n"
    );
    assert!(b.dir.join("workspace/copy/notes.md").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_conflicting_pull_waits_refuses_writes_and_ends_when_every_choice_is_made() {
    let (a, b) = (device("a3"), device("b3"));
    let remote = a.dir.join("bare.git");
    bare(&remote);
    let root = ws(&a, "band");
    std::fs::write(root.join("n.md"), "one\ntwo\nthree\n").unwrap();
    who(&a).await;
    who(&b).await;
    post(
        &a,
        Kind::Connect,
        "/git/connect",
        json!({ "workspace": "band", "url": url(&remote) }),
    )
    .await;
    post(
        &a,
        Kind::Commit,
        "/git/commit",
        json!({ "workspace": "band", "message": "first" }),
    )
    .await;
    post(&a, Kind::Push, "/git/push", json!({ "workspace": "band" })).await;
    post(
        &b,
        Kind::Clone,
        "/git/clone",
        json!({ "url": url(&remote), "name": "copy" }),
    )
    .await;
    let copy = b.dir.join("workspace/copy");
    std::fs::write(root.join("n.md"), "one\nTHEIRS\nthree\n").unwrap();
    post(
        &a,
        Kind::Commit,
        "/git/commit",
        json!({ "workspace": "band", "message": "a" }),
    )
    .await;
    post(&a, Kind::Push, "/git/push", json!({ "workspace": "band" })).await;
    std::fs::write(copy.join("n.md"), "one\nMINE\nthree\n").unwrap();
    post(
        &b,
        Kind::Commit,
        "/git/commit",
        json!({ "workspace": "copy", "message": "b" }),
    )
    .await;

    assert!(merge_refusal(&copy.display().to_string()).is_none());
    let (s, v, _) = post(&b, Kind::Pull, "/git/pull", json!({ "workspace": "copy" })).await;
    assert_eq!((s, v["state"].as_str()), (200, Some("conflicts")), "{v}");
    let refusal =
        merge_refusal(&copy.display().to_string()).expect("a pending merge refuses writes");
    assert_eq!(refusal.status().as_u16(), 423);
    let (_, v, _) = ask(&b, Kind::Merge, "GET", "/git/merge?workspace=copy", None).await;
    assert_eq!(v["pending"], true);
    let id = v["conflicts"][0]["id"].as_str().unwrap().to_string();
    let (s, v, _) = post(
        &b,
        Kind::Resolve,
        "/git/merge/resolve",
        json!({ "workspace": "copy", "choices": [] }),
    )
    .await;
    assert_eq!((s, v["code"].as_str()), (409, Some("unresolved")));
    let (s, v, _) = post(
        &b,
        Kind::Resolve,
        "/git/merge/resolve",
        json!({ "workspace": "copy", "choices": [{ "id": id, "choice": "both" }] }),
    )
    .await;
    assert_eq!(s, 200, "{v}");
    assert_eq!(
        std::fs::read_to_string(copy.join("n.md")).unwrap(),
        "one\nMINE\nTHEIRS\nthree\n"
    );
    assert!(
        merge_refusal(&copy.display().to_string()).is_none(),
        "the workspace is writable again"
    );
    let (_, v, _) = ask(&b, Kind::State, "GET", "/git/state?workspace=copy", None).await;
    assert_eq!((v["ahead"].as_u64(), v["merge"].is_null()), (Some(2), true));
}

#[tokio::test(flavor = "multi_thread")]
async fn an_abort_ends_a_waiting_merge_and_a_host_key_is_never_in_an_answer() {
    let (a, b) = (device("a4"), device("b4"));
    let remote = a.dir.join("bare.git");
    bare(&remote);
    let root = ws(&a, "band");
    std::fs::write(root.join("n.md"), "one\n").unwrap();
    who(&a).await;
    who(&b).await;
    post(
        &a,
        Kind::Connect,
        "/git/connect",
        json!({ "workspace": "band", "url": url(&remote) }),
    )
    .await;
    post(
        &a,
        Kind::Commit,
        "/git/commit",
        json!({ "workspace": "band", "message": "first" }),
    )
    .await;
    post(&a, Kind::Push, "/git/push", json!({ "workspace": "band" })).await;
    post(
        &b,
        Kind::Clone,
        "/git/clone",
        json!({ "url": url(&remote), "name": "copy" }),
    )
    .await;
    std::fs::write(root.join("n.md"), "T\n").unwrap();
    post(
        &a,
        Kind::Commit,
        "/git/commit",
        json!({ "workspace": "band", "message": "a" }),
    )
    .await;
    post(&a, Kind::Push, "/git/push", json!({ "workspace": "band" })).await;
    let copy = b.dir.join("workspace/copy");
    std::fs::write(copy.join("n.md"), "M\n").unwrap();
    post(
        &b,
        Kind::Commit,
        "/git/commit",
        json!({ "workspace": "copy", "message": "b" }),
    )
    .await;
    post(&b, Kind::Pull, "/git/pull", json!({ "workspace": "copy" })).await;
    let (s, ..) = post(
        &b,
        Kind::Abort,
        "/git/merge/abort",
        json!({ "workspace": "copy" }),
    )
    .await;
    assert_eq!(s, 200);
    assert!(merge_refusal(&copy.display().to_string()).is_none());
    assert_eq!(std::fs::read_to_string(copy.join("n.md")).unwrap(), "M\n");

    let (s, _, text) = post(&b, Kind::Connect, "/git/connect", json!({ "workspace": "copy", "url": "ssh://ann@127.0.0.1:1/x.git", "hostKey": "SECRET-FINGERPRINT" })).await;
    assert_ne!(s, 200);
    assert!(!text.contains("SECRET-FINGERPRINT"), "{text}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_commit_leaves_out_what_it_is_told_to_and_the_status_keeps_naming_it() {
    k7_git::set_default_max_file(1_000_000);
    let (a, remote) = (device("large"), device("remote-large").dir.join("bare.git"));
    bare(&remote);
    let root = ws(&a, "band");
    std::fs::write(root.join("song.k7x"), "a: 1\n").unwrap();
    std::fs::write(root.join("huge.mov"), vec![1u8; 1_000_001]).unwrap();
    post(
        &a,
        Kind::Connect,
        "/git/connect",
        json!({ "workspace": "band", "url": url(&remote) }),
    )
    .await;
    who(&a).await;
    let commit =
        |exclude: Value| json!({ "workspace": "band", "message": "m", "exclude": exclude });
    let (s, v, _) = post(&a, Kind::Commit, "/git/commit", commit(json!([]))).await;
    assert_eq!((s, v["code"].as_str()), (400, Some("too-big")), "{v}");
    let (s, v, _) = post(&a, Kind::Commit, "/git/commit", commit(json!(["huge.mov"]))).await;
    assert_eq!(s, 200, "{v}");
    let (_, v, _) = ask(&a, Kind::Status, "GET", "/git/status?workspace=band", None).await;
    assert_eq!(v["entries"][0]["path"], "huge.mov");
    assert_eq!(v["entries"][0]["tooLarge"], true);
    let (_, v, _) = ask(&a, Kind::State, "GET", "/git/state?workspace=band", None).await;
    assert_eq!(v["changed"], 0);
    assert_eq!(
        std::fs::metadata(root.join("huge.mov")).unwrap().len(),
        1_000_001
    );
    k7_git::set_default_max_file(k7_git::MAX_FILE);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_fetch_time_is_in_milliseconds_and_a_remote_nobody_pushed_to_says_so() {
    let (a, remote) = (
        device("a-empty"),
        device("remote-empty").dir.join("bare.git"),
    );
    bare(&remote);
    let root = ws(&a, "band");
    let c = json!({ "workspace": "band", "url": url(&remote) });
    let (s, ..) = post(&a, Kind::Connect, "/git/connect", c).await;
    assert_eq!(s, 200);
    let (_, v, _) = ask(&a, Kind::State, "GET", "/git/state?workspace=band", None).await;
    assert!(
        v.get("remoteEmpty").is_none(),
        "no fetch has completed: {v}"
    );
    let cadence = a.ctx.fetchers.of(&root.display().to_string());
    cadence
        .lock()
        .unwrap()
        .done(super::git_fetch::now(), Ok(()));
    let (_, v, _) = ask(&a, Kind::State, "GET", "/git/state?workspace=band", None).await;
    assert_eq!(v["remoteEmpty"], true, "{v}");
    let since = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
    let ms = since.unwrap().as_millis() as i64;
    let at = v["lastFetch"]["at"].as_i64().unwrap();
    assert!((ms - at).abs() < 5_000, "{at} vs {ms}");
    let failed = Err(("net".to_string(), "down".to_string()));
    cadence
        .lock()
        .unwrap()
        .done(super::git_fetch::now(), failed);
    let (_, v, _) = ask(&a, Kind::State, "GET", "/git/state?workspace=band", None).await;
    assert!(
        v.get("remoteEmpty").is_none(),
        "a failed fetch says nothing: {v}"
    );
}
