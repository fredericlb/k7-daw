//! tests_host.rs — the jobs' memories and the calls the stand-ins make of them (K3): the thawed list's order, the registry's record, a driverless answer.
//! What a reader gets wrong: the arm's and the stand-ins' tests live beside what they cross (`engine/tests_host.rs`, k7-service's `jobs_bridge`).

use std::sync::{Arc, Mutex};

use crate::routes::files::service::native::callbacks::Reply;
use serde_json::{Value, json};

use super::{Driver, Jobs};
use crate::routes::files::service::native::Tag;
use crate::routes::files::service::native::docs;
use crate::routes::files::service::native::tests::Lines;
use crate::routes::files::service::native::tests_docs::{answering, world};

fn value(v: Result<Value, String>) -> Value {
    v.unwrap()["value"].clone()
}

#[test]
fn the_thawed_list_keeps_the_order_thawed_and_is_keyed_by_the_resolved_folder() {
    let jobs = Jobs::new(None);
    for m in ["b", "a", "c"] {
        jobs.query(
            "jobs.thaw",
            &json!({ "dir": "/w/p.k7", "module": m, "on": true }),
        )
        .unwrap();
    }
    jobs.query(
        "jobs.thaw",
        &json!({ "dir": "/w/p.k7/", "module": "a", "on": false }),
    )
    .unwrap();
    let got = value(jobs.query("jobs.thawed", &json!({ "dir": "/w/x/../p.k7" })));
    assert_eq!(got, json!(["b", "c"]));
    // A list set whole keeps a repeated name at its first place; an empty one forgets the folder.
    jobs.query(
        "jobs.setThawed",
        &json!({ "dir": "/w/p.k7", "modules": ["z", "y", "z"] }),
    )
    .unwrap();
    assert_eq!(
        value(jobs.query("jobs.thawed", &json!({ "dir": "/w/p.k7" }))),
        json!(["z", "y"])
    );
    jobs.query(
        "jobs.setThawed",
        &json!({ "dir": "/w/p.k7", "modules": [] }),
    )
    .unwrap();
    assert_eq!(
        value(jobs.query("jobs.thawed", &json!({ "dir": "/w/p.k7" }))),
        json!([])
    );
    assert_eq!(
        jobs.counts
            .set_thawed
            .load(std::sync::atomic::Ordering::Relaxed),
        2
    );
}

#[test]
fn a_cut_with_no_machine_is_the_typescripts_failed_record_and_is_found_by_its_id() {
    let jobs = Jobs::new(None);
    let args = json!({ "userEmail": "a@b", "source": "/s.wav", "model": "/m.onnx", "outputs": [], "outDir": "/o" });
    let job = value(jobs.query("separate.start", &args));
    assert_eq!(job["failed"], json!(true));
    assert_eq!(
        job["error"],
        json!("no engine to cut on — open this project on an engine first")
    );
    let found = value(jobs.query("separate.job", &json!({ "id": job["id"] })));
    assert_eq!(found, job);
    assert_eq!(
        value(jobs.query("separate.job", &json!({ "id": "nope" }))),
        Value::Null
    );
}

struct Bound(Mutex<Vec<(String, u64)>>);

impl Driver for Bound {
    fn renderer_available(&self) -> bool {
        true
    }
    fn bound(&self) -> bool {
        true
    }
    fn render_run_until(&self, args: Value, reply: Reply, _gone: super::Gone) {
        reply(Ok(
            json!({ "value": { "mix": args["outDir"], "stems": [] } }),
        ));
    }
    fn render_all(&self, dir: String, reply: Reply) {
        reply(Ok(json!({ "value": [{ "module": dir, "plugins": [] }] })));
    }
    fn separate(&self, id: &str, generation: u64, _args: Value) {
        self.0.lock().unwrap().push((id.to_owned(), generation));
    }
}

#[test]
fn a_cut_started_twice_is_one_request_and_the_asks_reach_the_driver() {
    let driver = Arc::new(Bound(Mutex::default()));
    let jobs = Jobs::new(Some(driver.clone()));
    let args = json!({ "userEmail": "a@b", "source": "/s.wav", "model": "/m.onnx", "outputs": ["vocals"], "outDir": "/o" });
    let first = value(jobs.query("separate.start", &args));
    let second = value(jobs.query("separate.start", &args));
    assert_eq!(first, second);
    assert_eq!(first["phase"], json!("sending"));
    assert_eq!(driver.0.lock().unwrap().len(), 1);
    assert_eq!(
        value(jobs.query("render.available", &json!({}))),
        json!(true)
    );
    let got: Arc<Mutex<Vec<Value>>> = Arc::default();
    let into = got.clone();
    jobs.ask(
        "render.run",
        json!({ "outDir": "/x" }),
        Box::new(move |r| into.lock().unwrap().push(r.unwrap())),
    );
    let into = got.clone();
    jobs.ask(
        "jobs.renderAll",
        json!({ "dir": "/d" }),
        Box::new(move |r| into.lock().unwrap().push(r.unwrap())),
    );
    let got = got.lock().unwrap().clone();
    assert_eq!(got[0]["value"]["mix"], json!("/x"));
    assert_eq!(got[1]["value"][0]["module"], json!("/d"));
}

#[test]
fn with_no_driver_nothing_renders_and_the_sentence_is_the_typescripts() {
    let jobs = Jobs::new(None);
    assert_eq!(
        value(jobs.query("render.available", &json!({}))),
        json!(false)
    );
    let seen: Arc<Mutex<Vec<Result<Value, String>>>> = Arc::default();
    let into = seen.clone();
    jobs.ask(
        "render.run",
        json!({}),
        Box::new(move |r| into.lock().unwrap().push(r)),
    );
    assert_eq!(
        seen.lock().unwrap()[0],
        Err("no engine machine to render on".to_owned())
    );
    assert!(jobs.query("jobs.nothing", &json!({})).is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_ask_reads_the_thawed_list_from_the_jobs_not_from_the_runtime() {
    let (dir, acct) = world("jobs-thawed").await;
    let lines = Lines::default();
    let project = dir.join("home/workspace/songs/p.k7").display().to_string();
    let jobs = Jobs::new(None);
    jobs.thawed.set(&project, ["b".to_owned(), "a".to_owned()]);
    let mut c = answering(&acct, &dir, Tag::EngineDoc, &lines);
    let asked = |c: &crate::routes::files::service::native::Ctx| {
        let a = json!({ "active": project, "rev": "r", "thawed": ["from-the-host"] });
        docs::prepare(c, serde_json::from_value(a).unwrap()).map_err(|e| e.to_owned())
    };
    assert_eq!(asked(&c).unwrap().ask.thawed, ["from-the-host"]);
    c.docs = c.docs.take().map(|d| d.with_thawed(jobs.thawed.clone()));
    assert_eq!(asked(&c).unwrap().ask.thawed, ["b", "a"]);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_tap_hears_every_progress_event_with_or_without_a_runtime() {
    let jobs = Jobs::new(None);
    let heard: Arc<Mutex<Vec<Value>>> = Arc::default();
    let into = heard.clone();
    jobs.tap(Arc::new(move |v| into.lock().unwrap().push(v)));
    let sunk: Arc<Mutex<Vec<Value>>> = Arc::default();
    let into = sunk.clone();
    jobs.awaiting("/d");
    jobs.emit(json!({ "kind": "jobs", "dir": "/d", "done": 0 }));
    jobs.set_events(Some(Arc::new(move |v| into.lock().unwrap().push(v))));
    jobs.emit(json!({ "kind": "jobs", "dir": "/d", "done": 1 }));
    assert_eq!(heard.lock().unwrap().len(), 2);
    assert_eq!(
        *sunk.lock().unwrap(),
        [json!({ "kind": "jobs", "dir": "/d", "done": 1 })]
    );
}

/// A driver that keeps the `renderAll` reply unmade, so the ask stays in flight.
struct Holding(Mutex<Vec<Reply>>);

impl Driver for Holding {
    fn renderer_available(&self) -> bool {
        true
    }
    fn bound(&self) -> bool {
        true
    }
    fn render_run_until(&self, _args: Value, _reply: Reply, _gone: super::Gone) {}
    fn render_all(&self, _dir: String, reply: Reply) {
        self.0.lock().unwrap().push(reply);
    }
    fn separate(&self, _id: &str, _generation: u64, _args: Value) {}
}

#[test]
fn a_freeze_progress_reaches_the_runtime_only_while_its_render_all_is_asked() {
    let driver = Arc::new(Holding(Mutex::default()));
    let jobs = Jobs::new(Some(driver.clone()));
    let heard: Arc<Mutex<u32>> = Arc::default();
    let into = heard.clone();
    jobs.set_events(Some(Arc::new(move |_| *into.lock().unwrap() += 1)));
    let event = json!({ "kind": "jobs", "dir": "/d", "done": 1, "total": 2, "module": "m" });
    jobs.emit(event.clone());
    assert_eq!(*heard.lock().unwrap(), 0, "a share's freeze woke the sink");
    jobs.ask("jobs.renderAll", json!({ "dir": "/d" }), Box::new(|_| {}));
    jobs.emit(event.clone());
    assert_eq!(
        *heard.lock().unwrap(),
        1,
        "the asked folder's progress is lost"
    );
    jobs.emit(json!({ "kind": "jobs", "dir": "/other", "module": "m" }));
    assert_eq!(
        *heard.lock().unwrap(),
        1,
        "another folder's progress got in"
    );
    // An ask dropped unmade settles too.
    driver.0.lock().unwrap().clear();
    jobs.emit(event);
    assert_eq!(*heard.lock().unwrap(), 1, "the wait outlived its reply");
}
