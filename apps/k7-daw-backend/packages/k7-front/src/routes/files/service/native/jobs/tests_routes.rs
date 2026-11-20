//! tests_routes.rs — the five routes' rows over a real account and a fake engine: hostile bodies, the plugin pair, confinement, the folder a snapshot makes, a thaw (2340 to 2345).
//! What a reader gets wrong: `Deps` stands for what `hook` builds from the request, so the request's own parse (413, bad JSON) and the hand-back are tested in `tests_hook.rs`.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::Ordering;

use axum::response::Response;
use http_body_util::BodyExt;
use k7_file_service::libraries::Home;
use serde_json::{Value, json};

use super::routes::{Deps, act, plugin};
use crate::routes::files::service::native::Tag;
use crate::routes::files::service::native::engine::fake::until;
use crate::routes::files::service::native::engine::tests_real::{Account, SESSION, account};

fn code_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../..")
}

/// The account's environment as `account` makes it, which a route reads its home from.
fn env_of(a: &Account) -> BTreeMap<String, String> {
    [
        ("STUDIO_HOME", a.dir.join("home")),
        ("STUDIO_LIBRARIES_HOME", a.dir.join("libraries")),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v.display().to_string()))
    .collect()
}

async fn read(r: Response) -> (u16, Value) {
    let status = r.status().as_u16();
    let bytes = r.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn error_of(v: &Value) -> &str {
    v["error"].as_str().unwrap_or_default()
}

struct Rig {
    a: Account,
    env: BTreeMap<String, String>,
}

impl Rig {
    async fn new(tag: &str) -> Rig {
        let a = account(tag).await;
        let env = env_of(&a);
        let engine = a.engine.clone().unwrap();
        engine.hold_jobs(&env, &code_root());
        Rig { a, env }
    }

    fn deps(&self) -> Deps<'_> {
        let engine = self.a.engine.as_ref().unwrap();
        Deps {
            engine,
            jobs: engine.jobs.get().unwrap(),
            home: Home::of(&self.env),
            env: &self.env,
            code_root: code_root(),
        }
    }

    async fn post(&self, tag: Tag, body: Value) -> (u16, Value) {
        match act(&self.deps(), tag, &self.a.ws, &body).await {
            Ok(r) => read(r).await,
            Err(why) => (0, json!(why)),
        }
    }

    async fn get(&self, tag: Tag, query: &str) -> (u16, Value) {
        read(plugin(&self.deps(), tag, query).await).await
    }

    async fn open(&self) {
        self.a.open("p.k7");
        self.a.start().await;
    }

    fn project(&self) -> String {
        format!("{}/p.k7", self.a.ws)
    }

    /// What the fake engine read on the workspace's session, of one kind.
    fn frames(&self, kind: &str) -> Vec<Value> {
        self.a
            .fake
            .of(SESSION)
            .into_iter()
            .filter(|f| f["type"] == kind)
            .collect()
    }
}

const NUMBERS: &str = "Invalid input: expected number, received undefined";

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_hostile_body_gets_the_sentence_zod_gives() {
    let r = Rig::new("jr-bodies").await;
    let said = |(status, v): (u16, Value)| (status, error_of(&v).to_owned());
    let kind = "Invalid discriminator value. Expected 'source' | 'clip' | 'slice'";
    assert_eq!(
        said(r.post(Tag::Preview, json!({})).await),
        (400, kind.into())
    );
    assert_eq!(
        said(r.post(Tag::Preview, json!({ "kind": 1 })).await),
        (400, kind.into())
    );
    assert_eq!(
        said(r.post(Tag::Preview, json!([])).await),
        (400, "Invalid input: expected object, received array".into())
    );
    let slice = json!({ "kind": "slice", "source": "a" });
    assert_eq!(
        said(r.post(Tag::Preview, slice).await),
        (400, format!("{NUMBERS}; {NUMBERS}"))
    );
    let thaw = json!({ "module": "a" });
    assert_eq!(
        said(r.post(Tag::AutoFreezeThaw, thaw).await),
        (
            400,
            "Invalid input: expected boolean, received undefined".into()
        )
    );
    let snap = json!({ "path": "a", "name": "b" });
    assert_eq!(
        said(r.post(Tag::ModuleBaseSnapshot, snap).await),
        (
            400,
            "Invalid input: expected string, received undefined".into()
        )
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_audition_with_no_open_project_is_refused_and_sends_nothing() {
    let r = Rig::new("jr-noproject").await;
    let (status, v) = r
        .post(Tag::Preview, json!({ "kind": "source", "source": "a.wav" }))
        .await;
    assert_eq!(
        (status, error_of(&v)),
        (
            400,
            "no active project — select a project in the workspace first"
        )
    );
    assert!(r.frames("preview").is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_audition_is_queued_before_its_answer_with_the_link_up() {
    let r = Rig::new("jr-queued").await;
    std::fs::create_dir_all(format!("{}/clips", r.project())).unwrap();
    std::fs::write(format!("{}/clips/kick.wav", r.project()), b"RIFF").unwrap();
    r.open().await;
    let session = r.a.engine.as_ref().unwrap().link.session(Some(&r.a.ws));
    let before = session.queued();
    let body = json!({ "kind": "source", "source": "clips/kick.wav" });
    assert_eq!(r.post(Tag::Preview, body).await.0, 200);
    assert_eq!(
        session.queued(),
        before + 1,
        "the frame is queued when the answer returns"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_audition_plays_a_file_of_the_project_after_its_document() {
    let r = Rig::new("jr-preview").await;
    std::fs::create_dir_all(format!("{}/clips", r.project())).unwrap();
    std::fs::write(format!("{}/clips/kick.wav", r.project()), b"RIFF").unwrap();
    r.open().await;
    let body = json!({ "kind": "slice", "source": "clips/kick.wav", "from": 0.5, "to": 2 });
    let (status, v) = r.post(Tag::Preview, body).await;
    assert_eq!(
        (status, &v),
        (200, &json!({ "ok": true, "start": 0.5, "length": 1.5 }))
    );
    until(30, "the preview frame", || !r.frames("preview").is_empty()).await;
    let sent = &r.frames("preview")[0];
    let want = format!("{}/clips/kick.wav", r.project());
    assert_eq!(sent["file"].as_str(), Some(want.as_str()));
    let kinds = r.a.kinds();
    let (doc, play) = (
        kinds.iter().position(|k| k == "project"),
        kinds.iter().position(|k| k == "preview"),
    );
    assert!(doc.is_some() && doc < play, "{kinds:?}");
    assert_eq!(r.deps().jobs.counts.previews.load(Ordering::Relaxed), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_path_out_of_the_workspace_or_a_missing_file_is_refused_and_sends_nothing() {
    let r = Rig::new("jr-escape").await;
    r.open().await;
    let outside = r.a.dir.join("outside.wav");
    std::fs::write(&outside, b"RIFF").unwrap();
    for (source, why) in [
        (
            "../../../outside.wav",
            "audio path is outside the workspace",
        ),
        (
            outside.to_str().unwrap(),
            "audio path is outside the workspace",
        ),
        ("nothing.wav", "no audio for 'nothing.wav'"),
        ("", "Too small: expected string to have >=1 characters"),
    ] {
        let (status, v) = r
            .post(Tag::Preview, json!({ "kind": "source", "source": source }))
            .await;
        assert_eq!((status, error_of(&v)), (400, why), "{source}");
    }
    assert!(r.frames("preview").is_empty());
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_link_out_of_the_workspace_does_not_play() {
    let r = Rig::new("jr-link").await;
    r.open().await;
    let outside = r.a.dir.join("outside.wav");
    std::fs::write(&outside, b"RIFF").unwrap();
    std::os::unix::fs::symlink(&outside, format!("{}/leak.wav", r.project())).unwrap();
    let (status, v) = r
        .post(
            Tag::Preview,
            json!({ "kind": "source", "source": "leak.wav" }),
        )
        .await;
    assert_eq!(
        (status, error_of(&v)),
        (400, "audio path is outside the workspace")
    );
    assert!(r.frames("preview").is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_thaw_leaves_the_module_live_and_sends_the_project_again() {
    let r = Rig::new("jr-thaw").await;
    r.open().await;
    let session = r.a.engine.as_ref().unwrap().link.session(Some(&r.a.ws));
    until(60, "the project playable", || {
        session.serving().as_deref() == Some(r.project().as_str()) && session.load_phase().is_none()
    })
    .await;
    let first = r.frames("project").len();
    assert!(first > 0);
    let on = json!({ "module": "gtr", "on": true });
    assert_eq!(
        r.post(Tag::AutoFreezeThaw, on).await,
        (200, json!({ "ok": true }))
    );
    let jobs = r.deps().jobs;
    assert_eq!(jobs.thawed.of(&r.project()), ["gtr"]);
    until(30, "the reload's document", || {
        r.frames("project").len() > first
    })
    .await;
    // The freezer's thaw effect may send the project too (2270 collapses them or not), so the count is a floor.
    let off = json!({ "module": "gtr", "on": false });
    assert_eq!(r.post(Tag::AutoFreezeThaw, off).await.0, 200);
    assert!(jobs.thawed.of(&r.project()).is_empty());
    assert_eq!(jobs.counts.thaws.load(Ordering::Relaxed), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_thaw_with_no_open_project_answers_ok_and_changes_nothing() {
    let r = Rig::new("jr-thaw-none").await;
    let on = json!({ "module": "gtr", "on": true });
    assert_eq!(
        r.post(Tag::AutoFreezeThaw, on).await,
        (200, json!({ "ok": true }))
    );
    assert!(r.deps().jobs.thawed.entries().is_empty());
    assert_eq!(r.deps().jobs.counts.thaws.load(Ordering::Relaxed), 0);
}

fn scan(r: &Rig) {
    let studio = r.a.dir.join("home");
    std::fs::create_dir_all(&studio).unwrap();
    let plugins = json!([{ "name": "Café Delay", "format": "AU", "id": "au-1" }]);
    let text = json!({ "entries": [{ "plugins": plugins }] }).to_string();
    std::fs::write(studio.join("exts.json"), text).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_plugin_queries_resolve_a_ref_over_the_scan_and_refuse_what_the_typescript_refuses() {
    let r = Rig::new("jr-plugin").await;
    scan(&r);
    r.a.fake
        .queries(|s| s.params = vec![json!({ "name": "mix" })]);
    let asked = r.get(Tag::PluginParams, "ref=%40engine%2Fcaf-delay").await;
    assert_eq!(
        asked,
        (200, json!({ "found": true, "params": [{ "name": "mix" }] }))
    );
    let frame =
        r.a.fake
            .of(r.a.engine.as_ref().unwrap().link.session(None).name())
            .into_iter()
            .find(|f| f["type"] == "list-plugin-params")
            .unwrap();
    assert_eq!(
        (frame["format"].as_str(), frame["id"].as_str()),
        (Some("AU"), Some("au-1"))
    );
    let none = r.get(Tag::PluginParams, "ref=%40engine%2Fzzz").await;
    assert_eq!(
        (none.0, error_of(&none.1)),
        (404, "no scanned plugin for \"@engine/zzz\"")
    );
    let bare = r.get(Tag::PluginNativeUi, "format=VST3").await;
    assert_eq!(
        (bare.0, error_of(&bare.1)),
        (400, "format and id, or ref, are required")
    );
    // A repeated key is its values joined by commas, as Express hands an array to `String`.
    let twice = r.get(Tag::PluginNativeUi, "format=VST3&id=a&id=b").await;
    assert_eq!(twice, (200, json!({ "opened": "a,b" })));
    assert_eq!(r.deps().jobs.counts.queries.load(Ordering::Relaxed), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_snapshot_makes_the_base_folder_and_the_engine_writes_the_file() {
    let r = Rig::new("jr-snap").await;
    scan(&r);
    let body = json!({ "path": "p.k7/nodes/gtr.k7x", "name": "warm", "ref": "@engine/caf-delay" });
    let done = r.post(Tag::ModuleBaseSnapshot, body.clone()).await;
    assert_eq!(
        done,
        (200, json!({ "path": "p.k7/nodes/base/warm.vstpreset" }))
    );
    assert_eq!(
        std::fs::read(format!("{}/nodes/base/warm.vstpreset", r.project())).unwrap(),
        b"VST3"
    );
    // `warm` and `warm.vstpreset` are one base.
    let again = json!({ "path": "p.k7/nodes/gtr.k7x", "name": "warm.vstpreset", "ref": "@engine/caf-delay" });
    assert_eq!(
        r.post(Tag::ModuleBaseSnapshot, again).await.1["path"],
        "p.k7/nodes/base/warm.vstpreset"
    );
    r.a.fake.queries(|s| s.saved = false);
    let refused = r.post(Tag::ModuleBaseSnapshot, body).await;
    assert_eq!(refused.0, 409);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_snapshot_that_names_no_plugin_a_bad_name_or_a_path_outside_makes_no_folder() {
    let r = Rig::new("jr-snap-bad").await;
    scan(&r);
    let post = |path: &str, name: &str, plugin: &str| {
        r.post(
            Tag::ModuleBaseSnapshot,
            json!({ "path": path, "name": name, "ref": plugin }),
        )
    };
    let (status, v) = post("p.k7/nodes/gtr.k7x", "w", "@engine/zzz").await;
    assert_eq!(
        (status, error_of(&v)),
        (404, "no scanned plugin for \"@engine/zzz\"")
    );
    let (status, v) = post("p.k7/nodes/gtr.k7x", "..", "@engine/caf-delay").await;
    assert_eq!((status, error_of(&v)), (400, "invalid name: .."));
    let (status, _) = post("../../outside/x.k7x", "w", "@engine/caf-delay").await;
    assert_eq!(status, 400);
    assert!(!r.a.dir.join("home/outside").exists());
    assert!(!std::path::Path::new(&format!("{}/nodes/base", r.project())).exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_snapshot_the_engine_never_answers_still_made_its_folder_and_says_503() {
    let r = Rig::new("jr-snap-silent").await;
    scan(&r);
    r.a.fake.queries(|s| s.quiet = true);
    r.a.engine.as_ref().unwrap().link.0.waiters.shrink(150);
    let body = json!({ "path": "p.k7/nodes/gtr.k7x", "name": "w", "ref": "@engine/caf-delay" });
    let (status, v) = r.post(Tag::ModuleBaseSnapshot, body).await;
    assert_eq!(
        (status, error_of(&v)),
        (503, "the engine did not answer — is it running?")
    );
    assert!(std::path::Path::new(&format!("{}/nodes/base", r.project())).is_dir());
}
