//! The link against a fake engine on loopback: the frames' order, the dry run on the wire, the socket's life (2214, 2216).

use std::time::Duration;

use serde_json::json;

use super::fake::{FakeEngine, Rig, until};
use super::stats::Stats;

fn masters(rig: &Rig) -> Vec<(String, f64)> {
    rig.engine
        .of(Rig::SESSION)
        .iter()
        .filter(|f| f["type"] == "project")
        .map(|f| {
            (
                "project".to_string(),
                f["doc"]["masterVolume"].as_f64().unwrap(),
            )
        })
        .collect()
}

/// The master levels of the project frames, a document sent twice in a row counted once.
fn levels(rig: &Rig) -> Vec<f64> {
    let mut v: Vec<f64> = masters(rig).iter().map(|m| m.1).collect();
    v.dedup();
    v
}

#[tokio::test]
async fn an_open_goes_through_the_gate_and_the_dry_run_is_silent_on_the_wire() {
    let rig = Rig::new("dry").await;
    let s = rig.session();
    s.start(&rig.ws, &rig.project).await;
    until(15, "the load to end", || {
        s.load_phase().is_none() && levels(&rig).len() >= 3
    })
    .await;
    // The document (twice if the first open re-sends it, as the TypeScript's does), the silenced copy, the restore.
    assert_eq!(levels(&rig), vec![-3.0, -100.0, -3.0]);
    let kinds = rig.engine.kinds(Rig::SESSION);
    let at = |k: &str| {
        kinds
            .iter()
            .position(|x| x == k)
            .unwrap_or_else(|| panic!("{k} in {kinds:?}"))
    };
    assert!(at("record-enable") < at("transport play"));
    assert!(at("transport play") < at("transport stop"));
    assert!(at("transport stop") < at("transport rewind"));
    assert_eq!(kinds.iter().filter(|k| *k == "transport play").count(), 1);
    // Every frame carries the session, the bare one omitted.
    assert!(rig.engine.of(Rig::SESSION).len() == rig.engine.got().len());
    assert_eq!(Stats::get(&rig.link.stats().loads), 1);
}

#[tokio::test]
async fn a_socket_that_drops_and_comes_back_mid_apply_keeps_its_link_and_its_load() {
    let rig = Rig::new("blip").await;
    rig.engine.loaded_after(Duration::from_millis(1500));
    let s = rig.session();
    s.start(&rig.ws, &rig.project).await;
    until(5, "the document", || !masters(&rig).is_empty()).await;
    let link = s.link_no().unwrap();
    rig.engine.close_all();
    until(10, "the reconnect", || rig.engine.accepted() >= 2).await;
    assert_eq!(s.link_no(), Some(link), "a reconnect is the same link (F3)");
    until(15, "the load to end", || {
        s.load_phase().is_none() && levels(&rig).len() >= 3
    })
    .await;
    // One dry run, whatever the socket did meanwhile.
    let plays = rig
        .engine
        .kinds(Rig::SESSION)
        .iter()
        .filter(|k| *k == "transport play")
        .count();
    assert_eq!(plays, 1);
    assert_eq!(Stats::get(&rig.link.stats().redials), 0);
}

#[tokio::test]
async fn a_redial_is_a_new_link_and_a_load_waiting_on_the_old_one_is_taken_over() {
    let rig = Rig::new("redial").await;
    rig.engine.loaded_after(Duration::from_secs(3600));
    let s = rig.session();
    s.start(&rig.ws, &rig.project).await;
    until(5, "applying", || {
        s.load_phase() == Some(k7_file_service::engine::Phase::Applying)
    })
    .await;
    let first = s.link_no().unwrap();
    s.drop_socket();
    // Its run learns the engine changed under it and lets the gate wait for the next document.
    until(5, "copying", || {
        s.load_phase() == Some(k7_file_service::engine::Phase::Copying)
    })
    .await;
    s.resend_served().await;
    until(5, "applying again", || {
        s.load_phase() == Some(k7_file_service::engine::Phase::Applying)
    })
    .await;
    assert_ne!(s.link_no(), Some(first));
}

// M11: the poll answers `applying` while the engine has not said the project is loaded, and the dry counter counts it.
#[tokio::test]
async fn the_poll_says_applying_while_the_engine_has_not_loaded_the_project() {
    let rig = Rig::new("applying").await;
    rig.engine.loaded_after(Duration::from_secs(3600));
    let s = rig.session();
    s.start(&rig.ws, &rig.project).await;
    until(5, "applying", || {
        s.load_phase() == Some(k7_file_service::engine::Phase::Applying)
    })
    .await;
    let polled = s.engine_load_of(Some(&rig.project));
    assert_eq!(polled, Some(k7_file_service::engine::Phase::Applying));
    assert_eq!(Stats::get(&rig.link.stats().dry), 1);
}

#[tokio::test]
async fn the_bindings_rules_are_the_hubs() {
    let rig = Rig::new("bind").await;
    let s = rig.session();
    assert!(s.frame(json!({ "type": "flush" })));
    until(5, "a socket", || s.is_ready()).await;
    let link = s.link_no();
    // The same URL with a link open changes nothing of the engine, the jobs path being told anyway.
    rig.link
        .bind(&json!({ "url": rig.engine.url, "jobsUrl": "ws://127.0.0.1:1/j", "engineId": "e1" }));
    assert_eq!(s.link_no(), link);
    assert_eq!(rig.link.jobs_url().as_deref(), Some("ws://127.0.0.1:1/j"));
    // Another URL drops every link, none told `drop-session`.
    let other = FakeEngine::start().await;
    rig.link
        .bind(&json!({ "url": other.url, "engineId": "e2", "engineName": "Two" }));
    assert_eq!(s.link_no(), None);
    assert_eq!(rig.link.binding().engine, Some(("e2".into(), "Two".into())));
    assert!(
        !rig.engine
            .kinds(Rig::SESSION)
            .contains(&"drop-session".to_string())
    );
    // No URL clears it, the jobs path and the identity with it.
    rig.link.bind(&json!({ "url": null }));
    assert_eq!(rig.link.binding(), Default::default());
    assert!(
        !s.frame(json!({ "type": "flush" })),
        "no engine bound: nothing queued"
    );
}

#[tokio::test]
async fn frames_sent_while_the_engine_is_away_are_replayed_oldest_first() {
    let rig = Rig::new("outbox").await;
    let port = rig.engine.port;
    rig.engine.stop();
    let s = rig.link.session(Some(&rig.ws));
    for n in 0..3 {
        s.frame(json!({ "type": "preview", "file": format!("f{n}"), "start": 0, "length": 1 }));
    }
    let back = FakeEngine::on(port).await;
    until(15, "the replay", || back.of(Rig::SESSION).len() >= 3).await;
    let files: Vec<String> = back
        .of(Rig::SESSION)
        .iter()
        .map(|f| f["file"].as_str().unwrap_or("").to_string())
        .collect();
    assert_eq!(files, vec!["f0", "f1", "f2"]);
}

#[tokio::test]
async fn a_refused_upgrade_is_dialled_on_the_backoff_not_in_a_loop() {
    let rig = Rig::new("flap").await;
    rig.engine.refuse(true);
    let s = rig.session();
    s.frame(json!({ "type": "flush" }));
    tokio::time::sleep(Duration::from_secs(5)).await;
    // 500-1000 ms, then 1-2 s, then 2-4 s twice over: four to six dials in five seconds, never hundreds.
    let dials = rig.engine.dials();
    assert!((2..=6).contains(&dials), "{dials} dials");
}

#[tokio::test]
async fn state_frames_are_kept_and_never_forwarded() {
    let rig = Rig::new("state").await;
    let s = rig.session();
    s.frame(json!({ "type": "flush" }));
    until(5, "open", || s.is_ready()).await;
    s.listen(&["state".to_string(), "armed".to_string()]);
    for _ in 0..50 {
        rig.engine
            .send_all(json!({ "type": "state", "playing": true, "recordEnabled": true }));
    }
    rig.engine
        .send_all(json!({ "type": "armed", "track": "t", "armed": true }));
    until(5, "the transport", || s.playing()).await;
    until(5, "the armed frame", || {
        rig.host
            .forwarded
            .load(std::sync::atomic::Ordering::Relaxed)
            == 1
    })
    .await;
}

#[tokio::test]
async fn one_slow_session_does_not_hold_another() {
    let rig = Rig::new("two").await;
    rig.engine.loaded_after(Duration::from_secs(3600));
    let a = rig.session();
    a.start(&rig.ws, &rig.project).await;
    let b = rig.link.session_named("banc:a_b_c");
    assert!(b.frame(json!({ "type": "flush" })));
    until(5, "b's frame", || rig.engine.of("banc:a_b_c").len() == 1).await;
    assert_eq!(
        a.load_phase(),
        Some(k7_file_service::engine::Phase::Applying)
    );
}

// 2262: closing the link logs the counters' line with the final counts, since `Inner` is never dropped.
#[tokio::test]
async fn closing_the_link_logs_the_final_counts() {
    let rig = Rig::new("closeline").await;
    Stats::bump(&rig.link.stats().loads);
    Stats::bump(&rig.link.stats().polls);
    assert!(rig.host.said().is_empty());
    rig.link.close();
    let said = rig.host.said();
    assert_eq!(
        said.last().map(String::as_str),
        Some(rig.link.stats().line("a@b.c").as_str())
    );
    assert!(said[0].contains("loads=1") && said[0].contains("polls=1"));
}
