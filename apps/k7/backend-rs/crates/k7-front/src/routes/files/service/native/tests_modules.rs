//! The library move answered natively: the folder, the refs, the engine told twice and asked nothing, the journal's gate waited for.

use std::time::Duration;

use serde_json::{Value, json};

use super::Tag;
use super::entries::calls;
use super::tests::{Lines, scratch};
use super::tests_account::text;
use super::tests_entries::request;
use crate::routes::files::service::routes::{self, H};

/// A workspace with a library module, a song using it and a second folder to move it to.
fn fixture(name: &str) -> std::path::PathBuf {
    let dir = scratch(name);
    let lib = dir.join("libraries/k7-core/modules/audio");
    std::fs::create_dir_all(lib.join("verb")).unwrap();
    std::fs::create_dir_all(lib.join("fx/old")).unwrap();
    std::fs::write(lib.join("verb/module.k7x"), "extends(\"@engine/reverb\")\n").unwrap();
    std::fs::write(
        lib.join("fx/old/module.k7x"),
        "extends(\"@engine/reverb\")\n",
    )
    .unwrap();
    let songs = dir.join("home/workspace/songs");
    std::fs::create_dir_all(songs.join("song.k7/nodes")).unwrap();
    std::fs::write(
        songs.join("song.k7/nodes/v.k7x"),
        "extends(\"@k7-core/audio/verb\")\n",
    )
    .unwrap();
    dir
}

const MODULE: &str = "_libraries/k7-core/modules/audio/verb";
const FX: &str = "_libraries/k7-core/modules/audio/fx";

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_library_module_moved_repoints_its_song_and_asks_the_bundle_nothing() {
    let dir = fixture("m-move");
    let lines = Lines::default();
    let go = |body: Value| {
        let c = request(&dir, Tag::LibraryMove, body, true, &lines);
        async move { text(routes::dispatch(H::LibraryMove, &c).await).await }
    };
    let (status, body) = go(json!({ "path": MODULE, "toDir": FX })).await;
    assert_eq!(status, 200, "{body}");
    let v: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["path"], format!("{FX}/verb"));
    assert_eq!(v["to"], "@k7-core/audio/fx/verb");
    assert_eq!(v["repointed"], json!(["song.k7/nodes/v.k7x"]));
    let songs = dir.join("home/workspace/songs");
    assert_eq!(
        std::fs::read_to_string(songs.join("song.k7/nodes/v.k7x")).unwrap(),
        "extends(\"@k7-core/audio/fx/verb\")\n"
    );
    assert_eq!(calls::of(&songs.display().to_string()), ["before", "after"]);
    // Another library is refused in the route's words, with nothing told.
    let (status, body) =
        go(json!({ "path": MODULE, "toDir": "_libraries/other/modules/audio" })).await;
    assert_eq!(status, 400, "{body}");
    assert!(body.contains("a module stays in its library"), "{body}");
    assert_eq!(calls::of(&songs.display().to_string()).len(), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_library_move_waits_for_the_journals_gate() {
    let dir = fixture("m-gate");
    let lines = Lines::default();
    let songs = dir.join("home/workspace/songs").display().to_string();
    let held = k7_file_service::journal::Journals::global()
        .exclusive(&k7_file_service::paths::resolve_all(&[&songs]))
        .await;
    let c = request(
        &dir,
        Tag::LibraryMove,
        json!({ "path": MODULE, "toDir": FX }),
        true,
        &lines,
    );
    let task = tokio::spawn(async move { text(routes::dispatch(H::LibraryMove, &c).await).await });
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!task.is_finished(), "the move ran inside a held gate");
    assert!(dir.join("libraries/k7-core/modules/audio/verb").is_dir());
    drop(held);
    assert_eq!(task.await.unwrap().0, 200);
}

/// A `/module` call whose context carries the code root, as a docs-bearing route's does.
fn module_call(dir: &std::path::Path, tag: Tag, body: Value, shared: &Lines) -> routes::Call {
    let mut c = request(dir, tag, body, false, shared);
    let code = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../..");
    if let Some(n) = c.native.as_mut() {
        n.docs = Some(super::docs::Docs::of_env(&code, &Default::default()));
    }
    c
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_module_made_natively_asks_the_bundle_nothing_and_refuses_in_the_routes_words() {
    let dir = fixture("m-make");
    let lines = Lines::default();
    let songs = dir.join("home/workspace/songs");
    std::fs::create_dir_all(songs.join("_modules")).unwrap();
    let go = |body: Value| {
        let c = module_call(&dir, Tag::ModuleCreate, body, &lines);
        async move { text(routes::dispatch(H::Module, &c).await).await }
    };
    let (status, body) = go(json!({ "root": "_modules", "rel": "audio/verb" })).await;
    assert_eq!(
        (status, body.as_str()),
        (
            200,
            r#"{"path":"_modules/audio/verb/module.k7x","dir":"_modules/audio/verb"}"#
        )
    );
    let text = std::fs::read_to_string(songs.join("_modules/audio/verb/module.k7x")).unwrap();
    assert!(text.contains("verb") && text.contains("plugin"), "{text}");
    refused_with(
        go(json!({ "root": "_modules", "rel": "audio/verb" })),
        "already exists",
    )
    .await;
    refused_with(
        go(json!({ "root": "_modules", "rel": "x" })),
        "names no type folder",
    )
    .await;
    refused_with(
        go(json!({ "root": "_modules", "rel": "../x" })),
        "not something a folder can be called",
    )
    .await;
    refused_with(
        go(json!({ "root": "song.k7", "rel": "audio/x" })),
        "is not a module library of this workspace",
    )
    .await;
    assert_eq!(calls::of(&songs.display().to_string()), ["after"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_module_made_waits_for_the_journals_gate() {
    let dir = fixture("m-make-gate");
    let lines = Lines::default();
    let songs = dir.join("home/workspace/songs");
    std::fs::create_dir_all(songs.join("_modules")).unwrap();
    let held = k7_file_service::journal::Journals::global()
        .exclusive(&k7_file_service::paths::resolve_all(&[&songs
            .display()
            .to_string()]))
        .await;
    let c = module_call(
        &dir,
        Tag::ModuleCreate,
        json!({ "root": "_modules", "rel": "audio/v" }),
        &lines,
    );
    let task = tokio::spawn(async move { text(routes::dispatch(H::Module, &c).await).await });
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!task.is_finished());
    assert!(!songs.join("_modules/audio").exists());
    drop(held);
    assert_eq!(task.await.unwrap().0, 200);
}

async fn refused_with(call: impl std::future::Future<Output = (u16, String)>, want: &str) {
    let (s, body) = call.await;
    assert_eq!(s, 400, "{body}");
    assert!(body.contains(want), "{body}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_node_and_a_clone_are_made_natively_in_the_open_project_and_a_node_waits_for_the_gate() {
    let dir = fixture("m-node");
    let songs = dir.join("home/workspace/songs");
    std::fs::write(
        songs.join("song.k7/project.k7x"),
        "project({ name: \"song\" })\n",
    )
    .unwrap();
    let module = songs.join("_modules/audio/verb");
    std::fs::create_dir_all(&module).unwrap();
    std::fs::write(module.join("module.k7x"), "extends(\"@engine/reverb\")\n").unwrap();
    let marker = dir.join("home/workspace/.active-projects");
    std::fs::create_dir_all(&marker).unwrap();
    std::fs::write(marker.join("songs"), "song.k7").unwrap();
    let lines = Lines::default();
    let call = |tag: Tag, h: H, body: Value| {
        let c = module_call(&dir, tag, body, &lines);
        async move { text(routes::dispatch(h, &c).await).await }
    };
    let node = call(
        Tag::NodeCreate,
        H::Node,
        json!({ "path": "_modules/audio/verb" }),
    )
    .await;
    assert_eq!(node, (200, r#"{"path":"nodes/verb_1.k7x"}"#.to_string()));
    let clone = call(
        Tag::ModuleClone,
        H::ModuleClone,
        json!({ "path": "_modules/audio/verb", "newName": "warm" }),
    )
    .await;
    assert_eq!(
        clone,
        (
            200,
            r#"{"path":"_modules/audio/warm/module.k7x"}"#.to_string()
        )
    );
    assert!(songs.join("song.k7/nodes/verb_1.k7x").is_file());
    assert!(songs.join("_modules/audio/warm/module.k7x").is_file());
    assert_eq!(calls::of(&songs.display().to_string()), ["after", "after"]);
    let held = k7_file_service::journal::Journals::global()
        .exclusive(&k7_file_service::paths::resolve_all(&[&songs
            .display()
            .to_string()]))
        .await;
    let waiting = call(
        Tag::NodeCreate,
        H::Node,
        json!({ "path": "_modules/audio/verb", "node": "lead" }),
    );
    let task = tokio::spawn(waiting);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!task.is_finished());
    assert!(!songs.join("song.k7/nodes/lead.k7x").exists());
    drop(held);
    assert_eq!(task.await.unwrap().0, 200);
}

// An admin's move inside a versioned library's draft lands and repoints; one into a published version is refused.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_admin_moves_a_module_within_a_versioned_librarys_draft() {
    let dir = fixture("m-draft");
    let lib = dir.join("libraries/k7-core");
    std::fs::create_dir_all(lib.join("draft")).unwrap();
    std::fs::rename(lib.join("modules"), lib.join("draft/modules")).unwrap();
    std::fs::create_dir_all(lib.join("1/modules/audio/fx/old")).unwrap();
    std::fs::write(
        lib.join("1/modules/audio/fx/old/module.k7x"),
        "extends(\"@engine/reverb\")\n",
    )
    .unwrap();
    let lines = Lines::default();
    let go = |body: Value| {
        let c = request(&dir, Tag::LibraryMove, body, true, &lines);
        async move { text(routes::dispatch(H::LibraryMove, &c).await).await }
    };
    let (status, body) =
        go(json!({ "path": MODULE, "toDir": "_libraries/k7-core/1/modules/audio/fx" })).await;
    assert_eq!(status, 400, "{body}");
    assert!(lib.join("draft/modules/audio/verb").is_dir(), "{body}");
    let (status, body) = go(json!({ "path": MODULE, "toDir": FX })).await;
    assert_eq!(status, 200, "{body}");
    assert!(lib.join("draft/modules/audio/fx/verb/module.k7x").is_file());
    assert!(!lib.join("draft/modules/audio/verb").exists());
    let v: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["repointed"], json!(["song.k7/nodes/v.k7x"]));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_nodes_presets_are_listed_natively_from_the_open_project() {
    let dir = fixture("m-presets");
    let songs = dir.join("home/workspace/songs");
    std::fs::write(
        songs.join("song.k7/project.k7x"),
        "project({ name: \"song\" })\n",
    )
    .unwrap();
    let shelf = songs.join("_presets/k7-core/audio/verb");
    std::fs::create_dir_all(&shelf).unwrap();
    std::fs::write(shelf.join("Hall.k7x"), "params({ gain: 1 })\n").unwrap();
    let marker = dir.join("home/workspace/.active-projects");
    std::fs::create_dir_all(&marker).unwrap();
    std::fs::write(marker.join("songs"), "song.k7").unwrap();
    let lines = Lines::default();
    let go = |query: &str| {
        let mut c = module_call(&dir, Tag::Presets, json!({}), &lines);
        c.query = query.into();
        async move { text(routes::dispatch(H::Presets, &c).await).await }
    };
    let (status, body) = go("node=v").await;
    assert_eq!(status, 200, "{body}");
    let v: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["ref"], "@k7-core/audio/verb");
    assert_eq!(
        v["presets"],
        json!([{ "scope": "workspace", "name": "Hall" }])
    );
    let (status, body) = go("node=ghost").await;
    assert_eq!(
        (status, body.as_str()),
        (400, r#"{"error":"no node named 'ghost'"}"#)
    );
    assert_eq!(go("").await.0, 400);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn presets_with_no_open_project_say_so_as_the_bundle_does() {
    let dir = fixture("m-presets-none");
    let lines = Lines::default();
    let mut c = module_call(&dir, Tag::Presets, json!({}), &lines);
    c.query = "node=v".into();
    let got = text(routes::dispatch(H::Presets, &c).await).await;
    assert_eq!(got, (400, r#"{"error":"no active project"}"#.to_string()));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_project_is_created_natively_in_the_routes_words_and_waits_for_the_gate() {
    let dir = fixture("m-project");
    let lines = Lines::default();
    let songs = dir.join("home/workspace/songs");
    let go = |body: Value| {
        let c = module_call(&dir, Tag::ProjectCreate, body, &lines);
        async move { text(routes::dispatch(H::ProjectCreate, &c).await).await }
    };
    let (status, body) = go(json!({ "shape": "live", "name": "rig" })).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body, r#"{"project":"rig.k7","missing":[]}"#);
    assert!(songs.join("rig.k7/chains/IN.k7x").is_file());
    assert!(!songs.join("rig.k7/graph.k7x").exists());
    assert_eq!(calls::of(&songs.display().to_string()), ["after"]);
    refused_with(
        go(json!({ "shape": "live", "name": "rig.k7" })),
        "already exists and is not empty",
    )
    .await;
    refused_with(
        go(json!({ "shape": "live", "name": "..." })),
        "project name required",
    )
    .await;
    assert_eq!(go(json!({ "shape": "song", "name": "x" })).await.0, 400);
    let held = k7_file_service::journal::Journals::global()
        .exclusive(&k7_file_service::paths::resolve_all(&[&songs
            .display()
            .to_string()]))
        .await;
    let waiting = go(json!({ "shape": "live", "name": "late" }));
    let task = tokio::spawn(waiting);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!task.is_finished());
    assert!(!songs.join("late.k7").exists());
    drop(held);
    assert_eq!(task.await.unwrap().0, 200);
}
