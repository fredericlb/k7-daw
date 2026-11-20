//! The orphan reads answered natively natively: the open project asked once, the TypeScript's sentences in a 400, `..` refused.

use serde_json::{Value, json};

use super::Tag;
use super::tests::{Lines, scratch};
use super::tests_account::text;
use super::tests_entries::request;
use crate::routes::files::service::routes::{self, H};

const MODULE: &str = "_libraries/k7-core/modules/audio/verb";

/// A workspace with an open project, a library module holding a panel, bases and engines, and a console system.
fn fixture(name: &str) -> std::path::PathBuf {
    let dir = scratch(name);
    let lib = dir.join("libraries/k7-core/modules");
    let w = |p: std::path::PathBuf, t: &str| {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, t).unwrap();
    };
    let reverb = "extends(\"@engine/reverb\")\n";
    w(lib.join("audio/verb/module.k7x"), reverb);
    w(
        lib.join("audio/verb/ui.k7x"),
        "cols(4)\nrows(2)\nwidgets({})\n",
    );
    w(lib.join("audio/verb/base/warm.vstpreset"), "x");
    w(lib.join("audio/verb/engines/tracktion/module.k7x"), reverb);
    w(lib.join("audio/verb/engines/juce/module.k7x"), reverb);
    w(
        lib.join("console/console-6/channel/module.k7x"),
        "plugin({ type: \"internal\", name: \"C\" })\n",
    );
    w(
        lib.join("console/console-6/buss/module.k7x"),
        "plugin({ type: \"internal\", name: \"B\" })\n",
    );
    let songs = dir.join("home/workspace/songs");
    w(
        songs.join("song.k7/project.k7x"),
        "project({ name: \"song\", console: \"@k7-core/console/console-6\" })\n",
    );
    w(songs.join("song.k7/modules/audio/mine/module.k7x"), reverb);
    w(
        songs.join("_modules/instruments/lead/module.k7x"),
        "plugin({ type: \"VST3\", name: \"Serum\" })\n",
    );
    w(
        songs.join("song.k7/nodes/lead.k7x"),
        "extends(\"@workspace/instruments/lead\")\n",
    );
    w(
        songs.join("song.k7/graph.k7x"),
        "modules({ lead: [\"OUT\"] })\n",
    );
    w(
        songs.join("song.k7/clips/voice.k7x"),
        "source(\"voice.wav\")\n",
    );
    w(songs.join("song.k7/voice.wav"), "RIFFxxxx");
    w(dir.join("home/workspace/.active-projects/songs"), "song.k7");
    dir
}

/// One GET of `h`, its context carrying the code root as a docs-bearing route's does.
async fn get(dir: &std::path::Path, h: H, query: &str) -> (u16, Value) {
    let lines = Lines::default();
    let tag = Tag::of(h).unwrap();
    let mut c = request(dir, tag, json!({}), &lines);
    let code = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../..");
    if let Some(n) = c.native.as_mut() {
        n.docs = Some(super::docs::Docs::of_env(&code, &Default::default()));
    }
    c.query = query.into();
    let (status, body) = text(routes::dispatch(h, &c).await).await;
    (
        status,
        serde_json::from_str(&body).unwrap_or(Value::String(body)),
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_module_reads_answer_natively_and_refuse_in_the_typescripts_words() {
    let dir = fixture("o-modules");
    let path = |p: &str| format!("path={p}");
    let (s, v) = get(&dir, H::ModuleUi, &path(&format!("{MODULE}/module.k7x"))).await;
    assert_eq!(
        (s, v["ref"].as_str(), v["exists"].clone()),
        (200, Some("@k7-core/audio/verb"), json!(true))
    );
    let (s, v) = get(
        &dir,
        H::ModuleUi,
        &path("song.k7/modules/audio/mine/module.k7x"),
    )
    .await;
    assert_eq!((s, v["exists"].clone()), (200, json!(false)));
    assert!(
        v["content"]
            .as_str()
            .unwrap()
            .starts_with("// panel for audio/mine\n"),
        "{v}"
    );
    let (s, v) = get(&dir, H::ModuleUi, &path("../x/module.k7x")).await;
    assert_eq!(
        (s, v["error"].as_str()),
        (400, Some("path escapes the workspace: ../x/module.k7x"))
    );
    let (s, v) = get(&dir, H::ModuleUi, &path("song.k7/project.k7x")).await;
    assert_eq!(
        (s, v["error"].as_str()),
        (400, Some("song.k7/project.k7x is not a module file"))
    );
    let (s, v) = get(&dir, H::ModuleBase, &path(&format!("{MODULE}/module.k7x"))).await;
    assert_eq!((s, v), (200, json!({ "bases": ["warm"] })));
    let (s, v) = get(&dir, H::ModuleBase, "").await;
    assert_eq!((s, v["error"].as_str()), (400, Some("path is required")));
    let (s, v) = get(
        &dir,
        H::ModuleEngines,
        &path(&format!("{MODULE}/module.k7x")),
    )
    .await;
    assert_eq!(
        (s, v),
        (
            200,
            json!({ "engines": ["tracktion", "juce"], "engine": "tracktion" })
        )
    );
    let (s, v) = get(&dir, H::Consoles, "").await;
    assert_eq!(s, 200);
    assert_eq!(v["current"], "@k7-core/console/console-6");
    assert_eq!(
        v["systems"],
        json!([{ "ref": "@k7-core/console/console-6", "label": "console-6" }])
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_reads_that_need_a_project_say_so_when_none_is_open() {
    let dir = fixture("o-none");
    std::fs::remove_file(dir.join("home/workspace/.active-projects/songs")).unwrap();
    let want = Some("no active project — select a project in the workspace first");
    let (s, v) = get(&dir, H::Consoles, "").await;
    assert_eq!((s, v["error"].as_str()), (400, want));
    let (s, v) = get(&dir, H::ModuleUi, &format!("path={MODULE}/module.k7x")).await;
    assert_eq!((s, v["error"].as_str()), (400, want));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_project_reads_answer_natively() {
    let dir = fixture("o-project");
    std::fs::write(
        dir.join("home/exts.json"),
        r#"{"entries":[{"plugins":[{"name":"Other","format":"VST3"}]}]}"#,
    )
    .unwrap();
    let mut seen = Vec::new();
    for (h, target, status) in [
        (H::ProjectPlugins, "/project-plugins?project=song.k7", 200),
        (H::ProjectCompat, "/project-compat?project=song.k7", 200),
        (H::ProjectPlugins, "/project-plugins?project=ghost.k7", 404),
        (H::ProjectPlugins, "/project-plugins?project=..%2Fx.k7", 404),
        (H::ProjectPlugins, "/project-plugins", 400),
        (H::Resources, "/resources", 200),
    ] {
        let query = target.split_once('?').map_or("", |(_, q)| q);
        let got = get(&dir, h, query).await;
        seen.push((target, got.0, status));
    }
    let bad: Vec<_> = seen.iter().filter(|(_, g, s)| g != s).collect();
    assert!(bad.is_empty(), "{bad:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_graph_of_another_folder_is_answered_natively() {
    let dir = fixture("o-graph");
    let songs = dir.join("home/workspace/songs");
    std::fs::create_dir_all(songs.join("other.k7")).unwrap();
    std::fs::write(
        songs.join("other.k7/project.k7x"),
        "project({ name: \"other\" })\n",
    )
    .unwrap();
    std::fs::write(songs.join("other.k7/nodes.k7x"), "").ok();
    let mut seen = Vec::new();
    for (target, status) in [
        ("/graph?at=other.k7", 200),
        ("/graph?at=song.k7", 200),
        ("/graph?at=..%2Fx", 400),
        ("/graph?at=_libraries%2Fk7-core%2Fmodules", 200),
        ("/graph?at=nowhere.k7", 200),
        ("/graph", 200),
    ] {
        let query = target.split_once('?').map_or("", |(_, q)| q);
        let got = get(&dir, H::Graph, query).await;
        seen.push((target, got.0, status));
    }
    let bad: Vec<_> = seen.iter().filter(|(_, g, s)| g != s).collect();
    assert!(bad.is_empty(), "{bad:?}");
}
