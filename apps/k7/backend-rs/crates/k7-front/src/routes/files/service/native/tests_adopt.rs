//! `POST /changes` adopting a job's staged output (3f): only a job of the account that ended with its value is adopted.
//! What a reader gets wrong: the job table is the process's, so each test names its own account and ends only its own jobs.

use std::time::Duration;

use k7_file_service::verb_host::jobs::Table;
use serde_json::{Value, json};

use super::tests_journal::Bed;

fn adopt(job: &str) -> Value {
    json!([{ "kind": "adopt", "job": job, "out": "mix.wav", "file": "p.k7/mix.wav", "replace": false }])
}

fn stage(b: &Bed, job: &str) {
    let dir = std::path::Path::new(&b.home().studio)
        .join(".job-out")
        .join(job);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("mix.wav"), "RIFF").unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_staged_output_is_adopted_only_once_its_job_ended_with_its_value() {
    let b = Bed::new("ch-adopt").await;
    let table = Table::global();
    let t0 = Duration::from_secs(1);
    // The Bed's contexts are the account `acct`.
    let (running, _) = table.start("acct", true, t0).unwrap();
    let (failed, _) = table.start("acct", true, t0).unwrap();
    let (foreign, _) = table.start("other", true, t0).unwrap();
    let (done, _) = table.start("acct", true, t0).unwrap();
    table.fail(&failed, "no", None, t0);
    table.finish(&foreign, json!({}), t0);
    table.finish(&done, json!({}), t0);
    for job in [
        &running,
        &failed,
        &foreign,
        "0123456789abcdef0123456789abcdef",
    ] {
        stage(&b, job);
        let (status, ans) = b.changes(b.set(adopt(job))).await;
        assert_eq!(status, 400, "{job}: {ans}");
        assert!(!b.has("mix.wav"), "{job} landed");
    }
    stage(&b, &done);
    let (status, ans) = b.changes(b.set(adopt(&done))).await;
    assert_eq!(status, 200, "{ans}");
    assert!(b.has("mix.wav"));
    // The step is the journal's: an undo takes the adopted file back.
    let undone = b.ans(super::Tag::Undo, json!({ "confirm": true })).await.1;
    assert_eq!(undone["applied"], json!(true), "{undone}");
    assert!(!b.has("mix.wav"), "{undone}");
    for job in [&running, &failed, &foreign, &done] {
        table.release(job, t0);
    }
}
