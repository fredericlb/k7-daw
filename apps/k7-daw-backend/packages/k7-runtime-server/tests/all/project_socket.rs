//! project_socket.rs — the project and transport verbs over a real socket and the fake engine: the replies byte for
//! byte, what the engine was asked and in what order, a superseded document skipped, a dropped keyboard released.

use crate::harness::{Server, Wire, eventually};
use k7_runtime_server::engine::fake;
use k7_runtime_server::server::ServeConfig;
use serde_json::json;

fn start() -> Server {
    Server::start(ServeConfig::new(0))
}

fn calls(s: &Server) -> Vec<String> {
    fake::with_state(&s.engine, |st| st.calls.clone()).unwrap()
}

async fn saw(s: &Server, line: &str) {
    eventually(line, || calls(s).iter().any(|c| c == line)).await;
}

const PROJECT: &str = r#"{"type":"project","session":"k7:a","doc":{"name":"song","tracks":[{"id":"t1"},{"id":"t2"}]}}"#;

#[tokio::test]
async fn a_project_is_applied_between_busy_frames_and_answers_what_the_engine_changed() {
    let s = start();
    fake::with_state(&s.engine, |st| {
        st.project.changes = vec!["add track t1".into()]
    });
    fake::project::set_standing("k7:a", &[("chain:fx", "slow")]);
    let mut w = s.connect().await;
    w.send(PROJECT).await;
    assert_eq!(
        w.text().await,
        r#"{"busy":true,"session":"k7:a","type":"busy"}"#
    );
    assert_eq!(
        w.text().await,
        r#"{"changes":["add track t1"],"session":"k7:a","type":"applied","warnings":[{"message":"slow","subject":"chain:fx"}]}"#
    );
    assert_eq!(
        w.text().await,
        r#"{"busy":false,"session":"k7:a","type":"busy"}"#
    );
    let doc = fake::with_state(&s.engine, |st| st.project.docs["k7:a"].clone()).unwrap();
    assert_eq!(doc["tracks"][1]["name"], "t2");
    assert_eq!(doc["sampleRate"], 48000);
}

#[tokio::test]
async fn a_document_that_cannot_be_read_is_a_bad_project_to_that_socket_alone() {
    let s = start();
    let mut w = s.connect().await;
    let reply = w.ask(r#"{"type":"project","doc":{"tempo":"fast"}}"#).await;
    assert_eq!(
        reply,
        r#"{"message":"bad project: [json.exception.type_error.302] type must be number, but is string","type":"error"}"#
    );
    let reply = w.ask(r#"{"type":"project"}"#).await;
    assert_eq!(
        reply,
        r#"{"message":"bad project: [json.exception.out_of_range.403] key 'doc' not found","type":"error"}"#
    );
    let reply = w.ask(r#"{"type":"tracks","tracks":5}"#).await;
    assert!(
        reply.contains("bad tracks: [json.exception.type_error.302]"),
        "{reply}"
    );
}

#[tokio::test]
async fn a_failed_apply_is_told_and_busy_ends() {
    let s = start();
    fake::with_state(&s.engine, |st| {
        st.project.apply_error = Some("no plugin".into())
    });
    let mut w = s.connect().await;
    w.send(PROJECT).await;
    assert_eq!(
        w.text().await,
        r#"{"busy":true,"session":"k7:a","type":"busy"}"#
    );
    assert_eq!(
        w.text().await,
        r#"{"message":"apply failed: no plugin","session":"k7:a","type":"error"}"#
    );
    assert_eq!(
        w.text().await,
        r#"{"busy":false,"session":"k7:a","type":"busy"}"#
    );
}

#[tokio::test]
async fn a_project_opened_takes_the_slot_over_unless_it_is_the_slot() {
    let s = start();
    fake::with_state(&s.engine, |st| st.project.slot_loaded = true);
    let mut w = s.connect().await;
    w.send(r#"{"type":"project","session":"slot","doc":{"footswitch":{"cc":4,"plugins":["fx"]}}}"#)
        .await;
    for _ in 0..3 {
        w.text().await;
    }
    assert!(!calls(&s).contains(&"release_slot".to_string()));
    let slot = fake::with_state(&s.engine, |st| {
        st.project.docs["slot"]["footswitch"].clone()
    })
    .unwrap();
    assert_eq!(slot, json!({"cc": 4, "plugins": ["fx"]}));
    w.send(PROJECT).await;
    for _ in 0..3 {
        w.text().await;
    }
    assert!(calls(&s).contains(&"release_slot".to_string()));
}

#[tokio::test]
async fn tracks_replace_by_id_in_the_held_document_and_say_nothing() {
    let s = start();
    let mut w = s.connect().await;
    let reply = w
        .ask(r#"{"type":"tracks","session":"k7:a","tracks":[{"id":"t1"}]}"#)
        .await;
    assert_eq!(
        reply,
        r#"{"message":"tracks: this session holds no project","session":"k7:a","type":"error"}"#
    );
    w.send(PROJECT).await;
    for _ in 0..3 {
        w.text().await;
    }
    w.send(r#"{"type":"tracks","session":"k7:a","tracks":[{"id":"t2","name":"Keys","clips":[{"id":"c","notes":[{"key":999}]}]},{"id":"ghost"}]}"#).await;
    saw(&s, "apply_tracks k7:a").await;
    let doc = fake::with_state(&s.engine, |st| st.project.docs["k7:a"].clone()).unwrap();
    assert_eq!(doc["tracks"][0]["name"], "t1");
    assert_eq!(doc["tracks"][1]["name"], "Keys");
    assert_eq!(doc["tracks"][1]["clips"][0]["notes"][0]["key"], 127);
    assert_eq!(doc["tracks"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn only_the_last_of_the_documents_queued_behind_a_busy_loop_is_applied() {
    let mut s = Server::held(ServeConfig::new(0));
    let mut w = s.connect().await;
    for name in ["one", "two", "three"] {
        w.send(&format!(
            r#"{{"type":"project","session":"k7:a","doc":{{"name":"{name}"}}}}"#
        ))
        .await;
    }
    // A transport move is a fence: the document after it is not swallowed by the ones before.
    w.send(r#"{"type":"transport","session":"k7:a","action":"play"}"#)
        .await;
    w.send(r#"{"type":"project","session":"k7:a","doc":{"name":"four"}}"#)
        .await;
    // Everything is parsed and queued once the fence's own post is seen by a later whoami round trip.
    assert!(w.ask(r#"{"type":"whoami"}"#).await.contains("whoami"));
    s.release();
    saw(&s, "play k7:a").await;
    eventually("the last document", || {
        calls(&s).iter().filter(|c| *c == "apply k7:a").count() == 2
    })
    .await;
    let name = fake::with_state(&s.engine, |st| st.project.docs["k7:a"]["name"].clone()).unwrap();
    assert_eq!(name, "four");
    drain(&mut w, 4).await;
}

/// Reads and drops `n` frames.
async fn drain(w: &mut Wire, n: usize) {
    for _ in 0..n {
        w.text().await;
    }
}

#[tokio::test]
async fn flush_forgets_the_project_and_is_answered() {
    let s = start();
    let mut w = s.connect().await;
    w.send(PROJECT).await;
    drain(&mut w, 3).await;
    assert_eq!(
        w.ask(r#"{"type":"flush","session":"k7:a"}"#).await,
        r#"{"session":"k7:a","type":"flushed"}"#
    );
    assert!(fake::with_state(&s.engine, |st| st.project.docs.is_empty()).unwrap());
}

#[tokio::test]
async fn the_transport_verbs_reach_the_engine_in_order_and_toggle_from_its_state() {
    let s = start();
    let mut w = s.connect().await;
    for msg in [
        r#"{"type":"transport","session":"k7:a","action":"play"}"#,
        r#"{"type":"transport","session":"k7:a","action":"toggle"}"#,
        r#"{"type":"transport","session":"k7:a","action":"toggle"}"#,
        r#"{"type":"transport","session":"k7:a","action":"seek","position":2.5}"#,
        r#"{"type":"transport","session":"k7:a","action":"loop","start":1,"end":3,"enabled":true}"#,
        r#"{"type":"transport","session":"k7:a","action":"rewind"}"#,
        r#"{"type":"transport","session":"k7:a","action":"panic"}"#,
        r#"{"type":"transport","session":"k7:a","action":"nonsense"}"#,
        r#"{"type":"record-enable","session":"k7:a"}"#,
        r#"{"type":"note-preview","session":"k7:a","track":"t","key":60,"vel":0.5}"#,
        r#"{"type":"note-preview","session":"k7:a","track":"t","key":128}"#,
    ] {
        w.send(msg).await;
    }
    saw(&s, "preview_note k7:a t 60 0.5").await;
    let all: Vec<String> = calls(&s)
        .into_iter()
        .filter(|c| !c.starts_with("is_playing") && c != "open_audio_device")
        .collect();
    assert_eq!(
        all,
        [
            "play k7:a",
            "stop k7:a",
            "play k7:a",
            "seek k7:a 2.5",
            "set_loop k7:a 1 3 1",
            "rewind k7:a",
            "panic k7:a",
            "set_record_enabled k7:a -1",
            "preview_note k7:a t 60 0.5",
        ]
    );
    assert!(
        fake::with_state(&s.engine, |st| st.transport.record_enabled.contains("k7:a")).unwrap()
    );
}

#[tokio::test]
async fn a_malformed_transport_is_answered_and_nothing_reaches_the_engine() {
    let s = start();
    let mut w = s.connect().await;
    let reply = w
        .ask(r#"{"type":"transport","action":"seek","position":"soon"}"#)
        .await;
    assert_eq!(
        reply,
        r#"{"message":"malformed message: [json.exception.type_error.302] type must be number, but is string","type":"error"}"#
    );
    assert!(!calls(&s).iter().any(|c| c.starts_with("seek")));
}

#[tokio::test]
async fn a_preview_is_held_to_the_read_roots() {
    let s = start();
    let mut w = s.connect().await;
    w.send(r#"{"type":"preview","session":"k7:a","file":"//host/share/a.wav"}"#)
        .await;
    w.send(r#"{"type":"preview","session":"k7:a","file":"/tmp/a.wav","start":1,"length":2}"#)
        .await;
    saw(&s, "preview_sample k7:a /tmp/a.wav 1 2").await;
    assert_eq!(
        calls(&s)
            .iter()
            .filter(|c| c.starts_with("preview_sample"))
            .count(),
        1
    );
}

#[tokio::test]
async fn arming_answers_which_way_it_went_and_a_refusal_is_an_error() {
    let s = start();
    let mut w = s.connect().await;
    let arm = r#"{"type":"arm","session":"k7:a","track":"gtr"}"#;
    assert_eq!(
        w.ask(arm).await,
        r#"{"armed":true,"session":"k7:a","track":"gtr","type":"armed"}"#
    );
    assert_eq!(
        w.ask(arm).await,
        r#"{"armed":false,"session":"k7:a","track":"gtr","type":"armed"}"#
    );
    fake::with_state(&s.engine, |st| {
        st.transport.arm_error = Some("no input".into())
    });
    assert_eq!(
        w.ask(arm).await,
        r#"{"message":"no input","session":"k7:a","type":"error"}"#
    );
    fake::with_state(&s.engine, |st| {
        st.transport.recorder_error = Some("no recordDir".into())
    });
    let rec = r#"{"type":"recorder","session":"k7:a","node":"tape","action":"record"}"#;
    assert_eq!(
        w.ask(rec).await,
        r#"{"message":"no recordDir","session":"k7:a","type":"error"}"#
    );
    fake::with_state(&s.engine, |st| st.transport.recorder_error = None);
    w.send(rec).await;
    w.send(r#"{"type":"recorder","session":"k7:a","action":"clear"}"#)
        .await;
    w.send(r#"{"type":"recorder","session":"k7:a","node":"tape","action":"clear"}"#)
        .await;
    saw(&s, "recorder k7:a tape 1 -1").await;
    assert!(
        fake::with_state(&s.engine, |st| st
            .transport
            .recording
            .contains(&("k7:a".into(), "tape".into())))
        .unwrap()
    );
}

#[tokio::test]
async fn a_keyboard_that_drops_mid_note_has_its_notes_released() {
    let s = start();
    let mut w = s.connect().await;
    w.send(r#"{"type":"midi-live","session":"k7:a","track":"keys","data":[144,60,100]}"#)
        .await;
    w.send(r#"{"type":"midi-live","session":"k7:a","track":"keys","data":[176,64,127]}"#)
        .await;
    w.send(r#"{"type":"midi-live","session":"k7:a","track":"keys","data":[240,1]}"#)
        .await;
    saw(&s, "live_midi k7:a keys [176, 64, 127]").await;
    drop(w);
    saw(&s, "live_midi k7:a keys [128, 60, 0]").await;
    saw(&s, "live_midi k7:a keys [176, 64, 0]").await;
}
