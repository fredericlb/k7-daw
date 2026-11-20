//! Hostile input to the link: an engine, a socket and a channel that misbehave, each answered without a hang.
//! A bound is a count where one is observable; the wall time is a backstop.

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use k7_file_service::engine::sync::Reply;
use serde_json::json;
use tokio::net::TcpListener;

use super::fake::{FakeJobs, JobEnd, Rig, until};
use super::jobs::sync_within;

const BACKSTOP: u64 = if cfg!(target_env = "musl") { 60 } else { 15 };

#[tokio::test]
async fn another_sessions_frames_and_non_frames_change_nothing() {
    let rig = Rig::new("alien").await;
    let s = rig.session();
    s.frame(json!({ "type": "flush" }));
    until(5, "open", || s.is_ready()).await;
    rig.engine
        .send_all(json!({ "type": "state", "session": "someone-else", "playing": true }));
    rig.engine.send_all(json!("not an object"));
    rig.engine
        .send_all(json!({ "type": "state", "playing": true }));
    until(5, "the real one", || s.playing()).await;
}

#[tokio::test]
async fn ten_thousand_state_frames_reach_nobody_but_the_link() {
    let rig = Rig::new("flood").await;
    let s = rig.session();
    s.frame(json!({ "type": "flush" }));
    until(5, "open", || s.is_ready()).await;
    s.listen(&["state".to_string()]);
    let started = Instant::now();
    for _ in 0..10_000 {
        rig.engine
            .send_all(json!({ "type": "state", "playing": true }));
    }
    until(BACKSTOP, "the flood", || s.playing()).await;
    assert_eq!(rig.host.forwarded.load(Ordering::Relaxed), 0);
    assert!(started.elapsed() < Duration::from_secs(BACKSTOP));
}

// D1: a frame past 16 MiB closes the socket, which is dialled again; the frame is not read and the session goes on.
#[tokio::test]
async fn a_frame_past_sixteen_mebibytes_closes_the_socket_and_the_session_redials() {
    let rig = Rig::new("big").await;
    let s = rig.session();
    s.frame(json!({ "type": "flush" }));
    until(5, "open", || s.is_ready()).await;
    let before = rig.engine.accepted();
    rig.engine
        .send_all(json!({ "type": "x", "pad": "a".repeat(17 << 20) }));
    until(BACKSTOP, "the second connection", || {
        rig.engine.accepted() > before
    })
    .await;
    until(BACKSTOP, "open again", || s.is_ready()).await;
    rig.engine
        .send_all(json!({ "type": "state", "playing": true }));
    until(BACKSTOP, "the next frame", || s.playing()).await;
}

// Just under the limit is read as any frame.
#[tokio::test]
async fn a_frame_just_under_sixteen_mebibytes_is_read() {
    let rig = Rig::new("bigok").await;
    let s = rig.session();
    s.frame(json!({ "type": "flush" }));
    until(5, "open", || s.is_ready()).await;
    rig.engine
        .send_all(json!({ "type": "x", "pad": "a".repeat(15 << 20) }));
    rig.engine
        .send_all(json!({ "type": "state", "playing": true }));
    until(BACKSTOP, "the next frame", || s.playing()).await;
}

#[tokio::test]
async fn an_upgrade_never_answered_holds_nothing_and_closing_ends_it() {
    let silent = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", silent.local_addr().unwrap());
    let accept = tokio::spawn(async move {
        let mut held = vec![];
        while let Ok((tcp, _)) = silent.accept().await {
            held.push(tcp);
        }
    });
    let rig = Rig::new("mute").await;
    rig.link.bind(&json!({ "url": url, "engineId": "e1" }));
    let s = rig.session();
    assert!(
        s.frame(json!({ "type": "flush" })),
        "queued for when it opens"
    );
    assert!(!s.is_ready());
    let started = Instant::now();
    rig.link.close();
    assert!(started.elapsed() < Duration::from_secs(1));
    accept.abort();
}

// The filing is told inline and in order: fifty takes reach the tell as they came, and the reader goes on to the state after them.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn takes_reach_the_tell_in_the_order_they_came_and_the_reader_goes_on() {
    let rig = Rig::new("take").await;
    let s = rig.session();
    s.frame(json!({ "type": "flush" }));
    until(5, "open", || s.is_ready()).await;
    for n in 0..50 {
        rig.engine.send_all(
            json!({ "type": "recorded", "track": format!("t{n}"), "file": "../x.wav", "start": 0, "length": 1 }),
        );
    }
    rig.engine
        .send_all(json!({ "type": "state", "playing": true }));
    until(5, "the state after the takes", || s.playing()).await;
    until(5, "every take", || {
        rig.host.takes.lock().unwrap().len() == 50
    })
    .await;
    let order: Vec<String> = rig
        .host
        .takes
        .lock()
        .unwrap()
        .iter()
        .map(|(_, t)| t.track.clone())
        .collect();
    let want: Vec<String> = (0..50).map(|n| format!("t{n}")).collect();
    assert_eq!(order, want);
}

#[tokio::test]
async fn a_compile_that_panics_is_counted_and_the_queue_goes_on() {
    let rig = Rig::new("panic").await;
    rig.host.panic_compile.store(true, Ordering::Relaxed);
    let s = rig.session();
    s.start(&rig.ws, &rig.project).await;
    until(5, "the count", || {
        rig.link.stats().panics.load(Ordering::Relaxed) == 1
    })
    .await;
    rig.host.panic_compile.store(false, Ordering::Relaxed);
    s.reload(&rig.ws, &rig.project).await;
    until(15, "the next document", || {
        rig.engine
            .kinds(Rig::SESSION)
            .iter()
            .any(|k| k == "project")
    })
    .await;
}

#[tokio::test]
async fn a_sync_never_answered_ends_on_its_bound_and_a_closed_port_is_a_failure() {
    let jobs = FakeJobs::start().await;
    jobs.ends(JobEnd::Never);
    let started = Instant::now();
    let mut seen = 0;
    let r = sync_within(
        &jobs.url,
        vec!["/a".into()],
        &mut |_| seen += 1,
        Duration::from_millis(400),
        Duration::from_secs(60),
    )
    .await;
    assert!(
        matches!(r, Reply::Failed(m) if m.contains("did not finish")),
        "total bound"
    );
    assert!(seen >= 1 && started.elapsed() < Duration::from_secs(BACKSTOP));
    let gone = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", gone.local_addr().unwrap());
    drop(gone);
    let r = sync_within(
        &url,
        vec![],
        &mut |_| {},
        Duration::from_secs(5),
        Duration::from_secs(5),
    )
    .await;
    assert!(matches!(r, Reply::Failed(_)));
}

#[tokio::test]
async fn a_machine_refusing_another_verb_is_a_failure_with_its_words() {
    let jobs = FakeJobs::start().await;
    jobs.ends(JobEnd::Error("this machine does not do 'sync'".into()));
    let r = sync_within(
        &jobs.url,
        vec![],
        &mut |_| {},
        Duration::from_secs(5),
        Duration::from_secs(5),
    )
    .await;
    assert!(matches!(r, Reply::Failed(m) if m.contains("does not do 'sync'")));
}

// An engine that accepts the upgrade and closes at once is dialled on the backoff, not at the base rate (D3).
#[tokio::test]
async fn an_engine_that_opens_and_closes_at_once_is_dialled_on_the_growing_backoff() {
    let rig = Rig::new("slam").await;
    rig.engine.slam(true);
    let s = rig.session();
    s.frame(json!({ "type": "flush" }));
    tokio::time::sleep(Duration::from_secs(8)).await;
    let dials = rig.engine.dials();
    assert!((2..=5).contains(&dials), "{dials} dials");
    assert!(rig.host.compiled.lock().unwrap().len() <= 5);
}

// An engine that accepts and never reads is dropped after the send bound, and what was queued moves to the bounded outbox (D4).
#[tokio::test]
async fn a_stalled_engine_is_dropped_after_the_send_bound_and_the_queue_stops_growing() {
    let rig = Rig::new("stall").await;
    rig.engine.stall(true);
    let s = rig.session();
    for _ in 0..64 {
        s.frame(json!({ "type": "preview", "file": "y".repeat(1 << 20), "start": 0, "length": 1 }));
    }
    until(10, "a second dial", || rig.engine.dials() >= 2).await;
    let queued =
        s.st.lock()
            .unwrap()
            .socket
            .as_ref()
            .map_or(0, |k| k.backlog());
    assert_eq!(
        queued, 0,
        "{queued} frames still queued behind the stalled writer"
    );
}

// A hundred thousand rows or entries are read, and the session goes on to the next frame.
#[tokio::test]
async fn huge_sessions_and_plugin_listings_are_read_and_the_session_goes_on() {
    let rig = Rig::new("huge").await;
    let s = rig.session();
    s.frame(json!({ "type": "flush" }));
    until(5, "open", || s.is_ready()).await;
    let rows: Vec<serde_json::Value> = (0..100_000)
        .map(|n| json!({ "id": format!("s{n}"), "loaded": true }))
        .collect();
    rig.engine
        .send_all(json!({ "type": "sessions", "sessions": rows }));
    let plugins: Vec<serde_json::Value> = (0..100_000)
        .map(|n| json!({ "name": format!("p{n}") }))
        .collect();
    rig.engine
        .send_all(json!({ "type": "plugins", "plugins": plugins }));
    rig.engine
        .send_all(json!({ "type": "state", "playing": true }));
    until(BACKSTOP, "the next frame", || s.playing()).await;
}

// An `armed` for a track nobody asked about is forwarded as is, and a take with no track reaches the tell.
#[tokio::test]
async fn an_armed_for_another_track_and_a_take_with_no_track_are_told_as_they_came() {
    let rig = Rig::new("odd").await;
    let s = rig.session();
    s.frame(json!({ "type": "flush" }));
    until(5, "open", || s.is_ready()).await;
    s.listen(&["armed".to_string()]);
    rig.engine
        .send_all(json!({ "type": "armed", "track": "elsewhere", "armed": true }));
    rig.engine
        .send_all(json!({ "type": "recorded", "file": "a.wav" }));
    until(5, "both told", || {
        rig.host.forwarded.load(Ordering::Relaxed) == 1 && rig.host.takes.lock().unwrap().len() == 1
    })
    .await;
    assert_eq!(rig.host.takes.lock().unwrap()[0].1.track, "");
}
