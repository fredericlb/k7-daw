//! tests_git_autosave.rs — auto-save through the front over a bare `file://` remote: a round snapshots and pushes, the
//! page leaving the workspace and the front stopping flush at once, a Save resets the branch, and who names the branch.
//! What a reader gets wrong: the 20-s rule is `Idle`'s, tested with its own clock; here a wait is a bounded poll of the remote.

use std::path::Path;
use std::time::Duration;

use serde_json::json;

use super::git::Kind;
use super::git_autosave::{branch_for, round};

use super::git_autosave_task::flush_all;
use super::tests_git::{Dev, ask, bare, device, post, url, who, ws};
use super::tests_git_forgejo::{added, kept, sign_in};

const BRANCH: &str = "k7/autosave/ann-at-example.org";

/// A workspace connected to a fresh bare remote and saved once.
async fn connected(tag: &str) -> (Dev, std::path::PathBuf, std::path::PathBuf) {
    let d = device(tag);
    let remote = d.dir.join("remote.git");
    bare(&remote);
    let root = ws(&d, "band");
    std::fs::write(root.join("song.k7x"), "a: 1\n").unwrap();
    who(&d).await;
    let body = json!({ "workspace": "band", "url": url(&remote) });
    assert_eq!(post(&d, Kind::Connect, "/git/connect", body).await.0, 200);
    let body = json!({ "workspace": "band", "message": "first" });
    assert_eq!(post(&d, Kind::Commit, "/git/commit", body).await.0, 200);
    assert_eq!(
        post(&d, Kind::Push, "/git/push", json!({ "workspace": "band" }))
            .await
            .0,
        200
    );
    (d, root, remote)
}

fn remote_ref(remote: &Path, name: &str) -> Option<String> {
    let r = git2::Repository::open(remote).ok()?;
    r.refname_to_id(name).ok().map(|o| o.to_string())
}

fn remote_text(remote: &Path, path: &str) -> Option<String> {
    let r = git2::Repository::open(remote).ok()?;
    let id = r.refname_to_id(&format!("refs/heads/{BRANCH}")).ok()?;
    let tree = r.find_commit(id).ok()?.tree().ok()?;
    let blob = r
        .find_blob(tree.get_path(Path::new(path)).ok()?.id())
        .ok()?;
    Some(String::from_utf8_lossy(blob.content()).into_owned())
}

/// A round that waits out one a route started (a push starts one): skipped rounds are tried again.
async fn settled(d: &Dev, root: &Path) -> super::git_autosave::Round {
    for _ in 0..50 {
        let r = round(&d.ctx, root, true, false).await.unwrap();
        if !r.skipped {
            return r;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("every round was skipped");
}

/// Polls the remote until `ok` holds, for at most five seconds.
async fn until(mut ok: impl FnMut() -> bool) -> bool {
    for _ in 0..50 {
        if ok() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    false
}

#[tokio::test(flavor = "multi_thread")]
async fn a_round_snapshots_the_edit_and_pushes_the_branch_and_the_state_says_when() {
    let (d, root, remote) = connected("autosave-round").await;
    let r = settled(&d, &root).await;
    assert!(!r.snapshot, "nothing changed since the save");
    std::fs::write(root.join("song.k7x"), "a: 2\n").unwrap();
    let r = settled(&d, &root).await;
    assert!(r.snapshot && !r.unsent, "{r:?}");
    assert_eq!(remote_text(&remote, "song.k7x").as_deref(), Some("a: 2\n"));
    let (_, v, _) = ask(&d, Kind::State, "GET", "/git/state?workspace=band", None).await;
    assert_eq!(v["autosave"]["branch"], BRANCH);
    assert_eq!(v["autosave"]["unsent"], false);
    assert!(
        v["autosave"]["at"].as_i64().unwrap() > 1_000_000_000_000,
        "milliseconds"
    );
    assert_eq!(v["changed"], 1, "the edit is still unsaved on main");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_page_leaving_the_workspace_flushes_at_once() {
    let (d, root, remote) = connected("autosave-away").await;
    std::fs::write(root.join("song.k7x"), "a: away\n").unwrap();
    let body = json!({ "workspace": "band", "front": false });
    assert_eq!(post(&d, Kind::Focus, "/git/focus", body).await.0, 200);
    assert!(
        until(|| remote_text(&remote, "song.k7x").as_deref() == Some("a: away\n")).await,
        "no 20-s wait once the page is away"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_front_stopping_flushes_every_workspace() {
    let (d, root, remote) = connected("autosave-stop").await;
    super::git_autosave_task::ensure(&d.ctx, &root);
    std::fs::write(root.join("song.k7x"), "a: last\n").unwrap();
    flush_all(Duration::from_secs(10)).await;
    assert_eq!(
        remote_text(&remote, "song.k7x").as_deref(),
        Some("a: last\n")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_save_squashes_onto_main_and_the_branch_follows_it_on_the_remote() {
    let (d, root, remote) = connected("autosave-save").await;
    for i in 0..3 {
        std::fs::write(root.join("song.k7x"), format!("a: {i}\n")).unwrap();
        settled(&d, &root).await;
    }
    let before = remote_ref(&remote, "refs/heads/main").unwrap();
    let body = json!({ "workspace": "band", "message": "Saved on 26/11/2026 10:00" });
    let (s, v, _) = post(&d, Kind::Commit, "/git/commit", body).await;
    assert_eq!(s, 200, "{v}");
    let save = v["id"].as_str().unwrap().to_string();
    assert_eq!(
        post(&d, Kind::Push, "/git/push", json!({ "workspace": "band" }))
            .await
            .0,
        200
    );
    let r = git2::Repository::open(&remote).unwrap();
    let c = r.find_commit(git2::Oid::from_str(&save).unwrap()).unwrap();
    assert_eq!(
        c.parent_id(0).unwrap().to_string(),
        before,
        "one squash commit"
    );
    let name = format!("refs/heads/{BRANCH}");
    assert!(
        until(|| remote_ref(&remote, &name).as_deref() == Some(save.as_str())).await,
        "the branch restarts from the save on the remote too"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_custom_remote_names_the_branch_by_the_email_and_a_forge_by_the_account() {
    let (d, root, _remote) = connected("autosave-names").await;
    let repo = super::git::open(&root).unwrap();
    assert_eq!(branch_for(&d.ctx, &repo).as_deref(), Some(BRANCH));
    let (f, _fake, id) = added("autosave-forge").await;
    sign_in(&f, &id).await;
    let login = kept(&f, &id)["login"].as_str().unwrap().to_lowercase();
    let at = ws(&f, "band");
    let g = git2::Repository::init(&at).unwrap();
    g.remote("origin", "ssh://git@127.0.0.1/flb/x.git").unwrap();
    let repo = super::git::open(&at).unwrap();
    assert_eq!(
        branch_for(&f.ctx, &repo),
        Some(format!("k7/autosave/{login}"))
    );
}
