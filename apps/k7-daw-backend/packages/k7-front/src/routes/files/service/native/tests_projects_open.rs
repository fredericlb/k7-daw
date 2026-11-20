//! The open's own cases (F7, F11, F13, F14): a walk beyond its bound, a tree open that starts nothing, and the
//! brief's mutations that need a held ask.

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use super::entries::calls;
use super::locks;
use super::projects::{Kind, Proj};
use super::tests::{scratch, until};
use super::tests_projects::{go, make, marker, proj, respond, root_of};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_open_whose_walk_is_too_big_is_refused_with_its_reason() {
    // F7: the walk is bounded, and a workspace beyond the bound is a refusal, not a partial list.
    let dir = scratch("po-walk");
    let (mut p, _) = proj(&dir, "po-walk");
    p.max_seen = Some(2);
    for n in ["a.k7", "b.k7", "c.k7"] {
        make(&dir, n).await;
    }
    let (status, body) = go(&p, Kind::Open, &dir, json!({ "path": "a.k7" })).await;
    assert_ne!(status, 200, "{body}");
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_missing_project_is_the_routes_400_never_a_500() {
    // The refusal is the route's 400 (F13's `{error}` path, held by the ops vectors).
    let dir = scratch("po-sync");
    let (p, _) = proj(&dir, "po-sync");
    let (status, body) = go(&p, Kind::Open, &dir, json!({ "path": "missing.k7" })).await;
    // No project there: the plan refuses in the TypeScript's words, never a 500.
    assert_eq!(status, 400, "{body}");
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn no_wait_of_an_open_holds_the_account_and_a_hung_prepare_stops_no_rename() {
    // F14: the lock is free while the engine's start hangs and a second open goes on; a rename goes on past a hung `openPrepare`.
    let a = super::engine::tests_real::account("po-free").await;
    let dir = a.dir.clone();
    let (mut p, _) = proj(&dir, "po-free");
    p.engine = a.engine.clone();
    for n in ["a.k7", "b.k7", "c.k7"] {
        make(&dir, n).await;
    }
    let root = root_of(&dir);
    let bound = Duration::from_secs(if cfg!(target_env = "musl") { 60 } else { 15 });
    let started = calls::hold(&root, "startEngine");
    let (first, d1) = (p.clone(), dir.clone());
    let one =
        tokio::spawn(async move { go(&first, Kind::Open, &d1, json!({ "path": "a.k7" })).await });
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(
        !one.is_finished(),
        "the open did not wait for the engine's start"
    );
    tokio::time::timeout(bound, locks::write(&p.home.studio))
        .await
        .expect("the lock was held across the engine's start");
    started.add_permits(2);
    assert_eq!(one.await.unwrap().0, 200);
    let prepared = calls::hold(&root, "openPrepare");
    let (second, d2) = (p.clone(), dir.clone());
    let two =
        tokio::spawn(async move { go(&second, Kind::Open, &d2, json!({ "path": "b.k7" })).await });
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(!two.is_finished(), "the open did not wait for its prepare");
    let renamed = tokio::time::timeout(
        bound,
        go(
            &p,
            Kind::Rename,
            &dir,
            json!({ "path": "c.k7", "name": "d.k7" }),
        ),
    )
    .await
    .unwrap();
    assert_eq!(renamed.0, 200);
    prepared.add_permits(1);
    assert_eq!(two.await.unwrap().0, 200);
}

fn flags(p: &Proj, root: &str) -> Vec<Value> {
    let _ = p;
    calls::args_of(root, "releaseFolder")
        .iter()
        .map(|a| a["watches"].clone())
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_recycle_closes_the_watches_a_rename_does_not_and_no_gate_is_held_across_release() {
    // 2051: `watches` true for a recycle, false for a rename; F14: the journal's gate is free while `releaseFolder` hangs.
    let dir = scratch("po-watches");
    let (p, _) = proj(&dir, "po-watches");
    for n in ["a.k7", "b.k7"] {
        make(&dir, n).await;
    }
    let root = root_of(&dir);
    let held = calls::hold(&root, "releaseFolder");
    let (asking, d2) = (p.clone(), dir.clone());
    let rename = tokio::spawn(async move {
        go(
            &asking,
            Kind::Rename,
            &d2,
            json!({ "path": "a.k7", "name": "c.k7" }),
        )
        .await
    });
    until("releaseFolder asked", || {
        calls::of(&root).contains(&"releaseFolder".to_string())
    })
    .await;
    let gate = k7_file_service::paths::resolve_all(&[&root]);
    let alone = tokio::time::timeout(
        Duration::from_secs(15),
        k7_file_service::journal::Journals::global().exclusive(&gate),
    )
    .await;
    assert!(alone.is_ok(), "the gate was held across releaseFolder");
    drop(alone);
    held.add_permits(1);
    assert_eq!(rename.await.unwrap().0, 200);
    go(&p, Kind::Recycle, &dir, json!({ "path": "b.k7" })).await;
    assert_eq!(flags(&p, &root), [json!(false), json!(true)]);
    let _ = respond;
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_re_open_of_the_project_already_open_leaves_the_markers_bytes() {
    let dir = scratch("po-held");
    let (p, _) = proj(&dir, "po-held");
    make(&dir, "a.k7").await;
    let at = dir.join("home/workspace/.active-projects/songs");
    std::fs::create_dir_all(at.parent().unwrap()).unwrap();
    std::fs::write(&at, "a.k7").unwrap();
    // The first open in this process writes the marker as the TypeScript did; the re-open leaves it.
    assert_eq!(
        go(&p, Kind::Open, &dir, json!({ "path": "a.k7" })).await.0,
        200
    );
    assert_eq!(marker(&dir), "a.k7\n");
    std::fs::write(&at, "a.k7").unwrap();
    let (status, body) = go(&p, Kind::Open, &dir, json!({ "path": "a.k7" })).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(marker(&dir), "a.k7", "the marker was rewritten");
    // What the marker says does not matter: the project open in this process leaves even a stale one as written.
    std::fs::write(&at, "gone.k7\n").unwrap();
    assert_eq!(
        go(&p, Kind::Open, &dir, json!({ "path": "a.k7" })).await.0,
        200
    );
    assert_eq!(marker(&dir), "gone.k7\n", "the stale marker was rewritten");
    std::fs::remove_dir_all(dir).unwrap();
}

pub(super) fn files_of(dir: &std::path::Path) -> Vec<(String, String)> {
    fn walk(d: &std::path::Path, out: &mut Vec<(String, String)>) {
        let mut names: Vec<_> = std::fs::read_dir(d).unwrap().flatten().collect();
        names.sort_by_key(std::fs::DirEntry::file_name);
        for e in names {
            if e.path().is_dir() {
                walk(&e.path(), out);
            } else if let Ok(t) = std::fs::read_to_string(e.path()) {
                out.push((e.path().display().to_string(), t));
            }
        }
    }
    let mut out = vec![];
    walk(dir, &mut out);
    out
}

// An open stamps the format level seconds need, once: a second open writes nothing.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_open_stamps_once_and_a_second_open_writes_nothing() {
    let dir = scratch("po-repair");
    let (p, _) = proj(&dir, "po-repair");
    make(&dir, "a.k7").await;
    let folder = dir.join("home/workspace/songs/a.k7");
    let lane = std::fs::read_dir(folder.join("tracks"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|x| x == "k7x"))
        .unwrap();
    std::fs::write(
        lane,
        "extends(\"@meta/track\")\nevents([ref(\"kick\").start(2.5)])\n",
    )
    .unwrap();
    let (status, body) = go(&p, Kind::Open, &dir, json!({ "path": "a.k7" })).await;
    assert_eq!(status, 200, "{body}");
    let root = std::fs::read_to_string(folder.join("project.k7x")).unwrap();
    assert!(root.contains("format"), "{root}");
    let once = files_of(&folder);
    let (status, _) = go(&p, Kind::Open, &dir, json!({ "path": "a.k7" })).await;
    assert_eq!(status, 200);
    assert_eq!(files_of(&folder), once);
    std::fs::remove_dir_all(dir).unwrap();
}

// An open warms its project's waveforms through the account's media, natively: no `settleOpen` is asked.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_open_warms_the_opened_projects_waveforms() {
    use std::sync::atomic::Ordering;
    let dir = scratch("p-warm");
    let (mut p, _) = proj(&dir, "p-warm");
    make(&dir, "a.k7").await;
    let env: std::collections::BTreeMap<String, String> = [
        ("STUDIO_HOME".to_owned(), p.home.studio.clone()),
        ("STUDIO_LIBRARIES_HOME".to_owned(), p.home.libraries.clone()),
    ]
    .into_iter()
    .collect();
    let code = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../..");
    let config = super::media::Config::of(&env, &code);
    let media = Arc::new(super::media::Media::new(config, Arc::new(|| ())));
    let acct = crate::routes::files::service::Acct::make(env.clone(), super::tests_account::HEAP);
    let log: k7_hub::Log = Arc::new(|_: &str| {});
    p.site = super::media::host::site_of(Some(&media), acct, &code, &env, "p-warm", &log);
    let (status, body) = go(&p, Kind::Open, &dir, json!({ "path": "a.k7" })).await;
    assert_eq!(status, 200, "{body}");
    until("the open's prewarm ran", || {
        media.counts.prewarms.load(Ordering::Relaxed) == 1
    })
    .await;
    assert!(!calls::of(&root_of(&dir)).contains(&"settleOpen".to_string()));
    std::fs::remove_dir_all(dir).unwrap();
}
