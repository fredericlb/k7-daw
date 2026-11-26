//! server_state.rs — what the engine says unasked, over a real socket and the fake engine: the state frame
//! byte for byte against the fixture, to the sockets of its session only and droppable, a take, a
//! plugin's parameters (persisted and live), an engine warning, and the verb that watches a plugin.

use crate::harness::{Server, Wire, eventually};
use k7_runtime_server::engine::fake::{self, with_state};
use k7_runtime_server::engine::fake_state::{self, FrameSpec, RecorderSpec};
use k7_runtime_server::engine::state::{ParamInfo, Recorded};
use k7_runtime_server::server::ServeConfig;
use serde_json::{Value, json};
use std::time::Duration;

/// A fixture line's snapshot as the fake engine's frame.
fn spec(input: &Value) -> FrameSpec {
    let mut s = FrameSpec::idle("");
    let f = |v: &Value| v.as_f64().unwrap() as f32;
    let list = |v: &Value| -> Vec<f32> {
        v.as_array()
            .map(|a| a.iter().map(f).collect())
            .unwrap_or_default()
    };
    let points = |v: &Value| -> Vec<(f32, f32)> {
        v.as_array()
            .unwrap()
            .iter()
            .map(|p| (f(&p[0]), f(&p[1])))
            .collect()
    };
    s.session = input["session"].as_str().unwrap_or("").to_string();
    s.playing = input["playing"].as_bool().unwrap_or(false);
    s.playhead = input["playhead"].as_f64().unwrap_or(0.0);
    if let Some(l) = input.get("loop") {
        s.loop_on = l[0].as_bool().unwrap();
        s.loop_start = l[1].as_f64().unwrap();
        s.loop_end = l[2].as_f64().unwrap();
    }
    s.levels = list(&input["levels"]);
    s.input_levels = list(&input["inputLevels"]);
    s.armed = input["armed"]
        .as_array()
        .map(|a| a.iter().map(|v| v.as_str().unwrap().to_string()).collect())
        .unwrap_or_default();
    s.record_enabled = input["recordEnabled"].as_bool().unwrap_or(false);
    s.master_db = input.get("master").map_or(-100.0, f);
    s.master_clipped = input["clipped"].as_bool().unwrap_or(false);
    if let Some(w) = input["waveform"].as_object() {
        s.waveforms = w.iter().map(|(k, v)| (k.clone(), points(v))).collect();
    }
    if let Some(p) = input["platters"].as_object() {
        s.platters = p
            .iter()
            .map(|(k, v)| (k.clone(), v.as_f64().unwrap()))
            .collect();
    }
    if let Some(rs) = input["recorders"].as_object() {
        s.recorders = rs
            .iter()
            .map(|(id, r)| RecorderSpec {
                id: id.clone(),
                recording: r["recording"].as_bool().unwrap_or(false),
                seconds: r["seconds"].as_f64().unwrap_or(0.0),
                takes: r["takes"]
                    .as_array()
                    .map(|t| {
                        t.iter()
                            .map(|t| {
                                (
                                    t[0].as_str().unwrap().to_string(),
                                    t[1].as_f64().unwrap(),
                                    t[2].as_bool().unwrap(),
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                envelope: r.get("envelope").map(points).unwrap_or_default(),
                envelope_from: r["from"].as_u64().unwrap_or(0),
            })
            .collect();
    }
    if let Some(load) = input.get("load") {
        let plugins = load["plugins"]
            .as_object()
            .map(|p| {
                p.iter()
                    .map(|(k, v)| (k.clone(), v[0].as_f64().unwrap(), v[1].as_i64().unwrap()))
                    .collect()
            })
            .unwrap_or_default();
        s.load = Some((plugins, load["process"].as_i64().unwrap_or(-1)));
    }
    s
}

/// A server whose state timer the fake engine has started.
async fn started() -> Server {
    let s = Server::start(ServeConfig::new(0));
    eventually("the state timer starts", || fake_state::started(&s.engine)).await;
    s
}

/// A socket that has said its first real message about `session`, and is bound to it.
async fn subscribed(s: &Server, session: &str) -> Wire {
    let mut w = s.connect().await;
    w.send(&json!({"type": "watch-plugin-params", "session": session}).to_string())
        .await;
    eventually("the socket is bound", || {
        !s.ctx.state_targets(session).is_empty()
    })
    .await;
    w
}

/// The next frame within `ms`, or `None`: for what must NOT arrive.
async fn quiet(w: &mut Wire, ms: u64) -> Option<String> {
    use futures_util::StreamExt;
    use tokio_tungstenite::tungstenite::Message;
    match tokio::time::timeout(Duration::from_millis(ms), w.0.next()).await {
        Ok(Some(Ok(Message::Text(t)))) => Some(t.as_str().to_string()),
        _ => None,
    }
}

fn calls(s: &Server) -> Vec<String> {
    with_state(&s.engine, |st| st.calls.clone()).unwrap()
}

#[tokio::test]
async fn every_fixture_frame_is_the_cpps_bytes() {
    let fixture =
        crate::repo_file("apps/k7-daw-backend/packages/k7-runtime-server/fixtures/state_frame.txt");
    let s = started().await;
    let mut cases = 0;
    for line in fixture.lines() {
        let (input, expected) = line.split_once('\t').unwrap();
        let spec = spec(&serde_json::from_str(input).unwrap());
        let mut w = subscribed(&s, &spec.session).await;
        assert!(fake_state::tick(&s.engine, spec));
        assert_eq!(w.text().await, expected);
        cases += 1;
        // The next case binds its own session: this socket's is closed with it.
        drop(w);
        eventually("the socket is forgotten", || s.ctx.client_count() == 0).await;
    }
    assert!(cases >= 5);
}

#[tokio::test]
async fn a_frame_reaches_only_the_sockets_that_asked_about_its_session() {
    let s = started().await;
    let mut a1 = subscribed(&s, "a").await;
    let mut a2 = subscribed(&s, "a").await;
    let mut b = subscribed(&s, "b").await;
    fake_state::tick(&s.engine, FrameSpec::idle("a"));
    let frame = a1.text().await;
    assert!(frame.contains("\"session\":\"a\""));
    assert_eq!(a2.text().await, frame);
    assert_eq!(quiet(&mut b, 150).await, None);
    // A session nobody asked about builds and sends nothing.
    fake_state::tick(&s.engine, FrameSpec::idle("nobody"));
    fake_state::tick(&s.engine, FrameSpec::idle("b"));
    assert!(b.text().await.contains("\"session\":\"b\""));
    assert_eq!(quiet(&mut a1, 100).await, None);
}

#[tokio::test]
async fn a_probe_is_bound_to_nothing_and_a_socket_binds_once() {
    let s = started().await;
    let mut probe = s.connect().await;
    probe.ask(r#"{"type":"whoami","session":"a"}"#).await;
    let mut w = subscribed(&s, "a").await;
    // A later message naming another session does not rebind it.
    w.send(r#"{"type":"watch-plugin-params","session":"b"}"#)
        .await;
    fake_state::tick(&s.engine, FrameSpec::idle("a"));
    fake_state::tick(&s.engine, FrameSpec::idle("b"));
    assert!(w.text().await.contains("\"session\":\"a\""));
    assert_eq!(quiet(&mut w, 150).await, None);
    assert_eq!(quiet(&mut probe, 50).await, None);
}

#[tokio::test]
async fn a_state_frame_is_dropped_behind_a_stuck_socket_and_a_reply_is_kept() {
    let s = started().await;
    let stuck = subscribed(&s, "big").await;
    let client = s.ctx.state_targets("big").remove(0);
    // About a megabyte of text a frame, to a peer that never reads: the writer stalls and a backlog builds.
    let mut big = FrameSpec::idle("big");
    big.levels = vec![-12.5; 150_000];
    let mut ticks = 0;
    while client.outbox.queued_bytes() <= 64 * 1024 {
        assert!(ticks < 200, "the backlog never built");
        fake_state::tick(&s.engine, big.clone());
        s.engine.call(|_| ()).await;
        tokio::time::sleep(Duration::from_millis(5)).await;
        ticks += 1;
    }
    let behind = client.outbox.queued_bytes();
    for _ in 0..5 {
        fake_state::tick(&s.engine, big.clone());
        s.engine.call(|_| ()).await;
    }
    assert!(
        client.outbox.queued_bytes() <= behind,
        "a frame was queued behind the backlog"
    );
    // A reply is kept, whatever the backlog.
    fake_state::recorded(
        &s.engine,
        Recorded {
            session: "big".into(),
            track: "t".into(),
            file: "f.wav".into(),
            start: 0.0,
            length: 1.0,
        },
    );
    s.engine.call(|_| ()).await;
    assert!(client.outbox.queued_bytes() > behind);
    drop(stuck);
}

#[tokio::test]
async fn a_take_is_told_to_every_trusted_socket_stamped_with_its_session() {
    let s = started().await;
    let mut w = s.connect().await;
    // Not subscribed to anything: a take is a reply to a project, not a state frame.
    w.ask(r#"{"type":"whoami"}"#).await;
    let at = |ctx: &str| ctx.to_string();
    fake_state::recorded(
        &s.engine,
        Recorded {
            session: at("k7:song"),
            track: "t1".into(),
            file: "/rec/take-1.wav".into(),
            start: 1.5,
            length: 0.25,
        },
    );
    assert_eq!(
        w.text().await,
        r#"{"file":"/rec/take-1.wav","length":0.25,"session":"k7:song","start":1.5,"track":"t1","type":"recorded"}"#
    );
}

#[tokio::test]
async fn plugin_params_persist_as_text_and_live_ones_are_their_own_type() {
    let s = started().await;
    let mut w = s.connect().await;
    w.ask(r#"{"type":"whoami"}"#).await;
    fake_state::plugin_params(&s.engine, "s", "p1", r#"{"b":2,"a":[1,2.5]}"#);
    assert_eq!(
        w.text().await,
        r#"{"id":"p1","params":{"a":[1,2.5],"b":2},"session":"s","type":"plugin-params"}"#
    );
    // Text that is not JSON is an empty object, the parse error swallowed.
    fake_state::plugin_params(&s.engine, "s", "p1", "not json");
    assert_eq!(
        w.text().await,
        r#"{"id":"p1","params":{},"session":"s","type":"plugin-params"}"#
    );
    fake_state::plugin_params_live(
        &s.engine,
        "s",
        "p1",
        vec![("Cutoff".into(), 0.5), ("Res".into(), 1.0)],
    );
    assert_eq!(
        w.text().await,
        r#"{"id":"p1","params":{"Cutoff":0.5,"Res":1.0},"session":"s","type":"plugin-params-live"}"#
    );
}

#[tokio::test]
async fn a_warning_is_stamped_when_it_names_a_project_and_not_otherwise() {
    let s = started().await;
    let mut w = s.connect().await;
    w.ask(r#"{"type":"whoami"}"#).await;
    // The sink is the process's: another test's server may have taken it, so it is claimed again before each try.
    let mut say = async |session: &str, subject: &str, message: &str| {
        for _ in 0..50 {
            k7_runtime_server::server::state::install_warnings(&s.ctx);
            fake::warn(session, subject, message);
            if let Some(text) = quiet(&mut w, 200).await {
                return text;
            }
        }
        panic!("no warning arrived");
    };
    assert_eq!(
        say("k7:song", "chain:mod", "a plugin is gone").await,
        r#"{"message":"a plugin is gone","session":"k7:song","subject":"chain:mod","type":"engine-warning"}"#
    );
    assert_eq!(
        say("", "audio", "no device").await,
        r#"{"message":"no device","subject":"audio","type":"engine-warning"}"#
    );
}

#[tokio::test]
async fn watch_plugin_params_reaches_the_engine_with_its_session_and_switch() {
    let s = started().await;
    let mut w = s.connect().await;
    let watched = |on: i32| format!("watch_plugin_params k7:song p1 {on}");
    w.send(r#"{"type":"watch-plugin-params","session":"k7:song","id":"p1"}"#)
        .await;
    w.send(r#"{"type":"watch-plugin-params","session":"k7:song","id":"p1","on":false}"#)
        .await;
    // No id is nothing to watch.
    w.send(r#"{"type":"watch-plugin-params","session":"k7:song"}"#)
        .await;
    eventually("both reach the engine", || calls(&s).contains(&watched(0))).await;
    let watches: Vec<String> = calls(&s)
        .into_iter()
        .filter(|c| c.starts_with("watch_plugin_params"))
        .collect();
    assert_eq!(watches, vec![watched(1), watched(0)]);
    // A field of the wrong type is a malformed message, and the socket stays.
    let reply = w.ask(r#"{"type":"watch-plugin-params","id":5}"#).await;
    assert!(
        reply.starts_with(r#"{"message":"malformed message: "#),
        "{reply}"
    );
}

#[tokio::test]
async fn the_timer_starts_at_the_cpp_rate_and_a_new_subscriber_rewinds_the_recorders() {
    let s = started().await;
    assert!(calls(&s).contains(&"state_start 50".to_string()));
    let rewinds = |s: &Server| {
        calls(s)
            .into_iter()
            .filter(|c| c.starts_with("rewind_recorder_envelopes"))
            .collect::<Vec<_>>()
    };
    let mut w = s.connect().await;
    // A probe is no subscriber.
    w.ask(r#"{"type":"whoami","session":"x"}"#).await;
    w.send(r#"{"type":"watch-plugin-params","session":"x"}"#)
        .await;
    w.send(r#"{"type":"watch-plugin-params","session":"y"}"#)
        .await;
    eventually("the rewind is posted", || !rewinds(&s).is_empty()).await;
    s.engine.call(|_| ()).await;
    assert_eq!(rewinds(&s), vec!["rewind_recorder_envelopes x".to_string()]);
}

#[tokio::test]
async fn a_plugins_parameters_come_back_with_their_ranges_and_settings() {
    let s = started().await;
    let info = ParamInfo {
        name: "Mode".into(),
        value: 1.0,
        min: 0.0,
        max: 2.0,
        default: Some(0.0),
        states: vec![("LP".into(), 0.0), ("HP".into(), 1.0)],
    };
    let plain = ParamInfo {
        name: "Gain".into(),
        value: 0.5,
        min: -1.0,
        max: 1.0,
        default: None,
        states: Vec::new(),
    };
    fake_state::set_param_infos(&s.engine, "s", "p1", vec![info.clone(), plain.clone()]);
    let got = s
        .engine
        .call(|mt| {
            (
                mt.plugin_param_infos("s", "p1"),
                mt.plugin_param_infos("s", "none"),
            )
        })
        .await
        .unwrap();
    assert_eq!(got.0, vec![info, plain]);
    assert!(got.1.is_empty());
}
