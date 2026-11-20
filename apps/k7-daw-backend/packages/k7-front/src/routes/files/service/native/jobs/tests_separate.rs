//! tests_separate.rs — a cut through `separate.start` as the stand-in sends it, the machine a fake jobs path: the record at once, its progress, its end.
//! What a reader gets wrong: the registry's id is this side's, whatever id the machine's record carries.

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use super::Jobs;
use super::driver::{DriverEnv, real_driver};
use crate::routes::files::service::native::engine::fake::{FakeJobs, JobEnd};
use crate::routes::files::service::native::engine::testkit::{Rig, until};

/// The object `separation.ts` builds for `separate.start`.
fn start_args() -> Value {
    json!({ "userEmail": "a@b.c", "source": "songs/x.wav", "model": "htdemucs", "outputs": ["vocals", "drums"], "outDir": "/out" })
}

fn jobs_over(rig: &Rig) -> Arc<Jobs> {
    Jobs::new(Some(real_driver(
        &rig.link,
        DriverEnv::new("/none".into(), vec![]),
    )))
}

fn record(jobs: &Jobs, id: &str) -> Value {
    jobs.query("separate.job", &json!({ "id": id })).unwrap()["value"].clone()
}

async fn started(jobs: &Jobs) -> (String, Value) {
    let first = jobs.query("separate.start", &start_args()).unwrap()["value"].clone();
    (first["id"].as_str().unwrap().to_owned(), first)
}

#[tokio::test]
async fn a_cut_answers_at_once_then_follows_the_machines_records_under_its_own_id() {
    let rig = Rig::new("sep-ok").await;
    let machine = FakeJobs::start().await;
    machine.takes(Duration::from_millis(100));
    let finished = json!({ "id": "the-machines-id", "source": "songs/x.wav", "done": true, "failed": false, "error": null,
        "phase": "done", "percent": 100, "lines": [], "files": ["vocals.wav", "drums.wav"] });
    machine.ends(JobEnd::Separated { job: finished });
    rig.link
        .bind(&json!({ "url": rig.engine.url, "engineId": "e1", "jobsUrl": machine.url }));
    let jobs = jobs_over(&rig);
    let (id, first) = started(&jobs).await;
    assert_eq!(
        (first["phase"].as_str(), first["done"].as_bool()),
        (Some("sending"), Some(false))
    );
    until(10, "the cut's end", || record(&jobs, &id)["done"] == true).await;
    let done = record(&jobs, &id);
    assert_eq!(
        (done["id"].as_str(), done["files"].clone()),
        (Some(id.as_str()), json!(["vocals.wav", "drums.wav"]))
    );
    let asked = &machine.asked()[0];
    assert_eq!(
        (
            asked["type"].as_str(),
            asked["user"].as_str(),
            asked["outDir"].as_str()
        ),
        (Some("separate"), Some("a@b.c"), Some("/out"))
    );
    assert_eq!(asked["outputs"], json!(["vocals", "drums"]));
}

#[tokio::test]
async fn the_same_work_started_a_thousand_times_is_one_request() {
    let rig = Rig::new("sep-many").await;
    let machine = FakeJobs::start().await;
    machine.ends(JobEnd::Never);
    rig.link
        .bind(&json!({ "url": rig.engine.url, "engineId": "e1", "jobsUrl": machine.url }));
    let jobs = jobs_over(&rig);
    let (id, _) = started(&jobs).await;
    for _ in 0..999 {
        assert_eq!(
            jobs.query("separate.start", &start_args()).unwrap()["value"]["id"],
            id.as_str()
        );
    }
    until(10, "the request", || !machine.asked().is_empty()).await;
    assert_eq!(machine.asked().len(), 1);
}

#[tokio::test]
async fn a_machines_refusal_is_its_own_words_and_a_dead_path_is_the_machines_failure() {
    let rig = Rig::new("sep-fail").await;
    let machine = FakeJobs::start().await;
    machine.ends(JobEnd::Error("model missing".into()));
    rig.link
        .bind(&json!({ "url": rig.engine.url, "engineId": "e1", "jobsUrl": machine.url }));
    let jobs = jobs_over(&rig);
    let (id, _) = started(&jobs).await;
    until(10, "the refusal", || record(&jobs, &id)["failed"] == true).await;
    assert_eq!(record(&jobs, &id)["error"], "model missing");
    let dead = Rig::new("sep-dead").await;
    dead.link
        .bind(&json!({ "url": dead.engine.url, "engineId": "e1", "jobsUrl": "ws://127.0.0.1:1" }));
    let jobs = jobs_over(&dead);
    let (id, _) = started(&jobs).await;
    until(10, "the failure", || record(&jobs, &id)["failed"] == true).await;
    let said = record(&jobs, &id)["error"].as_str().unwrap().to_owned();
    assert!(said.starts_with("the engine's machine: "), "{said}");
}

#[tokio::test]
async fn with_no_machine_bound_a_cut_fails_at_once_and_sends_nothing() {
    let rig = Rig::new("sep-none").await;
    let jobs = jobs_over(&rig);
    let (_, first) = started(&jobs).await;
    assert_eq!(first["failed"], true);
    assert_eq!(
        first["error"],
        "no engine to cut on — open this project on an engine first"
    );
}

#[tokio::test]
async fn a_binding_that_dropped_after_the_start_fails_the_record_instead_of_leaving_it_sending() {
    let rig = Rig::new("sep-dropped").await;
    let jobs = jobs_over(&rig);
    // `start` saw a bound machine; by the time the driver sends, the binding is gone.
    let started = jobs
        .registry()
        .start(std::time::Instant::now(), "dropped", "songs/x.wav", true);
    assert!(started.send);
    jobs.driver
        .as_ref()
        .unwrap()
        .separate("dropped", started.generation, start_args());
    let rec = record(&jobs, "dropped");
    assert_eq!(rec["failed"], true, "{rec}");
    assert_eq!(
        rec["error"],
        "no engine to cut on — open this project on an engine first"
    );
}
