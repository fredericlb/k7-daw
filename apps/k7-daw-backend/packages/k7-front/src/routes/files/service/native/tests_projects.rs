//! The three POSTs and the poll through a real account (2000 to 2095): the hook's order, an act that outlives its
//! client, no lock across an ask, and the marker's bytes.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::extract::Request;
use k7_capture::account::Account as Capture;
use k7_file_service::libraries::Home;
use serde_json::{Value, json};

use super::entries::calls;
use super::projects::{Kind, Proj, hook};
use super::tests::{Lines, log_to, scratch, until};
use super::tests_account::text;
use super::{Piece, Tag, locks};

#[test]
fn the_four_routes_are_tagged() {
    for (m, p, k, t) in [
        ("POST", "/open", Kind::Open, Tag::Open),
        ("POST", "/project/rename", Kind::Rename, Tag::ProjectRename),
        ("POST", "/project/delete", Kind::Recycle, Tag::ProjectDelete),
    ] {
        let method = hyper::Method::from_bytes(m.as_bytes()).unwrap();
        assert_eq!(Kind::of(&method, p), Some(k));
        assert_eq!(
            (k.tag(), t.piece(), t.route()),
            (t, Piece::Projects, format!("{m} {p}").as_str())
        );
    }
    for (m, p) in [
        ("GET", "/open"),
        ("POST", "/project"),
        ("POST", "/project/create"),
    ] {
        let method = hyper::Method::from_bytes(m.as_bytes()).unwrap();
        assert!(Kind::of(&method, p).is_none(), "{m} {p}");
    }
    assert_eq!(
        (Tag::Project.piece(), Tag::Project.route()),
        (Piece::Projects, "GET /project")
    );
}

pub(super) fn proj(dir: &Path, who: &str) -> (Proj, Lines) {
    std::fs::create_dir_all(dir.join("libraries/k7-core/modules")).unwrap();
    let lines = Lines::default();
    let p = Proj {
        runtime: None,
        home: Home {
            studio: crate::routes::files::service::native::tests::slashed(&dir.join("home")),
            libraries: dir.join("libraries").display().to_string(),
        },
        account: who.into(),
        log: log_to(&lines),
        demos_from: (None, String::new()),
        engine: None,
        site: None,
        capture: Some(Arc::new(Capture::new(Arc::new(|_| {})))),
        hub: None,
        media: None,
        max_seen: None,
    };
    (p, lines)
}

pub(super) fn root_of(dir: &Path) -> String {
    // The slashed spelling the account keys its roots by.
    k7_file_service::paths::resolve_all(&[&dir.join("home/workspace/songs").display().to_string()])
}

fn request(body: &str) -> Request {
    Request::builder()
        .method("POST")
        .uri("/x")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

pub(super) async fn go(p: &Proj, kind: Kind, dir: &Path, body: Value) -> (u16, String) {
    let root = root_of(dir);
    let resolve = |_: &hyper::HeaderMap| Ok(("songs".to_string(), root));
    match hook(p, kind, resolve, request(&body.to_string())).await {
        Ok(r) => text(r).await,
        Err(_) => (0, "declined".into()),
    }
}

/// A real project of the crate's own scaffold, made natively.
pub(super) async fn make(dir: &Path, name: &str) {
    let home = k7_file_service::libraries::Home::of(&super::tests_docs::env_of(dir));
    let root = root_of(dir);
    let code = super::tests_docs::code_root().display().to_string();
    let mut mint = || {
        let mut b = [0u8; 16];
        let _ = getrandom::fill(&mut b);
        k7_file_service::media::models::uuid_v4(b)
    };
    let space = k7_file_service::space::Space::open(&home, &root);
    k7_file_service::edits::project_create::create_project(
        &home,
        &code,
        &space,
        "arrangement",
        name,
        &mut mint,
    )
    .unwrap();
}

pub(super) fn marker(dir: &Path) -> String {
    std::fs::read_to_string(dir.join("home/workspace/.active-projects/songs")).unwrap_or_default()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_hook_parses_resolves_then_checks_the_fields() {
    let dir = scratch("p-hook");
    let (p, _) = proj(&dir, "p-hook");
    let none = |_: &hyper::HeaderMap| -> Result<(String, String), Box<axum::response::Response>> {
        panic!("resolved")
    };
    // A body that does not parse is answered before the owner and the workspace are asked.
    let r = hook(&p, Kind::Open, none, request("{nope"))
        .await
        .ok()
        .unwrap();
    assert_eq!(text(r).await.0, 400);
    let root = root_of(&dir);
    let refused = |_: &hyper::HeaderMap| {
        Err(Box::new(super::projects::error(
            403,
            "these workspaces belong to another account",
        )))
    };
    let r = hook(&p, Kind::Open, refused, request("{\"path\":\"a.k7\"}"))
        .await
        .ok()
        .unwrap();
    assert_eq!(text(r).await.0, 403);
    // zod's words, after the workspace and before any ask.
    for (kind, body, msg) in [
        (
            Kind::Open,
            "{\"path\":\"\"}",
            "Too small: expected string to have >=1 characters",
        ),
        (
            Kind::Rename,
            "{\"path\":\"a.k7\"}",
            "Invalid input: expected string, received undefined",
        ),
        (
            Kind::Recycle,
            "{}",
            "Invalid input: expected string, received undefined",
        ),
    ] {
        let ok = |_: &hyper::HeaderMap| Ok(("songs".to_string(), root.clone()));
        let r = hook(&p, kind, ok, request(body)).await.ok().unwrap();
        assert_eq!(
            text(r).await,
            (400, json!({ "error": msg }).to_string()),
            "{body}"
        );
    }
    assert!(calls::of(&root).is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_open_writes_the_marker_once_and_answers_in_key_order() {
    let dir = scratch("p-open");
    let (p, _) = proj(&dir, "p-open");
    for n in ["a.k7", "b.k7"] {
        make(&dir, n).await;
    }
    let _root = root_of(&dir);
    let (status, body) = go(&p, Kind::Open, &dir, json!({ "path": "a.k7" })).await;
    assert_eq!(status, 200, "{body}");
    let v: Value = serde_json::from_str(&body).unwrap();
    let keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(
        &keys[..4],
        ["root", "rootIsProject", "activeProject", "projects"],
        "{body}"
    );
    assert_eq!(
        (&v["activeProject"], &v["projects"]),
        (&json!("a.k7"), &json!(["a.k7", "b.k7"]))
    );
    assert_eq!(marker(&dir), "a.k7\n");
    // 2022: the same project again leaves the marker's file as it was.
    let stamp = || {
        std::fs::metadata(dir.join("home/workspace/.active-projects/songs"))
            .unwrap()
            .modified()
            .unwrap()
    };
    let was = stamp();
    std::thread::sleep(Duration::from_millis(30));
    assert_eq!(
        go(&p, Kind::Open, &dir, json!({ "path": "a.k7" })).await.0,
        200
    );
    assert_eq!(stamp(), was);
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_open_refuses_in_the_typescripts_order_and_writes_nothing() {
    let dir = scratch("p-refuse");
    let (p, _) = proj(&dir, "p-refuse");
    make(&dir, "a.k7").await;
    std::fs::create_dir_all(dir.join("home/workspace/songs/plain")).unwrap();
    std::fs::create_dir_all(dir.join("home/workspace/songs/_recycle_bin/x.k7")).unwrap();
    for (path, starts) in [
        ("gone.k7", "no project 'gone.k7' in this workspace"),
        ("plain", "not a project (the folder must end in "),
        (
            "_recycle_bin/x.k7",
            "project is in the recycle bin: _recycle_bin/x.k7",
        ),
    ] {
        let (status, body) = go(&p, Kind::Open, &dir, json!({ "path": path })).await;
        let msg = serde_json::from_str::<Value>(&body).unwrap()["error"]
            .as_str()
            .unwrap()
            .to_string();
        assert_eq!(status, 400, "{path}");
        assert!(msg.starts_with(starts), "{path}: {msg}");
    }
    assert_eq!(marker(&dir), "");
    assert!(!calls::of(&root_of(&dir)).contains(&"activate".to_string()));
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_rename_and_a_recycle_move_the_marker_and_tell_the_memory() {
    let dir = scratch("p-move");
    let (p, _) = proj(&dir, "p-move");
    for n in ["a.k7", "b.k7"] {
        make(&dir, n).await;
    }
    let root = root_of(&dir);
    assert_eq!(
        go(&p, Kind::Open, &dir, json!({ "path": "a.k7" })).await.0,
        200
    );
    let (status, body) = go(
        &p,
        Kind::Rename,
        &dir,
        json!({ "path": "a.k7", "name": "c.k7" }),
    )
    .await;
    assert_eq!((status, body.as_str()), (200, r#"{"path":"c.k7"}"#));
    assert_eq!(marker(&dir), "c.k7\n");
    let after_open = calls::of(&root);
    assert_eq!(
        &after_open[after_open.len() - 2..],
        ["releaseFolder", "moved"]
    );
    // The name it has: answered, nothing asked.
    let (status, body) = go(
        &p,
        Kind::Rename,
        &dir,
        json!({ "path": "c.k7", "name": "c.k7" }),
    )
    .await;
    assert_eq!((status, body.as_str()), (200, r#"{"path":"c.k7"}"#));
    assert_eq!(calls::of(&root).len(), after_open.len());
    // A recycle of the open project empties the marker; the same name again takes `-2`.
    let (status, body) = go(&p, Kind::Recycle, &dir, json!({ "path": "c.k7" })).await;
    assert_eq!(
        (status, body.as_str()),
        (200, r#"{"recycled":"_recycle_bin/c.k7"}"#)
    );
    assert_eq!(marker(&dir), "");
    make(&dir, "c.k7").await;
    let (_, body) = go(&p, Kind::Recycle, &dir, json!({ "path": "c.k7" })).await;
    assert_eq!(body, r#"{"recycled":"_recycle_bin/c.k7-2"}"#);
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_unload_holds_no_lock_and_an_act_outlives_its_client() {
    let dir = scratch("p-lock");
    let (p, _) = proj(&dir, "p-lock");
    make(&dir, "a.k7").await;
    let root = root_of(&dir);
    // 2060: `releaseFolder` hangs, and the account's lock is still free.
    let held = calls::hold(&root, "releaseFolder");
    let asking = p.clone();
    let d2 = dir.clone();
    let client =
        tokio::spawn(
            async move { go(&asking, Kind::Recycle, &d2, json!({ "path": "a.k7" })).await },
        );
    until("releaseFolder is asked", || {
        calls::of(&root).contains(&"releaseFolder".to_string())
    })
    .await;
    let bound = if cfg!(target_env = "musl") { 60 } else { 15 };
    tokio::time::timeout(Duration::from_secs(bound), locks::write(&p.home.studio))
        .await
        .expect("the account's lock was held across releaseFolder");
    // 2043: the client goes, the recycle still ends.
    client.abort();
    let _ = client.await;
    assert!(dir.join("home/workspace/songs/a.k7").exists());
    held.add_permits(1);
    until("the folder is recycled", || {
        dir.join("home/workspace/songs/_recycle_bin/a.k7").exists()
    })
    .await;
    until("moved is asked", || {
        calls::of(&root).contains(&"moved".to_string())
    })
    .await;
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_poll_answers_name_and_root() {
    let dir = scratch("p-poll");
    let (p, lines) = proj(&dir, "p-poll");
    make(&dir, "a.k7").await;
    assert_eq!(
        go(&p, Kind::Open, &dir, json!({ "path": "a.k7" })).await.0,
        200
    );
    let c = super::tests::ctx(&dir, Tag::Project, "", &lines);
    let ours = super::projects::answer(&c).await.expect("answered");
    let (status, body) = text(ours).await;
    assert_eq!(status, 200);
    let want = json!({ "name": "a.k7", "root": format!("{}/a.k7", root_of(&dir)) });
    assert_eq!(serde_json::from_str::<Value>(&body).unwrap(), want);
    std::fs::remove_dir_all(dir).unwrap();
}

pub(super) fn respond((status, text): (u16, String)) -> axum::response::Response {
    super::projects::raw(status, text)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_marker_that_cannot_be_written_fails_the_open_before_activate_and_a_tree_opens_bare() {
    let dir = scratch("p-marker");
    let (p, lines) = proj(&dir, "p-marker");
    let root = root_of(&dir);
    // 2030: a workspace with no project at all opens as a tree, with nothing set and nothing written.
    let (status, body) = go(&p, Kind::Open, &dir, json!({ "path": "." })).await;
    assert_eq!(status, 200, "{body}");
    assert!(body.contains("\"activeProject\":null"), "{body}");
    assert_eq!(marker(&dir), "");
    // 2093: `.active-projects` is a file, so the marker cannot be written; no activation is ever asked.
    make(&dir, "a.k7").await;
    std::fs::write(dir.join("home/workspace/.active-projects"), "x").unwrap();
    let (status, body) = go(&p, Kind::Open, &dir, json!({ "path": "a.k7" })).await;
    assert_eq!(status, 500, "{body}");
    assert!(
        body.starts_with(r#"{"error":"the file could not be changed ("#),
        "{body}"
    );
    assert!(!calls::of(&root).contains(&"activate".to_string()));
    assert!(
        lines
            .lock()
            .unwrap()
            .iter()
            .any(|l| l.starts_with("[files] "))
    );
    std::fs::remove_dir_all(dir).unwrap();
}
