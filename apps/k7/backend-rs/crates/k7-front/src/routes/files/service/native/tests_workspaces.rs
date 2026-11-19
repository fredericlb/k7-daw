//! The home page's six routes through a real account (1603 to 1624): the switch, the hook's order, the steps in
//! order, a removal that outlives its client, the lock never held across an ask, the memo, the excuse of a stale root.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::extract::Request;
use k7_file_service::libraries::Home;
use serde_json::{Value, json};

use super::entries::calls;
use super::tests::{Lines, log_to, scratch};
use super::tests_account::text;
use super::workspaces::{Acct, Kind, serve};
use super::workspaces_hook::hook;
use super::workspaces_memo as memo;
use super::workspaces_start::stocked;
use super::{Piece, Tag};

/// An account of its own over `dir`, answering, with a library on disk (a make asks for one).
pub(super) fn ws_acct(dir: &Path, who: &str) -> (Acct, Lines) {
    std::fs::create_dir_all(dir.join("libraries/k7-core/modules")).unwrap();
    let lines = Lines::default();
    let a = Acct {
        runtime: None,
        home: Home {
            studio: crate::routes::files::service::native::tests::slashed(&dir.join("home")),
            libraries: dir.join("libraries").display().to_string(),
        },
        account: who.into(),
        log: log_to(&lines),
        demos_from: (None, String::new()),
        gate: super::answering::global(),
        engine: None,
    };
    (a, lines)
}

pub(super) async fn go(a: &Acct, kind: Kind, body: Value) -> (u16, String) {
    match serve(a, kind, &body).await {
        Some(r) => text(r).await,
        None => (0, "declined".into()),
    }
}

fn root_of(dir: &Path, name: &str) -> String {
    // The account keys its roots by the slashed spelling `resolve_all` gives.
    k7_file_service::paths::resolve_all(&[&dir
        .join("home/workspace")
        .join(name)
        .display()
        .to_string()])
}

#[test]
fn the_six_routes_are_tagged_and_the_rest_are_not_this_piece_s() {
    for (m, p, t) in [
        ("GET", "/workspaces", Tag::Workspaces),
        ("POST", "/workspaces", Tag::WorkspaceMake),
        ("POST", "/workspaces/remove", Tag::WorkspaceRemove),
        ("POST", "/workspaces/copy-project", Tag::CopyProject),
        ("GET", "/workspaces/projects", Tag::WsProjects),
        ("GET", "/workspaces/surfaces", Tag::WsSurfaces),
    ] {
        let method = hyper::Method::from_bytes(m.as_bytes()).unwrap();
        assert_eq!(Kind::of(&method, p).map(Kind::tag), Some(t), "{m} {p}");
        assert_eq!(t.piece(), Piece::Workspaces);
        assert_eq!(t.route(), format!("{m} {p}"));
    }
    // The bench, the sync routes and a wrong method are another hook's, or a 404.
    for (m, p) in [
        ("POST", "/workspaces/bench"),
        ("GET", "/workspaces/sync"),
        ("POST", "/workspaces/sync/download"),
        ("GET", "/workspaces/remove"),
        ("PUT", "/workspaces"),
        ("GET", "/workspaces/"),
    ] {
        let method = hyper::Method::from_bytes(m.as_bytes()).unwrap();
        assert!(Kind::of(&method, p).is_none(), "{m} {p}");
    }
}

fn request(method: &str, path: &str, body: &str, json_type: bool) -> Request {
    let mut b = Request::builder().method(method).uri(path);
    if json_type {
        b = b.header("content-type", "application/json");
    }
    b.body(Body::from(body.to_string())).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_hook_parses_then_checks_the_owner_then_answers() {
    let dir = scratch("w-hook");
    let (a, _) = ws_acct(&dir, "w-hook");
    let post = |b: &str| request("POST", "/workspaces", b, true);
    // A body that does not parse is answered before the owner is asked, as the live router does.
    let r = hook(&a, Kind::Make, || panic!("owner"), post("{nope"))
        .await
        .ok()
        .unwrap();
    assert_eq!(text(r).await.0, 400);
    let r = hook(&a, Kind::Make, || false, post("{\"name\":\"x\"}"))
        .await
        .ok()
        .unwrap();
    assert_eq!(
        text(r).await,
        (
            403,
            r#"{"error":"these workspaces belong to another account"}"#.to_string()
        )
    );
    // zod's words, then the act.
    let r = hook(&a, Kind::Make, || true, post("{\"name\":\"\"}"))
        .await
        .ok()
        .unwrap();
    assert_eq!(
        text(r).await,
        (
            400,
            r#"{"error":"Too small: expected string to have >=1 characters"}"#.to_string()
        )
    );
    let r = hook(&a, Kind::Make, || true, post("{\"name\":\"fresh\"}"))
        .await
        .ok()
        .unwrap();
    assert_eq!(text(r).await, (200, r#"{"created":true}"#.to_string()));
    // A decline hands the request back: no library on disk is the hub's to ask (1662).
    std::fs::remove_dir_all(dir.join("libraries")).unwrap();
    let back = hook(&a, Kind::Make, || true, post("{\"name\":\"two\"}")).await;
    assert!(back.is_err() && !dir.join("home/workspace/two").exists());
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sixty_four_makes_of_one_name_create_it_once_and_pin_it_once() {
    let dir = scratch("w-race");
    let (a, _) = ws_acct(&dir, "w-race");
    let mut all = Vec::new();
    for _ in 0..64 {
        let a = a.clone();
        all.push(tokio::spawn(async move {
            go(&a, Kind::Make, json!({ "name": "once" })).await
        }));
    }
    let mut created = 0;
    for h in all {
        let (status, body) = h.await.unwrap();
        assert_eq!(status, 200);
        created += usize::from(body == r#"{"created":true}"#);
    }
    assert_eq!(created, 1);
    assert_eq!(calls::of(&root_of(&dir, "once")), ["pinsMade"]);
    // The refusals are the route's words.
    for (bad, msg) in [
        ("..", "invalid workspace name: .."),
        ("a/b", "invalid workspace name: a/b"),
    ] {
        assert_eq!(
            go(&a, Kind::Make, json!({ "name": bad })).await,
            (400, json!({ "error": msg }).to_string())
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_removal_asks_in_order_and_outlives_a_client_that_hangs_up() {
    let dir = scratch("w-remove");
    let (a, _) = ws_acct(&dir, "w-remove");
    let doomed = dir.join("home/workspace/doomed");
    std::fs::create_dir_all(doomed.join("p.k7")).unwrap();
    std::fs::create_dir_all(dir.join("home/workspace/.active-projects")).unwrap();
    std::fs::write(dir.join("home/workspace/.active-projects/doomed"), "p.k7").unwrap();
    let root = root_of(&dir, "doomed");
    // The client goes away while `letGo` is held: the removal still ends, in its own task.
    let held = calls::hold(&root, "letGo");
    let asking = a.clone();
    let client =
        tokio::spawn(async move { go(&asking, Kind::Remove, json!({ "name": "doomed" })).await });
    while calls::of(&root).is_empty() {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    client.abort();
    let _ = client.await;
    assert!(doomed.exists(), "nothing is removed before letGo returns");
    // `forget` is sent only once the folder and its marker are gone (1621): held, the disk is already clean.
    let forgetting = calls::hold(&root, "forget");
    held.add_permits(1);
    for _ in 0..1000 {
        if calls::of(&root).len() == 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(calls::of(&root), ["letGo", "forget"]);
    assert!(!doomed.exists(), "forget was sent before the folder went");
    assert!(!dir.join("home/workspace/.active-projects/doomed").exists());
    forgetting.add_permits(1);
    assert!(!dir.join("home/workspace/.active-projects/doomed").exists());
    // `forget` comes after the folder is gone, `letGo` before anything (1621).
    assert_eq!(calls::of(&root), ["letGo", "forget"]);
    // The refusals: a bad name, one absent, and the memo of the listing dropped.
    assert_eq!(
        go(&a, Kind::Remove, json!({ "name": "doomed" })).await,
        (400, r#"{"error":"no such workspace: doomed"}"#.to_string())
    );
    assert_eq!(go(&a, Kind::Remove, json!({ "name": ".." })).await.0, 400);
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn no_lock_is_held_across_let_go_so_another_act_goes_on() {
    let dir = scratch("w-lock");
    let (a, _) = ws_acct(&dir, "w-lock");
    std::fs::create_dir_all(dir.join("home/workspace/slow")).unwrap();
    let root = root_of(&dir, "slow");
    let held = calls::hold(&root, "letGo");
    let asking = a.clone();
    let removal =
        tokio::spawn(async move { go(&asking, Kind::Remove, json!({ "name": "slow" })).await });
    while calls::of(&root).is_empty() {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    // `letGo` hangs; the account's other acts are served meanwhile.
    let bound = if cfg!(target_env = "musl") { 60 } else { 15 };
    let other = tokio::time::timeout(
        Duration::from_secs(bound),
        go(&a, Kind::Make, json!({ "name": "other" })),
    )
    .await
    .expect("a make waited on a hung letGo");
    assert_eq!(other, (200, r#"{"created":true}"#.to_string()));
    held.add_permits(1);
    assert_eq!(removal.await.unwrap().0, 200);
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_listing_memo_follows_acts_and_the_roots_mtime() {
    let dir = scratch("w-memo");
    let (a, _) = ws_acct(&dir, "w-memo");
    let ws = dir.join("home/workspace");
    let list = |a: &Acct| {
        let a = a.clone();
        async move { go(&a, Kind::Projects, json!({})).await }
    };
    let names = |body: &str| -> Vec<String> {
        let v: Value = serde_json::from_str(body).unwrap();
        v["workspaces"]
            .as_array()
            .unwrap()
            .iter()
            .map(|w| w["name"].as_str().unwrap().to_string())
            .collect()
    };
    assert_eq!(names(&list(&a).await.1), ["songs"]);
    // A project made below a workspace does not move the root: the window keeps the old answer.
    std::fs::create_dir_all(ws.join("songs/late.k7")).unwrap();
    let held = list(&a).await.1;
    assert!(!held.contains("late.k7"), "{held}");
    // A workspace made behind the front's back (a room's join) moves the root's mtime and shows at once.
    std::thread::sleep(Duration::from_millis(50));
    std::fs::create_dir_all(ws.join("joined")).unwrap();
    let after = list(&a).await.1;
    assert_eq!(names(&after), ["joined", "songs"]);
    assert!(after.contains("late.k7"));
    // An act of this piece drops the memo.
    let before = memo::acts("w-memo");
    assert_eq!(go(&a, Kind::Make, json!({ "name": "made" })).await.0, 200);
    assert_eq!(memo::acts("w-memo"), before + 1);
    assert_eq!(names(&list(&a).await.1), ["joined", "made", "songs"]);
    // `/surfaces` comes from the same walk.
    let surfaced = go(&a, Kind::Surfaces, json!({})).await;
    assert_eq!(surfaced, (200, r#"{"projects":[]}"#.to_string()));
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_copy_joins_both_ends_first_and_tells_the_list_after_its_writes() {
    let dir = scratch("w-copy");
    let (a, _) = ws_acct(&dir, "w-copy");
    let ws = dir.join("home/workspace");
    std::fs::create_dir_all(ws.join("songs/s.k7")).unwrap();
    std::fs::write(ws.join("songs/s.k7/project.k7x"), "project({})\n").unwrap();
    std::fs::create_dir_all(ws.join("other")).unwrap();
    let ask = json!({ "from": "songs", "path": "s.k7", "to": "other", "name": "c.k7" });
    assert_eq!(
        go(&a, Kind::Copy, ask.clone()).await,
        (200, r#"{"project":"c.k7"}"#.to_string())
    );
    assert!(ws.join("other/c.k7/project.k7x").is_file());
    assert_eq!(calls::of(&root_of(&dir, "other")), ["listed"]);
    // The refusals, in the TypeScript's words; the dest that is gone is a 404.
    assert_eq!(
        go(&a, Kind::Copy, ask).await,
        (400, r#"{"error":"\"c.k7\" already exists"}"#.to_string())
    );
    let gone = json!({ "from": "songs", "path": "s.k7", "to": "nowhere", "name": "c.k7" });
    assert_eq!(
        go(&a, Kind::Copy, gone).await,
        (404, r#"{"error":"unknown workspace: nowhere"}"#.to_string())
    );
    let thin = json!({ "from": "songs", "path": "s.k7", "to": "other" });
    assert_eq!(go(&a, Kind::Copy, thin).await.0, 400);
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_copy_to_a_missing_workspace_under_a_native_capture_is_a_404_and_makes_no_folder() {
    let dir = scratch("w-copy-cap");
    let (a, _) = ws_acct(&dir, "w-copy-cap");
    let ws = dir.join("home/workspace");
    std::fs::create_dir_all(ws.join("songs/s.k7")).unwrap();
    std::fs::write(ws.join("songs/s.k7/project.k7x"), "project({})\n").unwrap();
    let rt = tokio::runtime::Handle::current();
    let host = super::tests_capture::host(&a.home.studio);
    k7_capture::Captures::global()
        .start_account(Arc::new(|_| {}), host, &rt)
        .await;
    let gone = json!({ "from": "songs", "path": "s.k7", "to": "nowhere", "name": "c.k7" });
    assert_eq!(
        go(&a, Kind::Copy, gone).await,
        (404, r#"{"error":"unknown workspace: nowhere"}"#.to_string())
    );
    assert!(
        !ws.join("nowhere").exists(),
        "the join made the missing workspace"
    );
    k7_capture::Captures::global()
        .stop_account(&a.home.studio)
        .await;
    // A capture's late index write can land inside the folder, so the removal is retried.
    let mut left = None;
    for _ in 0..600 {
        match std::fs::remove_dir_all(&dir) {
            Ok(()) => {
                left = None;
                break;
            }
            Err(e) => left = Some(e),
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(left.is_none(), "the scratch folder stays: {left:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_remove_and_a_copy_into_it_at_once_end_with_the_workspace_gone() {
    let dir = scratch("w-both");
    let (a, _) = ws_acct(&dir, "w-both");
    let ws = dir.join("home/workspace");
    std::fs::create_dir_all(ws.join("songs/s.k7")).unwrap();
    std::fs::create_dir_all(ws.join("target")).unwrap();
    let copy = json!({ "from": "songs", "path": "s.k7", "to": "target", "name": "c.k7" });
    let (x, y) = (a.clone(), a.clone());
    let removal =
        tokio::spawn(async move { go(&x, Kind::Remove, json!({ "name": "target" })).await });
    let copying = tokio::spawn(async move { go(&y, Kind::Copy, copy).await });
    let (removed, copied) = (removal.await.unwrap(), copying.await.unwrap());
    assert_eq!(removed.0, 200);
    // Served in arrival order: the copy either landed before the removal took it, or found no workspace.
    assert!(
        copied.0 == 200 || copied.1 == r#"{"error":"unknown workspace: target"}"#,
        "{copied:?}"
    );
    assert!(!ws.join("target").exists());
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn examples_are_stocked_before_the_actor_and_the_bundle_is_told_not_to() {
    let dir = scratch("w-start");
    let demos: PathBuf = dir.join("demos");
    std::fs::create_dir_all(demos.join("synth.k7")).unwrap();
    std::fs::write(demos.join("synth.k7/project.k7x"), "project({})\n").unwrap();
    let env = std::collections::BTreeMap::from([
        (
            "STUDIO_HOME".to_string(),
            crate::routes::files::service::native::tests::slashed(&dir.join("home")),
        ),
        ("STUDIO_DEMOS".to_string(), demos.display().to_string()),
    ]);
    let lines = Lines::default();
    let log = log_to(&lines);
    let code = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(stocked(&env, code, &log).await);
    assert!(
        dir.join("home/workspace/Examples/synth.k7/project.k7x")
            .is_file()
    );
    assert!(stocked(&env, code, &log).await);
    assert_eq!(
        *lines.lock().unwrap(),
        ["   Examples: synth.k7", "   Examples: demos up to date"]
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_name_of_the_folders_other_case_is_asked_in_the_folders_own_spelling() {
    let dir = scratch("w-case");
    let ws = dir.join("home/workspace");
    std::fs::create_dir_all(ws.join("Casey/s.k7")).unwrap();
    std::fs::create_dir_all(ws.join("Target")).unwrap();
    // A disk that keeps cases apart (ext4) does not know `casey` for `Casey`: nothing to resolve there (1632).
    if std::fs::metadata(ws.join("casey")).is_err() {
        std::fs::remove_dir_all(dir).unwrap();
        return;
    }
    let (a, _) = ws_acct(&dir, "w-case");
    let copy = json!({ "from": "casey", "path": "s.k7", "to": "target", "name": "c.k7" });
    assert_eq!(go(&a, Kind::Copy, copy).await.0, 200);
    // The destination is told its list changed under the name it has on disk, not the one asked.
    assert_eq!(calls::of(&root_of(&dir, "Target")), ["listed"]);
    assert!(calls::of(&root_of(&dir, "target")).is_empty());
    assert_eq!(
        go(&a, Kind::Remove, json!({ "name": "casey" })).await.0,
        200
    );
    let seen = (
        calls::of(&root_of(&dir, "Casey")),
        calls::of(&root_of(&dir, "casey")),
    );
    assert_eq!(seen.0, ["letGo", "forget"], "{seen:?}");
    assert!(calls::of(&root_of(&dir, "casey")).is_empty());
    assert!(!ws.join("Casey").exists());
    std::fs::remove_dir_all(dir).unwrap();
}
