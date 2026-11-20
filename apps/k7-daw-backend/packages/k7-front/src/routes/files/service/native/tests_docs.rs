//! The nine compile routes answered natively (1110 to 1121): a decline is refused with a
//! sentence, and the shared session is what answered.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use crate::routes::files::service::Acct;
use serde_json::{Value, json};

use super::answering::Gate;
use super::docs::{Docs, catalogue};
use super::tests::{Lines, ctx, scratch};
use super::tests_account::{call, start, text};
use super::{Ctx, Tag};
use crate::routes::files::service::routes::{self, H};

pub(super) const ROUTES: [(H, Tag); 9] = [
    (H::ProjectDoc, Tag::ProjectDoc),
    (H::EngineDoc, Tag::EngineDoc),
    (H::ProjectErrors, Tag::ProjectErrors),
    (H::Graph, Tag::Graph),
    (H::Hints, Tag::Hints),
    (H::Surfaces, Tag::Surfaces),
    (H::IoPorts, Tag::IoPorts),
    (H::ModuleCatalogue, Tag::Catalogue),
    (H::BaseModules, Tag::BaseModules),
];

pub(super) fn code_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../..")
}

pub(super) fn env_of(dir: &Path) -> BTreeMap<String, String> {
    BTreeMap::from([
        (
            "STUDIO_HOME".to_string(),
            crate::routes::files::service::native::tests::slashed(&dir.join("home")),
        ),
        (
            "STUDIO_LIBRARIES_HOME".to_string(),
            dir.join("libraries").display().to_string(),
        ),
    ])
}

/// A home whose workspace holds one project, an account, and the project open.
pub(super) async fn world(name: &str) -> (PathBuf, Arc<Acct>) {
    let dir = scratch(name);
    let ws = dir.join("home/workspace/songs");
    let proj = ws.join("p.k7");
    std::fs::create_dir_all(proj.join("nodes")).unwrap();
    std::fs::write(proj.join("nodes/gtr.k7x"), "extends(\"@meta/track\")\n").unwrap();
    std::fs::write(proj.join("graph.k7x"), "modules({ gtr: [\"MIX\"] })\n").unwrap();
    std::fs::write(proj.join("project.k7x"), "project({ tempo: 120 })\n").unwrap();
    std::fs::create_dir_all(dir.join("home/workspace/.active-projects")).unwrap();
    std::fs::write(dir.join("home/workspace/.active-projects/songs"), "p.k7").unwrap();
    let acct = start(&dir);
    (dir, acct)
}

/// A context that answers the docs route `tag`.
pub(super) fn answering(acct: &Arc<Acct>, dir: &Path, tag: Tag, lines: &Lines) -> Ctx {
    let mut c = ctx(dir, tag, "", lines);
    c.docs = Some(Docs::of(acct, &code_root()));
    c
}

pub(super) async fn route(dir: &Path, h: H, query: &str, native: Option<Ctx>) -> (u16, String) {
    let call = call(dir, query, json!({}), native);
    text(super::refuse::scoped(routes::dispatch(h, &call)).await).await
}

pub(super) fn value(s: &str) -> Value {
    serde_json::from_str(s).unwrap_or_else(|e| panic!("not JSON ({e}): {}", &s[..s.len().min(200)]))
}

/// What docs declined: its own counter, which the reads' `declined` never holds.
pub(super) fn declined() -> u64 {
    super::stats::global()
        .docs_declined
        .load(std::sync::atomic::Ordering::Relaxed)
}

fn session_holds(acct: &Arc<Acct>, dir: &Path) -> usize {
    let ws = dir.join("home/workspace/songs").display().to_string();
    let key = k7_file_service::songs::key_of(&ws, song::PathRule::HOST).unwrap();
    acct.songs()
        .with(&key, song::PathRule::HOST, |s| s.held())
        .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_route_answers_from_the_shared_session() {
    let (dir, acct) = world("docs-shared").await;
    let lines = Lines::default();
    assert_eq!(session_holds(&acct, &dir), 0);
    let c = answering(&acct, &dir, Tag::ProjectDoc, &lines);
    let (status, body) = route(&dir, H::ProjectDoc, "", Some(c)).await;
    assert_eq!(status, 200, "{body}");
    assert!(value(&body).is_object());
    assert!(
        session_holds(&acct, &dir) >= 1,
        "the front compiled in the shared session"
    );
    assert_eq!(declined(), 0, "{:?}", lines.lock().unwrap());
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_nine_routes_are_answered_natively() {
    let (dir, acct) = world("docs-nine").await;
    let lines = Lines::default();
    for (h, tag) in ROUTES {
        let c = answering(&acct, &dir, tag, &lines);
        let ours = route(&dir, h, "", Some(c)).await;
        assert_eq!(ours.0, 200, "{h:?}: {}", ours.1);
        value(&ours.1);
    }
    assert_eq!(declined(), 0, "{:?}", lines.lock().unwrap());
    assert_eq!(
        super::stats::global().docs_answered.load(Ordering::Relaxed),
        9
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_docs_decline_is_counted_apart_from_the_reads() {
    let (dir, acct) = world("docs-apart").await;
    let lines = Lines::default();
    std::fs::write(
        dir.join("home/workspace/.active-projects/songs"),
        "_libraries/x/p.k7",
    )
    .unwrap();
    let c = answering(&acct, &dir, Tag::ProjectDoc, &lines);
    let ours = route(&dir, H::ProjectDoc, "", Some(c)).await;
    assert_eq!(ours.0, 409, "{}", ours.1);
    assert_eq!(declined(), 1, "{:?}", lines.lock().unwrap());
    assert_eq!(super::stats::global().declined.load(Ordering::Relaxed), 0);
    assert_eq!(
        super::stats::global().docs_answered.load(Ordering::Relaxed),
        0
    );
    let line = super::stats::global().line();
    assert!(
        line.contains(" declined=0 docsAnswered=0 docsDeclined=1 "),
        "{line}"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_engine_doc_is_the_docs_slice_of_the_engine_view() {
    let (dir, acct) = world("docs-engine").await;
    let lines = Lines::default();
    let c = answering(&acct, &dir, Tag::EngineDoc, &lines);
    let (status, body) = route(&dir, H::EngineDoc, "", Some(c)).await;
    assert_eq!(status, 200);
    let doc = value(&body);
    assert!(
        doc.get("freezes").is_none() && doc.get("doc").is_none(),
        "{body}"
    );
    assert!(doc.get("tracks").is_some() || doc.get("tempo").is_some() || doc.is_object());
    assert_eq!(declined(), 0);
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn what_the_native_side_cannot_prove_is_refused_with_a_sentence() {
    let (dir, acct) = world("docs-declines").await;
    let lines = Lines::default();
    // A graph of another folder is the song session's own (Folder::Other), not a decline: numbers are compared by value.
    let c = answering(&acct, &dir, Tag::Graph, &lines);
    let at = route(&dir, H::Graph, "at=p.k7", Some(c)).await;
    assert_eq!(at.0, 200, "a graph of another folder: {}", at.1);
    assert_eq!(declined(), 0);
    // A library as the open project: the roots cannot be made, the ask is refused with a sentence (4-7), the decline said once.
    std::fs::write(
        dir.join("home/workspace/.active-projects/songs"),
        "_libraries/x/p.k7",
    )
    .unwrap();
    for asked in 1..=2u64 {
        let c = answering(&acct, &dir, Tag::Hints, &lines);
        let ours = route(&dir, H::Hints, "", Some(c)).await;
        assert_eq!(ours.0, 409, "ask {asked}: {}", ours.1);
        assert_eq!(value(&ours.1)["declined"], "library-active");
        assert_eq!(declined(), asked, "ask {asked}");
    }
    let legacy = |l: &String| l.contains("library-active");
    let said = lines.lock().unwrap().iter().filter(|l| legacy(l)).count();
    assert_eq!(said, 1, "{:?}", lines.lock().unwrap());
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_project_that_does_not_compile_is_a_400_with_the_compiler_s_words() {
    let (dir, acct) = world("docs-failure").await;
    let lines = Lines::default();
    std::fs::write(
        dir.join("home/workspace/songs/p.k7/project.k7x"),
        "project({ tempo: \n",
    )
    .unwrap();
    for (h, tag) in [
        (H::ProjectDoc, Tag::ProjectDoc),
        (H::EngineDoc, Tag::EngineDoc),
    ] {
        let c = answering(&acct, &dir, tag, &lines);
        let ours = route(&dir, h, "", Some(c)).await;
        assert_eq!(ours.0, 400, "{h:?}: {}", ours.1);
        assert!(value(&ours.1)["error"].is_string(), "{h:?}: {}", ours.1);
    }
    assert_eq!(declined(), 0, "{:?}", lines.lock().unwrap());
    assert_eq!(
        super::stats::global().docs_answered.load(Ordering::Relaxed),
        2
    );
    super::tests::remove_tree(&dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_workspace_with_no_active_project_is_answered_not_refused() {
    let (dir, acct) = world("docs-idle").await;
    let lines = Lines::default();
    let other = dir.join("home/workspace/idle");
    std::fs::create_dir_all(&other).unwrap();
    let mut c = answering(&acct, &dir, Tag::ProjectDoc, &lines);
    c.root = other.display().to_string();
    let mut req = call(&dir, "", json!({}), Some(c));
    req.root = other.display().to_string();
    let ours = text(routes::dispatch(H::ProjectDoc, &req).await).await;
    assert!(value(&ours.1).get("declined").is_none(), "{ours:?}");
    assert_eq!(declined(), 0);
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn no_weight_to_be_had_is_a_busy_refusal() {
    let (dir, acct) = world("docs-busy").await;
    let lines = Lines::default();
    let none: &'static Gate =
        Box::leak(Box::new(Gate::new(0, 0).waiting(Duration::from_millis(50))));
    let mut c = answering(&acct, &dir, Tag::ProjectDoc, &lines);
    c.gate = none;
    let ours = route(&dir, H::ProjectDoc, "", Some(c)).await;
    assert_eq!(ours.0, 503, "{}", ours.1);
    assert_eq!(value(&ours.1)["declined"], "busy");
    // A busy decline is a shed (1780a), not a decline.
    assert_eq!(declined(), 0);
    assert_eq!(
        super::stats::global()
            .shed
            .load(std::sync::atomic::Ordering::Relaxed),
        1
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_session_that_panicked_is_made_again_by_the_next_ask() {
    let (dir, acct) = world("docs-panic").await;
    let lines = Lines::default();
    let ws = dir.join("home/workspace/songs").display().to_string();
    let key = k7_file_service::songs::key_of(&ws, song::PathRule::HOST).unwrap();
    let c = answering(&acct, &dir, Tag::ProjectDoc, &lines);
    let first = route(&dir, H::ProjectDoc, "", Some(c)).await;
    let gone = acct.songs().with(&key, song::PathRule::HOST, |_| {
        panic!("a session that dies")
    });
    assert!(gone.is_err());
    let c = answering(&acct, &dir, Tag::ProjectDoc, &lines);
    assert_eq!(route(&dir, H::ProjectDoc, "", Some(c)).await, first);
    assert_eq!(declined(), 0);
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_catalogue_s_paths_are_the_workspace_s() {
    let dir = scratch("docs-catalogue");
    let lines = Lines::default();
    let c = ctx(&dir, Tag::Catalogue, "", &lines);
    let inside = format!("{}/sub/a.k7x", c.root);
    let rows = json!([
        { "name": "a", "path": inside, "kind": "x" },
        { "name": "engine", "path": "" },
        { "name": "null", "path": null },
        { "name": "bare" }
    ]);
    let mapped = value(&catalogue(&c, &rows.to_string()).unwrap());
    assert_eq!(
        mapped[0],
        json!({ "name": "a", "path": "sub/a.k7x", "kind": "x" })
    );
    assert_eq!(mapped[1], json!({ "name": "engine" }));
    assert_eq!(mapped[2], json!({ "name": "null" }));
    assert_eq!(mapped[3], json!({ "name": "bare" }));
    assert!(catalogue(&c, r#"[{"path":5}]"#).is_none());
    assert!(catalogue(&c, r#"{"not":"a list"}"#).is_none());
    std::fs::remove_dir_all(dir).unwrap();
}

/// A module whose free-form plugin value nests to the k7x parser's limit, and a node that extends it.
fn deep_module(dir: &Path, depth: usize) {
    let ws = dir.join("home/workspace/songs");
    let module = ws.join("_modules/audio/deep");
    std::fs::create_dir_all(&module).unwrap();
    let nest = format!("{}1{}", "[".repeat(depth), "]".repeat(depth));
    let text = format!("plugin({{ type: \"internal\", name: \"reverb\", extra: {nest} }})\n");
    std::fs::write(module.join("module.k7x"), text).unwrap();
    let proj = ws.join("p.k7");
    std::fs::write(
        proj.join("nodes/fx.k7x"),
        "extends(\"@workspace/audio/deep\")\n",
    )
    .unwrap();
    std::fs::write(
        proj.join("graph.k7x"),
        "modules({ gtr: [\"fx\"], fx: [\"MIX\"] })\n",
    )
    .unwrap();
}

async fn compiled_deep(name: &str, depth: usize) {
    let (dir, acct) = world(name).await;
    let lines = Lines::default();
    deep_module(&dir, depth);
    let started = std::time::Instant::now();
    for (h, tag) in ROUTES {
        let c = answering(&acct, &dir, tag, &lines);
        let ours = route(&dir, h, "", Some(c)).await;
        assert!(
            value(&ours.1).get("declined").is_none(),
            "{h:?}: {}",
            ours.1
        );
    }
    assert!(
        started.elapsed() < Duration::from_secs(if cfg!(target_env = "musl") { 30 } else { 5 })
    );
    assert_eq!(declined(), 0, "{:?}", lines.lock().unwrap());
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_deepest_plugin_value_compiles_through_the_nine_routes() {
    compiled_deep("docs-deep", 254).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_compile_that_panics_is_counted_said_and_refused() {
    let (dir, acct) = world("docs-compile-panic").await;
    let lines = Lines::default();
    let mut c = answering(&acct, &dir, Tag::Hints, &lines);
    c.chaos = super::Chaos::Panic;
    let ours = route(&dir, H::Hints, "", Some(c)).await;
    assert_eq!(ours.0, 500, "{}", ours.1);
    assert_eq!(value(&ours.1)["declined"], "panic");
    assert_eq!(
        super::stats::global()
            .panics
            .load(std::sync::atomic::Ordering::Relaxed),
        1
    );
    assert_eq!(declined(), 1);
    let said = lines.lock().unwrap().clone();
    assert!(
        said.iter().any(|l| l.contains("GET /hints panicked")),
        "{said:?}"
    );
    std::fs::remove_dir_all(dir).unwrap();
}
