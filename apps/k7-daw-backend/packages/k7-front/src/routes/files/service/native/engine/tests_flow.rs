//! The flows against a fake engine and a fake jobs agent: the copy gate, the scan wait, the reconcile, the unload and
//! the pattern watch (2220 to 2228).

use std::sync::atomic::Ordering;
use std::time::Duration;

use k7_file_service::engine::Phase;
use serde_json::{Value, json};

use super::fake::{FakeJobs, JobEnd, Rig, until};
use super::stats::Stats;

fn with_file(rig: &Rig, name: &str) -> (String, Value) {
    let p = format!("{}/ws/{name}", rig.dir.display());
    std::fs::write(&p, "x".repeat(10)).unwrap();
    let mut doc = super::fake::minimal_doc(-3.0);
    doc["recordDir"] = json!(p);
    (p, doc)
}

fn bind_jobs(rig: &Rig, jobs: &FakeJobs) {
    rig.link
        .bind(&json!({ "url": rig.engine.url, "jobsUrl": jobs.url, "engineId": "e1" }));
}

fn projects(rig: &Rig) -> usize {
    rig.engine
        .kinds(Rig::SESSION)
        .iter()
        .filter(|k| *k == "project")
        .count()
}

#[tokio::test]
async fn a_document_waits_for_its_files_and_a_settled_file_is_not_copied_twice() {
    let rig = Rig::new("copy").await;
    let jobs = FakeJobs::start().await;
    jobs.takes(Duration::from_millis(500));
    bind_jobs(&rig, &jobs);
    let (path, doc) = with_file(&rig, "a.wav");
    *rig.host.doc.lock().unwrap() = doc;
    let s = rig.session();
    s.start(&rig.ws, &rig.project).await;
    until(5, "the job", || jobs.asked().len() == 1).await;
    assert_eq!(jobs.asked()[0]["files"], json!([path]));
    assert_eq!(projects(&rig), 0, "nothing is sent before the copy ends");
    assert_eq!(s.sync_state().map(|x| x.files), Some(1));
    until(10, "the document", || projects(&rig) >= 1).await;
    s.reload(&rig.ws, &rig.project).await;
    until(10, "the second document", || projects(&rig) >= 2).await;
    assert_eq!(jobs.asked().len(), 1, "the file is settled");
    assert_eq!(Stats::get(&rig.link.stats().copies), 1);
}

#[tokio::test]
async fn a_failed_copy_holds_the_document_until_the_person_retries() {
    let rig = Rig::new("fail").await;
    let jobs = FakeJobs::start().await;
    jobs.ends(JobEnd::Synced {
        failed: 1,
        failures: vec!["a.wav: gone".into()],
    });
    bind_jobs(&rig, &jobs);
    let (_, doc) = with_file(&rig, "a.wav");
    *rig.host.doc.lock().unwrap() = doc;
    let s = rig.session();
    s.start(&rig.ws, &rig.project).await;
    until(5, "the failure", || {
        s.sync_state().is_some_and(|x| x.failed == Some(1))
    })
    .await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(projects(&rig), 0);
    assert_eq!(
        s.engine_load_of(Some(&rig.project)),
        None,
        "a failed copy ends the wait"
    );
    jobs.ends(JobEnd::Synced {
        failed: 0,
        failures: vec![],
    });
    s.retry_sync(&rig.ws, &rig.project).await;
    until(10, "the document after the retry", || projects(&rig) >= 1).await;
    assert_eq!(jobs.asked().len(), 2);
}

#[tokio::test]
async fn a_path_outside_the_roots_is_never_stated() {
    let rig = Rig::new("roots").await;
    let jobs = FakeJobs::start().await;
    bind_jobs(&rig, &jobs);
    let mut doc = super::fake::minimal_doc(-3.0);
    doc["recordDir"] = json!("/etc/passwd");
    *rig.host.doc.lock().unwrap() = doc;
    let s = rig.session();
    s.start(&rig.ws, &rig.project).await;
    until(10, "the document", || projects(&rig) >= 1).await;
    assert!(jobs.asked().is_empty(), "{:?}", jobs.asked());
}

#[tokio::test]
async fn a_load_naming_an_external_plugin_waits_for_the_scan_then_goes_compiled_anew() {
    let rig = Rig::new("scan").await;
    rig.host.external.store(true, Ordering::Relaxed);
    rig.engine.answer_scan_with(vec![
        json!({ "type": "plugin-scan", "scanning": true, "done": 1, "total": 2, "current": "x", "version": 1 }),
    ]);
    rig.engine.plugins(vec![json!({ "name": "Surge" })]);
    let s = rig.session();
    s.start(&rig.ws, &rig.project).await;
    until(5, "the hold", || s.load_phase() == Some(Phase::Scanning)).await;
    assert_eq!(projects(&rig), 0);
    until(5, "the scan's word", || {
        s.scan_view().map(|v| (v.done, v.total)) == Some((1, 2))
    })
    .await;
    rig.engine
        .send_all(json!({ "type": "plugins", "plugins": [{ "name": "Surge" }] }));
    rig.engine.send_all(
        json!({ "type": "plugin-scan", "scanning": false, "done": 2, "total": 2, "version": 2 }),
    );
    until(10, "the catalogue", || {
        !rig.host.catalogues.lock().unwrap().is_empty()
    })
    .await;
    until(10, "the project", || projects(&rig) >= 1).await;
    assert!(
        rig.host.invalidated.load(Ordering::Relaxed) >= 1,
        "compiled anew"
    );
}

#[tokio::test]
async fn a_silent_engine_is_redialled_by_the_poll() {
    let rig = Rig::new("deaf").await;
    let s = rig.session();
    s.start(&rig.ws, &rig.project).await;
    until(15, "the load to end", || {
        s.load_phase().is_none() && projects(&rig) >= 3
    })
    .await;
    rig.engine.silent(true);
    s.prewarm(&rig.ws, &rig.project).await;
    until(10, "the re-dial", || {
        Stats::get(&rig.link.stats().redials) == 1
    })
    .await;
    until(10, "a new connection", || rig.engine.accepted() >= 2).await;
    assert!(
        rig.host.said().iter().any(|l| l.contains("unresponsive")),
        "{:?}",
        rig.host.said()
    );
}

#[tokio::test]
async fn an_engine_not_holding_the_project_is_sent_it_again() {
    let rig = Rig::new("resend").await;
    let s = rig.session();
    s.start(&rig.ws, &rig.project).await;
    until(15, "the load to end", || {
        s.load_phase().is_none() && projects(&rig) >= 3
    })
    .await;
    rig.engine.loaded_after(Duration::from_secs(3600));
    let before = projects(&rig);
    s.prewarm(&rig.ws, &rig.project).await;
    until(10, "the re-send", || projects(&rig) > before).await;
    assert!(Stats::get(&rig.link.stats().resends) >= 1);
}

#[tokio::test]
async fn an_unload_stops_flushes_and_forgets_the_session() {
    let rig = Rig::new("unload").await;
    let s = rig.session();
    s.start(&rig.ws, &rig.project).await;
    until(15, "the load to end", || {
        s.load_phase().is_none() && projects(&rig) >= 3
    })
    .await;
    s.unload().await;
    until(5, "drop-session", || {
        rig.engine
            .kinds(Rig::SESSION)
            .contains(&"drop-session".to_string())
    })
    .await;
    let kinds = rig.engine.kinds(Rig::SESSION);
    let tail: Vec<&str> = kinds
        .iter()
        .map(String::as_str)
        .skip(kinds.len() - 4)
        .collect();
    assert_eq!(
        tail,
        ["transport stop", "flush", "record-enable", "drop-session"]
    );
    assert_eq!(s.served(), None);
    assert!(!s.has_socket());
}

#[tokio::test]
async fn a_saved_pattern_the_compile_inlines_recompiles_once_and_another_does_not() {
    let rig = Rig::new("pattern").await;
    k7_file_service::watch::Watch::global().watch(&rig.ws);
    let s = rig.session();
    s.start(&rig.ws, &rig.project).await;
    until(15, "the load to end", || {
        s.load_phase().is_none() && projects(&rig) >= 3
    })
    .await;
    let beat = format!("{}/beat.strudel", rig.project);
    *rig.host.patterns.lock().unwrap() = vec![beat.clone()];
    let compiles = rig.host.compiled.lock().unwrap().len();
    std::fs::write(format!("{}/other.strudel", rig.project), "s('hh')").unwrap();
    tokio::time::sleep(Duration::from_millis(800)).await;
    assert_eq!(
        rig.host.compiled.lock().unwrap().len(),
        compiles,
        "a pattern nothing inlines"
    );
    // A fresh event stream can miss the first write under load, so the save is repeated until one is heard.
    for n in 0..30 {
        std::fs::write(&beat, format!("s('bd{n}')")).unwrap();
        tokio::time::sleep(Duration::from_secs(1)).await;
        if rig.host.compiled.lock().unwrap().len() > compiles {
            return;
        }
    }
    panic!("a saved pattern was never recompiled");
}

#[tokio::test]
async fn with_no_engine_bound_the_poll_says_copying() {
    let rig = Rig::new("nobind").await;
    let s = rig.session();
    s.start(&rig.ws, &rig.project).await;
    until(15, "the load to end", || {
        s.load_phase().is_none() && projects(&rig) >= 3
    })
    .await;
    rig.link.bind(&json!({ "url": null }));
    assert_eq!(s.engine_load_of(Some(&rig.project)), Some(Phase::Copying));
}
