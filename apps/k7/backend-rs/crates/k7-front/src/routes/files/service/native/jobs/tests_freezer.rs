//! tests_freezer.rs — the freezer's driver over a real account and a fake link: the real compile, `renderAll`, a pass, the thaw.
//! What a reader gets wrong: the compile of these tests is the real one only where a test says so; the others inject a list of freezes.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crate::routes::files::service::Acct;

use k7_file_service::jobs::AutoFreeze;
use serde_json::{Value, json};

use super::freeze_run::Freezing;
use super::{Driver, Jobs, RenderFuture};
use crate::routes::files::service::native::docs::Docs;
use crate::routes::files::service::native::engine::testkit::{Rig, until};
use crate::routes::files::service::native::tests_docs::{code_root, world};

/// A freeze of `module` rendering to `<dir>/<module>.wav`, made or not.
pub(super) fn freeze(dir: &Path, module: &str, hosted: bool) -> AutoFreeze {
    let render = dir.join(format!("{module}.wav")).display().to_string();
    serde_json::from_value(json!({
        "module": module, "render": render, "ready": false, "hosted": hosted,
        "plugins": [{ "format": "VST3", "name": module }],
        "plan": { "at": module, "render": render, "lanes": [], "modules": [] },
        "cut": serde_json::to_value(song::doc::ProjectDoc::empty()).unwrap(),
    }))
    .unwrap()
}

/// A driver whose renders write the file, or say why not.
#[derive(Default)]
pub(super) struct Stub {
    pub(super) fail: bool,
    pub(super) asked: Mutex<Vec<PathBuf>>,
}

impl Driver for Stub {
    fn renderer_available(&self) -> bool {
        true
    }
    fn bound(&self) -> bool {
        false
    }
    fn render_run_until(
        &self,
        _a: Value,
        r: crate::routes::files::service::native::callbacks::Reply,
        _g: super::Gone,
    ) {
        r(Err("not here".into()));
    }
    fn render_all(&self, _d: String, r: crate::routes::files::service::native::callbacks::Reply) {
        r(Ok(json!({ "value": [] })));
    }
    fn separate(&self, _i: &str, _g: u64, _a: Value) {}
    fn render_freeze(&self, _doc: String, out: PathBuf) -> RenderFuture {
        self.asked.lock().unwrap().push(out.clone());
        let fail = self.fail;
        Box::pin(async move {
            if fail {
                return Err("the plugin crashed".into());
            }
            std::fs::write(out, b"RIFFfake").map_err(|e| e.to_string())
        })
    }
}

pub(super) struct Fixture {
    pub(super) rig: Rig,
    pub(super) jobs: Arc<Jobs>,
    pub(super) freezing: Arc<Freezing>,
    pub(super) stub: Arc<Stub>,
    pub(super) dir: PathBuf,
    pub(super) events: Arc<Mutex<Vec<Value>>>,
    pub(super) _acct: Arc<Acct>,
}

pub(super) async fn fixture(name: &str, fail: bool) -> Fixture {
    let stub = Arc::new(Stub {
        fail,
        ..Stub::default()
    });
    let driver = stub.clone();
    fixture_on(name, stub, move |_, _| driver).await
}

/// The same over the driver `make` builds from the rig and the folder; `stub` is only what the test reads back.
pub(super) async fn fixture_on(
    name: &str,
    stub: Arc<Stub>,
    make: impl FnOnce(&Rig, &Path) -> Arc<dyn Driver>,
) -> Fixture {
    let rig = Rig::new(name).await;
    let (dir, acct) = world(name).await;
    let jobs = Jobs::new(Some(make(&rig, &dir)));
    let docs = Docs::of(&acct, &code_root()).with_thawed(jobs.thawed.clone());
    let freezing = Freezing::new(
        &jobs,
        rig.link.clone(),
        docs,
        tokio::runtime::Handle::current(),
    );
    let _ = jobs.freezing.set(freezing.clone());
    let events: Arc<Mutex<Vec<Value>>> = Arc::default();
    let into = events.clone();
    jobs.set_events(Some(Arc::new(move |v| into.lock().unwrap().push(v))));
    Fixture {
        rig,
        jobs,
        freezing,
        stub,
        dir,
        events,
        _acct: acct,
    }
}

pub(super) fn inject(f: &Fixture, freezes: Vec<AutoFreeze>) {
    *f.freezing.fake.lock().unwrap() = Some(Arc::new(move |_| Ok(freezes.clone())));
}

pub(super) async fn render_all(f: &Fixture, dir: &str) -> Result<Value, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    f.jobs.awaiting(dir);
    f.freezing
        .render_all(dir, Box::new(move |r| drop(tx.send(r))));
    let answer = rx.await.unwrap().map(|v| v["value"].clone());
    f.jobs.awaited.end(dir);
    answer
}

#[tokio::test]
async fn the_real_compile_of_a_project_with_no_vst_finds_no_freeze() {
    let f = fixture("fz-real", false).await;
    let project = f
        .dir
        .join("home/workspace/songs/p.k7")
        .display()
        .to_string();
    assert_eq!(f.freezing.compile(&project).await.unwrap().len(), 0);
    assert_eq!(render_all(&f, &project).await.unwrap(), json!([]));
}

#[tokio::test]
async fn render_all_renders_what_a_machine_can_and_names_what_none_can_with_its_progress() {
    let f = fixture("fz-all", false).await;
    let out = f.dir.join("renders");
    std::fs::create_dir_all(&out).unwrap();
    inject(
        &f,
        vec![freeze(&out, "gtr", true), freeze(&out, "lead", false)],
    );
    let impossible = render_all(&f, "/p.k7").await.unwrap();
    assert_eq!(
        impossible,
        json!([{ "module": "lead", "plugins": ["lead"] }])
    );
    assert!(out.join("gtr.wav").exists() && !out.join("lead.wav").exists());
    let progress: Vec<(u64, u64, String)> = f
        .events
        .lock()
        .unwrap()
        .iter()
        .map(|e| {
            (
                e["done"].as_u64().unwrap(),
                e["total"].as_u64().unwrap(),
                e["module"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert_eq!(progress, [(0, 1, "gtr".into()), (1, 1, String::new())]);
    assert_eq!(f.events.lock().unwrap()[0]["kind"], "jobs");
    assert_eq!(f.jobs.counts.renders.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn a_render_that_fails_is_impossible_and_the_pass_ends() {
    let f = fixture("fz-fail", true).await;
    let out = f.dir.join("renders");
    std::fs::create_dir_all(&out).unwrap();
    inject(&f, vec![freeze(&out, "gtr", true)]);
    let impossible = render_all(&f, "/p.k7").await.unwrap();
    assert_eq!(impossible, json!([{ "module": "gtr", "plugins": ["gtr"] }]));
    // The next ask is not stuck behind a pass that failed.
    assert_eq!(render_all(&f, "/p.k7").await.unwrap(), impossible);
    assert_eq!(f.stub.asked.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn a_compile_that_fails_or_panics_answers_the_ask_and_frees_the_folder() {
    let f = fixture("fz-bad", false).await;
    *f.freezing.fake.lock().unwrap() = Some(Arc::new(|_| Err("no such project".into())));
    let said = render_all(&f, "/p.k7").await;
    assert!(said.is_err(), "{said:?}");
    *f.freezing.fake.lock().unwrap() = Some(Arc::new(|_| panic!("a compile that dies")));
    assert!(render_all(&f, "/p.k7").await.is_err());
    assert_eq!(f.jobs.counts.panics.load(Ordering::Relaxed), 1);
    inject(&f, vec![]);
    assert_eq!(render_all(&f, "/p.k7").await.unwrap(), json!([]));
}

#[tokio::test]
async fn a_thaw_reaches_the_machine_and_the_list_the_compile_reads() {
    let f = fixture("fz-thaw", false).await;
    f.jobs.thaw("/p.k7", "synth", true);
    f.jobs.thaw("/p.k7", "pad", true);
    assert_eq!(f.freezing.thawed_of("/p.k7"), ["synth", "pad"]);
    assert_eq!(f.jobs.thawed.of("/p.k7"), ["synth", "pad"]);
    f.jobs.thaw("/p.k7", "synth", false);
    assert_eq!(f.freezing.thawed_of("/p.k7"), ["pad"]);
    f.jobs
        .query(
            "jobs.setThawed",
            &json!({ "dir": "/p.k7", "modules": ["x"] }),
        )
        .unwrap();
    assert_eq!(f.freezing.thawed_of("/p.k7"), ["x"]);
}

#[tokio::test]
async fn a_tick_of_a_served_project_renders_what_is_missing_once_per_revision() {
    let f = fixture("fz-tick", false).await;
    let out = f.dir.join("renders");
    std::fs::create_dir_all(&out).unwrap();
    inject(&f, vec![freeze(&out, "gtr", true)]);
    let session = f.rig.session();
    session.start(&f.rig.ws, &f.rig.project).await;
    until(10, "the session ready", || session.is_ready()).await;
    f.freezing.tick(&session);
    until(10, "the render", || out.join("gtr.wav").exists()).await;
    until(10, "the pass ended", || {
        f.jobs.counts.freezes.load(Ordering::Relaxed) == 1
    })
    .await;
    // The same revision again: nothing more is rendered.
    f.freezing.tick(&session);
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert_eq!(f.stub.asked.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn a_thousand_freezes_in_one_project_render_once_each_in_order() {
    let f = fixture("fz-1000", false).await;
    let out = f.dir.join("renders");
    std::fs::create_dir_all(&out).unwrap();
    let many: Vec<AutoFreeze> = (0..1000)
        .map(|i| freeze(&out, &format!("m{i}"), true))
        .collect();
    inject(&f, many);
    assert_eq!(render_all(&f, "/p.k7").await.unwrap(), json!([]));
    let order: Vec<PathBuf> = (0..1000).map(|i| out.join(format!("m{i}.wav"))).collect();
    assert_eq!(*f.stub.asked.lock().unwrap(), order);
    assert_eq!(f.events.lock().unwrap().len(), 1001);
}

#[tokio::test]
async fn a_second_thaw_during_the_first_ones_write_leaves_the_machines_list_in_the_compiles_hands()
{
    let f = fixture("fz-race", false).await;
    *f.freezing.slow_apply.lock().unwrap() = Some(std::time::Duration::from_millis(150));
    let jobs = f.jobs.clone();
    let first = std::thread::spawn(move || jobs.thaw("/p.k7", "x", true));
    std::thread::sleep(std::time::Duration::from_millis(40));
    f.jobs.thaw("/p.k7", "y", true);
    first.join().unwrap();
    assert_eq!(f.freezing.thawed_of("/p.k7"), ["x", "y"]);
    assert_eq!(f.jobs.thawed.of("/p.k7"), ["x", "y"]);
}

#[tokio::test]
async fn renderall_asks_made_behind_a_pass_are_answered_in_the_order_they_were_made() {
    let f = fixture("fz-fifo", false).await;
    let passes = Arc::new(AtomicUsize::new(0));
    let count = passes.clone();
    let out = f.dir.join("renders");
    *f.freezing.fake.lock().unwrap() = Some(Arc::new(move |_| {
        // Pass n names module `p<n>` as impossible, so each ask's answer says which pass it got.
        match count.fetch_add(1, Ordering::Relaxed) + 1 {
            1 => Ok(vec![]),
            n => Ok(vec![freeze(&out, &format!("p{n}"), false)]),
        }
    }));
    let ask = || {
        let (tx, rx) = tokio::sync::oneshot::channel();
        f.freezing
            .render_all("/p.k7", Box::new(move |r| drop(tx.send(r))));
        rx
    };
    let asks = [ask(), ask(), ask()];
    let mut said = Vec::new();
    for rx in asks {
        said.push(rx.await.unwrap().unwrap()["value"].clone());
    }
    let one = |m: &str| json!([{ "module": m, "plugins": [m] }]);
    assert_eq!(said, [json!([]), one("p2"), one("p3")]);
}
