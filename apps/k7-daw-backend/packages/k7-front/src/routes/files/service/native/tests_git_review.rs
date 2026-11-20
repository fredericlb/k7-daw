//! tests_git_review.rs — the G4 review's cases over the git routes: one lock per workspace, a pending merge refusing a commit,
//! the fetch's turn skipped, a server front refused, a failed connect leaving the workspace as it was.

use std::sync::{Arc, Mutex};

use serde_json::json;

use super::git::Kind;
use super::git_fetch::Cadence;
use super::git_lock;
use super::git_task::{Turn, turn};
use super::tests_git::{ask, bare, device, post, url, who, ws};

/// A connected workspace `band` of device `a`, one commit pushed to `remote`.
async fn connected(
    name: &str,
) -> (
    super::tests_git::Dev,
    std::path::PathBuf,
    std::path::PathBuf,
) {
    let a = device(name);
    let remote = a.dir.join("bare.git");
    bare(&remote);
    let root = ws(&a, "band");
    std::fs::write(root.join("n.md"), "one\n").unwrap();
    who(&a).await;
    let body = json!({ "workspace": "band", "url": url(&remote) });
    post(&a, Kind::Connect, "/git/connect", body).await;
    let m = json!({ "workspace": "band", "message": "first" });
    post(&a, Kind::Commit, "/git/commit", m).await;
    post(&a, Kind::Push, "/git/push", json!({ "workspace": "band" })).await;
    (a, root, remote)
}

#[tokio::test(flavor = "multi_thread")]
async fn a_second_operation_on_a_workspace_answers_busy_at_once_and_reads_still_answer() {
    let (a, root, _) = connected("lock1").await;
    let held = git_lock::try_take(&root.display().to_string()).expect("free");
    let w = json!({ "workspace": "band", "message": "m" });
    for (kind, uri) in [
        (Kind::Pull, "/git/pull"),
        (Kind::Push, "/git/push"),
        (Kind::Commit, "/git/commit"),
        (Kind::Resolve, "/git/merge/resolve"),
        (Kind::Abort, "/git/merge/abort"),
    ] {
        let (s, v, _) = post(&a, kind, uri, w.clone()).await;
        assert_eq!((s, v["code"].as_str()), (409, Some("busy")), "{uri}");
    }
    let (s, ..) = ask(&a, Kind::State, "GET", "/git/state?workspace=band", None).await;
    assert_eq!(s, 200, "a read does not take the lock");
    drop(held);
    let (s, ..) = post(&a, Kind::Push, "/git/push", w).await;
    assert_eq!(s, 200);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_background_fetch_skips_its_turn_while_a_pull_or_push_holds_the_remote() {
    let (a, root, _) = connected("lock2").await;
    let cadence = Arc::new(Mutex::new(Cadence::default()));
    cadence.lock().unwrap().focus(true, super::git_fetch::now());
    let held = git_lock::try_take_remote(&root.display().to_string()).expect("free");
    assert_eq!(turn(&a.ctx.net, &root, &cadence).await, Turn::Skipped);
    {
        let c = cadence.lock().unwrap();
        assert!(c.last.is_none(), "no outcome, so no error and no backoff");
        assert!(c.due(1_000_000), "the turn is still due");
    }
    drop(held);
    assert_eq!(turn(&a.ctx.net, &root, &cadence).await, Turn::Fetched);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_commit_goes_through_while_a_fetch_holds_the_remote_but_a_pull_and_a_push_do_not() {
    let (a, root, _) = connected("lock4").await;
    let held = git_lock::try_take_remote(&root.display().to_string()).expect("free");
    std::fs::write(root.join("n.md"), "two\n").unwrap();
    let m = json!({ "workspace": "band", "message": "during a fetch" });
    let (s, v, _) = post(&a, Kind::Commit, "/git/commit", m).await;
    assert_eq!(s, 200, "{v}");
    for (kind, uri) in [(Kind::Pull, "/git/pull"), (Kind::Push, "/git/push")] {
        let (s, v, _) = post(&a, kind, uri, json!({ "workspace": "band" })).await;
        assert_eq!((s, v["code"].as_str()), (409, Some("busy")), "{uri}");
    }
    drop(held);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_remote_with_no_branch_yet_counts_every_commit_as_ahead() {
    let a = device("first");
    let remote = a.dir.join("bare.git");
    bare(&remote);
    let root = ws(&a, "band");
    std::fs::write(root.join("n.md"), "one\n").unwrap();
    who(&a).await;
    let body = json!({ "workspace": "band", "url": url(&remote) });
    post(&a, Kind::Connect, "/git/connect", body).await;
    let m = json!({ "workspace": "band", "message": "first" });
    post(&a, Kind::Commit, "/git/commit", m).await;
    let (_, v, _) = ask(&a, Kind::State, "GET", "/git/state?workspace=band", None).await;
    assert_eq!(
        (v["ahead"].as_u64(), v["behind"].as_u64()),
        (Some(1), Some(0)),
        "{v}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_push_refused_as_behind_leaves_a_state_that_says_to_pull() {
    let (a, root, remote) = connected("behind").await;
    let b = device("behindb");
    who(&b).await;
    let c = json!({ "url": url(&remote), "name": "copy" });
    post(&b, Kind::Clone, "/git/clone", c).await;
    std::fs::write(b.dir.join("workspace/copy/n.md"), "B\n").unwrap();
    post(
        &b,
        Kind::Commit,
        "/git/commit",
        json!({ "workspace": "copy", "message": "b" }),
    )
    .await;
    post(&b, Kind::Push, "/git/push", json!({ "workspace": "copy" })).await;
    std::fs::write(root.join("n.md"), "A\n").unwrap();
    post(
        &a,
        Kind::Commit,
        "/git/commit",
        json!({ "workspace": "band", "message": "a" }),
    )
    .await;
    let (s, v, _) = post(&a, Kind::Push, "/git/push", json!({ "workspace": "band" })).await;
    assert_eq!((s, v["code"].as_str()), (409, Some("behind")), "{v}");
    let (_, v, _) = ask(&a, Kind::State, "GET", "/git/state?workspace=band", None).await;
    assert_eq!(
        (v["ahead"].as_u64(), v["behind"].as_u64()),
        (Some(1), Some(1)),
        "{v}"
    );
}

/// Device `b` holding a clone whose pull left a merge waiting; its folder and the conflict's id.
async fn waiting(name: &str) -> (super::tests_git::Dev, std::path::PathBuf) {
    let (a, root, remote) = connected(name).await;
    let b = device(&format!("{name}b"));
    who(&b).await;
    let c = json!({ "url": url(&remote), "name": "copy" });
    post(&b, Kind::Clone, "/git/clone", c).await;
    let copy = b.dir.join("workspace/copy");
    std::fs::write(root.join("n.md"), "T\n").unwrap();
    let m = json!({ "workspace": "band", "message": "a" });
    post(&a, Kind::Commit, "/git/commit", m).await;
    post(&a, Kind::Push, "/git/push", json!({ "workspace": "band" })).await;
    std::fs::write(copy.join("n.md"), "M\n").unwrap();
    let m = json!({ "workspace": "copy", "message": "b" });
    post(&b, Kind::Commit, "/git/commit", m).await;
    let (_, v, _) = post(&b, Kind::Pull, "/git/pull", json!({ "workspace": "copy" })).await;
    assert_eq!(v["state"], "conflicts", "{v}");
    (b, copy)
}

fn head(copy: &std::path::Path) -> String {
    let r = git2::Repository::open(copy.join(".git")).unwrap();
    r.refname_to_id("HEAD").unwrap().to_string()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_commit_or_a_pull_with_a_message_is_refused_while_a_merge_waits_and_the_merge_still_ends()
{
    let (b, copy) = waiting("wait1").await;
    let before = head(&copy);
    std::fs::write(copy.join("other.md"), "from outside\n").unwrap();
    let m = json!({ "workspace": "copy", "message": "no" });
    let (s, v, _) = post(&b, Kind::Commit, "/git/commit", m.clone()).await;
    assert_eq!((s, v["code"].as_str()), (409, Some("merge-pending")), "{v}");
    let (s, v, _) = post(&b, Kind::Pull, "/git/pull", m).await;
    assert_eq!((s, v["code"].as_str()), (409, Some("merge-pending")), "{v}");
    assert_eq!(head(&copy), before, "HEAD did not move");
    std::fs::remove_file(copy.join("other.md")).unwrap();
    let (_, v, _) = ask(&b, Kind::Merge, "GET", "/git/merge?workspace=copy", None).await;
    let id = v["conflicts"][0]["id"].as_str().unwrap().to_string();
    let c = json!({ "workspace": "copy", "choices": [{ "id": id, "choice": "theirs" }] });
    let (s, v, _) = post(&b, Kind::Resolve, "/git/merge/resolve", c).await;
    assert_eq!(s, 200, "{v}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failed_connect_leaves_the_remote_and_the_trusted_hosts_as_they_were() {
    let (a, root, remote) = connected("conn1").await;
    a.ctx
        .hosts
        .remember("127.0.0.1", 1, "OLD-FINGERPRINT")
        .unwrap();
    let dead = json!({ "workspace": "band", "url": "ssh://ann@127.0.0.1:1/x.git", "hostKey": "NEW-FINGERPRINT" });
    let (s, ..) = post(&a, Kind::Connect, "/git/connect", dead).await;
    assert_ne!(s, 200);
    let (_, v, _) = ask(&a, Kind::State, "GET", "/git/state?workspace=band", None).await;
    assert_eq!(v["remote"].as_str(), Some(url(&remote).as_str()), "{v}");
    assert_eq!(
        a.ctx.hosts.get("127.0.0.1", 1).unwrap().as_deref(),
        Some("OLD-FINGERPRINT"),
        "the host key that was there stays"
    );
    let fresh =
        json!({ "workspace": "fresh", "url": "ssh://bo@127.0.0.1:2/y.git", "hostKey": "T" });
    std::fs::create_dir_all(root.parent().unwrap().join("fresh")).unwrap();
    let (s, ..) = post(&a, Kind::Connect, "/git/connect", fresh).await;
    assert_ne!(s, 200);
    assert!(
        a.ctx.hosts.get("127.0.0.1", 2).unwrap().is_none(),
        "a new host key is not kept"
    );
    assert!(
        !root.parent().unwrap().join("fresh/.git").exists(),
        "no repository was made"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn an_address_that_is_not_ssh_is_refused_with_ssh_only_and_a_sentence_that_names_the_way() {
    let (a, ..) = connected("sshonly").await;
    for addr in [
        "https://example.com/o/r.git",
        "http://127.0.0.1:1/r.git",
        "git://example.com/o/r.git",
    ] {
        let body = json!({ "workspace": "band", "url": addr });
        let (s, v, _) = post(&a, Kind::Connect, "/git/connect", body).await;
        assert_eq!(
            (s, v["code"].as_str()),
            (400, Some("ssh-only")),
            "{addr}: {v}"
        );
        assert!(v["error"].as_str().unwrap().contains("ssh address"), "{v}");
    }
    let (s, v, _) = post(
        &a,
        Kind::Clone,
        "/git/clone",
        json!({ "url": "https://example.com/o/r.git", "name": "nope" }),
    )
    .await;
    assert_eq!((s, v["code"].as_str()), (400, Some("ssh-only")), "{v}");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_device_key_is_one_stable_public_line_and_the_private_half_is_never_answered() {
    let a = device("keyroute");
    let (s, first, text) = ask(&a, Kind::Key, "GET", "/git/key", None).await;
    assert_eq!(s, 200, "{first}");
    let line = first["publicKey"].as_str().unwrap().to_string();
    assert!(line.starts_with("ssh-ed25519 "), "{line}");
    assert!(!text.contains("PRIVATE"), "{text}");
    let (_, second, _) = ask(&a, Kind::Key, "GET", "/git/key", None).await;
    assert_eq!(second["publicKey"].as_str(), Some(line.as_str()));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_device_with_no_identity_asks_for_one_and_a_commit_never_has_an_empty_author() {
    let a = device("ident");
    let remote = a.dir.join("bare.git");
    bare(&remote);
    let root = ws(&a, "band");
    std::fs::write(root.join("n.md"), "one\n").unwrap();
    let body = json!({ "workspace": "band", "url": url(&remote) });
    post(&a, Kind::Connect, "/git/connect", body).await;
    let (_, v, _) = ask(&a, Kind::Identity, "GET", "/git/identity", None).await;
    assert_eq!(
        (v["name"].as_str(), v["email"].as_str()),
        (Some(""), Some(""))
    );
    let m = json!({ "workspace": "band", "message": "first" });
    let (s, v, _) = post(&a, Kind::Commit, "/git/commit", m.clone()).await;
    assert_eq!((s, v["code"].as_str()), (400, Some("no-identity")));
    who(&a).await;
    let (s, v, _) = post(&a, Kind::Commit, "/git/commit", m).await;
    assert_eq!(s, 200, "{v}");
    let (_, v, _) = ask(&a, Kind::Log, "GET", "/git/log?workspace=band", None).await;
    assert_eq!(v["commits"][0]["author"], "Ann");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_demo_workspace_is_never_connected_to_git() {
    let a = device("demos");
    let remote = a.dir.join("bare.git");
    bare(&remote);
    let root = ws(&a, k7_file_service::workspaces::EXAMPLES);
    let body = json!({ "workspace": k7_file_service::workspaces::EXAMPLES, "url": url(&remote) });
    let (s, v, _) = post(&a, Kind::Connect, "/git/connect", body).await;
    assert_eq!((s, v["code"].as_str()), (400, Some("bad-path")), "{v}");
    assert!(!root.join(".git").exists());
}

#[test]
fn a_server_front_is_refused_and_a_device_front_without_a_mediatheque_is_not() {
    // A device front is told by its own flag, whatever the media holds.
    assert!(super::git::server_refusal(true).is_none());
    let refused = super::git::server_refusal(false).unwrap();
    assert_eq!(refused.status().as_u16(), 403);
}
