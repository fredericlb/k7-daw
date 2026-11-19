//! Scenarios of the jobs' frame replays: the plugin queries, an audition, a thaw, an edit while thawed, and an event's take
//! armed and then filed. What a reader gets wrong: `route` calls the handler directly.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::{Value, json};

use super::tests_real::Account;
use super::tests_scenarios::silent_wav;
use super::tests_scene::{frames, is_frame, open, until_frame, verb, write_wait};
use crate::routes::files::service::native::Tag;
use crate::routes::files::service::native::docs::Docs;
use crate::routes::files::service::native::jobs::freeze_run::renders_of;
use crate::routes::files::service::native::jobs::routes::{Deps, act, plugin};

pub(super) fn code_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../..")
}

pub(super) fn env_of(a: &Account) -> BTreeMap<String, String> {
    [
        ("STUDIO_HOME", a.dir.join("home")),
        ("STUDIO_LIBRARIES_HOME", a.dir.join("libraries")),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v.display().to_string()))
    .collect()
}

/// The front's jobs held.
pub(super) fn hold(a: &Account) {
    let e = a.engine.as_ref().expect("the front's side");
    e.hold_jobs(&env_of(a), &code_root());
}

/// A route of the jobs through the front's handler; the body it answered.
pub(super) async fn route(a: &Account, method: &str, target: &str, body: Value) -> Value {
    let e = a.engine.as_ref().expect("the front's side");
    let env = env_of(a);
    let deps = Deps {
        engine: e,
        jobs: e.jobs.get().expect("held by `hold`"),
        home: k7_file_service::libraries::Home::of(&env),
        env: &env,
        code_root: code_root(),
    };
    let tag = match target.split('?').next().unwrap_or("") {
        "/plugin-params" => Tag::PluginParams,
        "/plugin-native-ui" => Tag::PluginNativeUi,
        "/module-base/snapshot" => Tag::ModuleBaseSnapshot,
        "/preview" => Tag::Preview,
        _ => Tag::AutoFreezeThaw,
    };
    let response = match target.split_once('?') {
        Some((_, query)) => plugin(&deps, tag, query).await,
        None if method == "GET" => plugin(&deps, tag, "").await,
        None => act(&deps, tag, &a.ws, &body).await.expect("no decline"),
    };
    let bytes = http_body_util::BodyExt::collect(response.into_body())
        .await
        .unwrap()
        .to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(Value::Null)
}

/// Each query answered, a window refused and a base written; one ask by `ref` over the scan copy.
pub(super) fn queries_scenario<'a>(
    a: &'a Account,
    _name: &'a str,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + 'a>> {
    Box::pin(async move {
        hold(a);
        let scan = json!({ "entries": [{ "plugins": [{ "name": "Café Delay", "format": "AU", "id": "au-1" }] }] });
        std::fs::write(a.dir.join("home/exts.json"), scan.to_string()).unwrap();
        a.fake
            .queries(|s| s.params = vec![json!({ "name": "mix", "index": 0 })]);
        route(a, "GET", "/plugin-params?format=VST3&id=u1", Value::Null).await;
        route(
            a,
            "GET",
            "/plugin-params?ref=%40engine%2Fcaf-delay",
            Value::Null,
        )
        .await;
        route(a, "GET", "/plugin-native-ui?format=VST3&id=u1", Value::Null).await;
        a.fake.queries(|s| {
            s.opened = false;
            s.found = false;
        });
        route(a, "GET", "/plugin-native-ui?format=VST3&id=u2", Value::Null).await;
        route(a, "GET", "/plugin-params?format=VST3&id=u2", Value::Null).await;
        let snap =
            json!({ "path": "p.k7/nodes/gtr.k7x", "name": "warm", "ref": "@engine/caf-delay" });
        route(a, "POST", "/module-base/snapshot", snap).await;
        let nowhere = json!({ "path": "p.k7/x.k7x", "name": "w", "ref": "@engine/none" });
        route(a, "POST", "/module-base/snapshot", nowhere).await;
    })
}

/// An audition of a file, of a slice and of a path outside, after an open.
pub(super) fn preview_scenario<'a>(
    a: &'a Account,
    name: &'a str,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + 'a>> {
    Box::pin(async move {
        hold(a);
        std::fs::create_dir_all(std::path::Path::new(&a.ws).join("p.k7/clips")).unwrap();
        std::fs::write(
            std::path::Path::new(&a.ws).join("p.k7/clips/kick.wav"),
            b"RIFF",
        )
        .unwrap();
        open(a, name, "p.k7").await;
        let seen = frames(a).len();
        route(
            a,
            "POST",
            "/preview",
            json!({ "kind": "source", "source": "clips/kick.wav" }),
        )
        .await;
        let slice = json!({ "kind": "slice", "source": "clips/kick.wav", "from": 0.5, "to": 2 });
        route(a, "POST", "/preview", slice).await;
        route(
            a,
            "POST",
            "/preview",
            json!({ "kind": "source", "source": "../../../x.wav" }),
        )
        .await;
        until_frame(a, seen, "the second audition", |f| {
            f["type"] == "preview" && f["start"] == json!(0.5)
        })
        .await;
    })
}

/// A freezable module thawed on and off in the open project: the project goes again each time, with the module live and then frozen.
pub(super) fn thaw_scenario<'a>(
    a: &'a Account,
    name: &'a str,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + 'a>> {
    Box::pin(async move {
        hold(a);
        freezable(a);
        open(a, name, "p.k7").await;
        let mut docs = vec![];
        for on in [true, false] {
            let seen = frames(a).len();
            route(
                a,
                "POST",
                "/auto-freeze/thaw",
                json!({ "module": MODULE, "on": on }),
            )
            .await;
            let want = |f: &Value| f["type"] == "project" && plays_module(f) == on;
            let at = until_frame(a, seen, "the reload's document", want).await;
            docs.push(frames(a)[at]["doc"].clone());
        }
        assert_ne!(docs[0], docs[1], "{name}: a thaw changes the document sent");
    })
}

/// The lane's event `a1` at 1:1 with a clip to arm a take on; both sides get the same files before the open.
fn event_files(a: &Account) {
    let project = std::path::Path::new(&a.ws).join("p.k7");
    silent_wav(&project.join("clips/a.wav"));
    std::fs::write(project.join("clips/a1.k7x"), "source(\"clips/a.wav\")\n").unwrap();
    let gtr = "extends(\"@meta/track\")\nevents([{ ref: \"a1\", start: \"1:1\" }])\n";
    std::fs::write(project.join("nodes/gtr.k7x"), gtr).unwrap();
}

/// The takes the event holds on disk, counted as its `takes` list names them.
fn takes_in_event(a: &Account) -> usize {
    let text = std::fs::read_to_string(std::path::Path::new(&a.ws).join("p.k7/nodes/gtr.k7x"))
        .unwrap_or_default();
    let list = text.split("takes: [").nth(1).unwrap_or_default();
    list.split(']')
        .next()
        .unwrap_or_default()
        .matches("ref(")
        .count()
}

/// Returns once no frame has come for 700 ms, so a document still on its way is in the comparison.
async fn settle(a: &Account) {
    let mut seen = frames(a).len();
    loop {
        tokio::time::sleep(std::time::Duration::from_millis(700)).await;
        let now = frames(a).len();
        if now == seen {
            return;
        }
        seen = now;
    }
}

/// An event armed for a take, a take reported, and the event holding it; `drop` runs the runtime's drop between the arm and the take.
async fn event_take(a: &Account, name: &str, drop: bool) {
    event_files(a);
    open(a, name, "p.k7").await;
    verb(a, "event take arm gtr 1:1 1:1 3:1").await;
    // The arm lives on the link, so a dropped runtime changes nothing: `drop` names the scenario only.
    let _ = drop;
    let wav = std::path::Path::new(&a.ws).join("p.k7/clips/take-tmp.wav");
    silent_wav(&wav);
    let seen = frames(a).len();
    a.fake.send_all(json!({
        "type": "recorded", "track": "gtr", "file": wav.display().to_string(), "start": 0, "length": 2,
    }));
    until_frame(a, seen, "the document with the take", is_frame("project")).await;
    for _ in 0..300 {
        if takes_in_event(a) == 1 {
            settle(a).await;
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    let text = std::fs::read_to_string(std::path::Path::new(&a.ws).join("p.k7/nodes/gtr.k7x"));
    panic!(
        "{name} {}: the take never reached its event: {text:?}",
        a.engine.is_some()
    );
}

/// A take armed on an event and recorded: it goes into the event, the project is sent with it.
pub(super) fn armed_take_scenario<'a>(
    a: &'a Account,
    name: &'a str,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + 'a>> {
    Box::pin(event_take(a, name, false))
}

/// The same after the front's runtime was dropped: the arm lives on the link, so the off side (no drop) is the referee (2311).
pub(super) fn dropped_take_scenario<'a>(
    a: &'a Account,
    name: &'a str,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + 'a>> {
    Box::pin(event_take(a, name, true))
}

/// The module the lane's chain runs through: the last (and only) `vst3` of its run, so it is the one frozen.
pub(super) const MODULE: &str = "a0";

/// The lane through a `vst3` module with its render cached, written before the open; the render's file is the one the compile names.
pub(super) fn freezable(a: &Account) {
    let songs = a.dir.join("home/workspace/songs");
    let module = songs.join("_modules/audio/amp");
    std::fs::create_dir_all(&module).unwrap();
    std::fs::write(
        module.join("module.k7x"),
        "plugin({ type: \"vst3\", name: \"Amp\" })\n",
    )
    .unwrap();
    let project = songs.join("p.k7");
    std::fs::write(
        project.join(format!("nodes/{MODULE}.k7x")),
        "extends(\"@workspace/audio/amp\")\n",
    )
    .unwrap();
    let graph = format!(
        "modules({{ gtr: [\"{MODULE}\"], {MODULE}: [\"@workspace/io/IN:in-1\"], \"@workspace/io/IN:in-1\": [\"MIX\"] }})\n"
    );
    std::fs::write(project.join("graph.k7x"), graph).unwrap();
    let docs = Docs::of(&a.acct, &code_root());
    let renders = renders_of(&docs, &project.display().to_string());
    assert_eq!(renders.len(), 1, "one freeze in the lane: {renders:?}");
    let render = std::path::Path::new(&renders[0]);
    std::fs::create_dir_all(render.parent().unwrap()).unwrap();
    silent_wav(render);
}

/// Whether a project frame carries the module live, as a thawed one does; a frozen one carries its render instead.
pub(super) fn plays_module(frame: &serde_json::Value) -> bool {
    frame["doc"]["tracks"][0]["plugins"]
        .as_array()
        .is_some_and(|p| !p.is_empty())
}

/// The module thawed, the project edited, the module let go: the edit's document goes out while it is live.
pub(super) fn edit_while_thawed_scenario<'a>(
    a: &'a Account,
    name: &'a str,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + 'a>> {
    Box::pin(async move {
        hold(a);
        freezable(a);
        open(a, name, "p.k7").await;
        let thaw = |on| json!({ "module": MODULE, "on": on });
        let seen = frames(a).len();
        route(a, "POST", "/auto-freeze/thaw", thaw(true)).await;
        until_frame(a, seen, "the module live", plays_module).await;
        let seen = frames(a).len();
        write_wait(a, "p.k7/project.k7x", "project({ tempo: 90 })\n").await;
        until_frame(a, seen, "the edit's document", |f| {
            f["type"] == "project" && f["doc"]["tempo"] == json!(90.0)
        })
        .await;
        route(a, "POST", "/auto-freeze/thaw", thaw(false)).await;
    })
}
