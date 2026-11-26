//! server_socket.rs — the server over a real socket on 127.0.0.1 and the fake engine: what a client is sent for
//! each message, byte for byte, the screens, the sessions' ceilings, the subscription and the startup order.

use crate::harness::{Server, Wire, eventually};
use k7_runtime_server::engine::fake;
use k7_runtime_server::engine::ffi::K7AudioTiming;
use k7_runtime_server::server::ServeConfig;
use serde_json::json;

fn cfg() -> ServeConfig {
    let mut c = ServeConfig::new(0);
    c.engine_id = "e1".into();
    c.engine_token = "t0k".into();
    c
}

/// The `error` reply as the wire prints it: sorted keys, compact.
fn err(message: &str) -> String {
    format!(
        "{{\"message\":{},\"type\":\"error\"}}",
        serde_json::to_string(message).unwrap()
    )
}

/// A whoami whose field `x` nests arrays so the whole message is `levels` deep.
fn nested(levels: usize) -> String {
    format!(
        "{{\"type\":\"whoami\",\"x\":{}{}}}",
        "[".repeat(levels - 1),
        "]".repeat(levels - 1)
    )
}

const WHOAMI: &str = r#"{"type":"whoami"}"#;

#[tokio::test]
async fn whoami_is_answered_byte_for_byte_with_the_message_loop_not_yet_running() {
    let mut s = Server::held(cfg());
    fake::with_state(&s.engine, |st| {
        st.timing = K7AudioTiming {
            buffer_frames: 512,
            sample_rate: 48000.0,
        }
    });
    let mut w = s.connect().await;
    let reply = w.ask(WHOAMI).await;
    let boot = serde_json::from_str::<serde_json::Value>(&reply).unwrap()["boot"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        reply,
        format!(
            "{{\"audioToBrowser\":false,\"boot\":\"{boot}\",\"bufferSize\":512,\"build\":\"{}\",\"id\":\"e1\",\"messageThreadMs\":0.0,\"protocol\":1,\"sampleRate\":48000.0,\"token\":\"t0k\",\"type\":\"whoami\"}}",
            k7_runtime_server::wire::version::build()
        )
    );
    // The port answered before the device opened, which waits for the loop.
    let opened = |s: &Server| {
        fake::with_state(&s.engine, |st| {
            st.calls.contains(&"open_audio_device".into())
        })
        .unwrap()
    };
    assert!(!opened(&s));
    s.release();
    eventually("the device opens", || opened(&s)).await;
    assert_eq!(s.stop(), 0);
}

#[tokio::test]
async fn hello_answers_alike_and_counts_a_driver_once() {
    let s = Server::start(cfg());
    let mut w = s.connect().await;
    let hello = r#"{"type":"hello","role":"driver","session":"k7:fred"}"#;
    assert!(w.ask(hello).await.contains("\"type\":\"whoami\""));
    w.ask(hello).await;
    w.ask(r#"{"type":"hello","role":"driver","session":"other"}"#)
        .await;
    assert_eq!(s.ctx.bindings().drivers.len(), 1);
    assert_eq!(s.ctx.bindings().drivers.values().next().unwrap(), "k7:fred");
    drop(w);
    eventually("the driver leaves with its socket", || {
        s.ctx.bindings().drivers.is_empty() && s.ctx.client_count() == 0
    })
    .await;
}

#[tokio::test]
async fn safe_mode_is_said_on_whoami() {
    let mut c = cfg();
    c.safe_mode_crashes = 3;
    let s = Server::start(c);
    let mut w = s.connect().await;
    let j = w.ask(WHOAMI).await;
    assert!(
        j.contains("\"safeMode\":{\"crashes\":3,\"lastPlugin\":\"Crashy.vst3\"}"),
        "{j}"
    );
}

#[tokio::test]
async fn a_malformed_message_is_answered_and_the_socket_stays() {
    let s = Server::start(cfg());
    let mut w = s.connect().await;
    let answers = [
        (
            "[]",
            "[json.exception.type_error.306] cannot use value() with array",
        ),
        (
            "null",
            "[json.exception.type_error.306] cannot use value() with null",
        ),
        (
            "\"hi\"",
            "[json.exception.type_error.306] cannot use value() with string",
        ),
        (
            r#"{"type":5}"#,
            "[json.exception.type_error.302] type must be string, but is number",
        ),
        (
            r#"{"type":"whoami","role":5}"#,
            "[json.exception.type_error.302] type must be string, but is number",
        ),
        (
            r#"{"type":"nope","session":false}"#,
            "[json.exception.type_error.302] type must be string, but is boolean",
        ),
    ];
    for (text, what) in answers {
        assert_eq!(
            w.ask(text).await,
            err(&format!("malformed message: {what}")),
            "{text}"
        );
    }
    // A parse failure is its own answer, and the socket still works after all of them.
    assert!(w.ask("{").await.starts_with("{\"message\":\"bad json: "));
    assert!(w.ask(WHOAMI).await.contains("\"id\":\"e1\""));
}

#[tokio::test]
async fn nesting_is_refused_past_64_levels_and_a_deep_one_is_not_walked() {
    let s = Server::start(cfg());
    let mut w = s.connect().await;
    assert!(w.ask(&nested(64)).await.contains("\"type\":\"whoami\""));
    assert_eq!(
        w.ask(&nested(65)).await,
        err("bad json: message nested deeper than 64 levels")
    );
    assert_eq!(
        w.ask(&nested(100_000)).await,
        err("bad json: message nested deeper than 64 levels")
    );
    assert!(w.ask(WHOAMI).await.contains("\"type\":\"whoami\""));
}

#[tokio::test]
async fn an_unknown_type_is_answered_by_name() {
    let s = Server::start(cfg());
    let mut w = s.connect().await;
    assert_eq!(
        w.ask(r#"{"type":"no-such-verb"}"#).await,
        err("unknown message type: no-such-verb")
    );
    assert_eq!(w.ask("{}").await, err("unknown message type: "));
}

#[tokio::test]
async fn a_new_session_past_the_ceiling_is_refused_and_an_existing_one_answered() {
    let s = Server::start(cfg());
    fake::with_state(&s.engine, |st| {
        st.sessions = (0..64).map(|i| format!("s{i}")).collect();
    });
    let mut w = s.connect().await;
    let long = "x".repeat(257);
    assert_eq!(
        w.ask(&format!(r#"{{"type":"nope","session":"{long}"}}"#))
            .await,
        err("session id too long")
    );
    assert_eq!(
        w.ask(r#"{"type":"nope","session":"fresh"}"#).await,
        err("this engine holds too many sessions")
    );
    assert_eq!(
        w.ask(r#"{"type":"nope","session":"s3"}"#).await,
        err("unknown message type: nope")
    );
    // Asking never creates: a probe on a new session is answered, a driver's hello is not.
    assert!(
        w.ask(r#"{"type":"whoami","session":"fresh"}"#)
            .await
            .contains("\"type\":\"whoami\"")
    );
    assert_eq!(
        w.ask(r#"{"type":"hello","role":"driver","session":"fresh"}"#)
            .await,
        err("this engine holds too many sessions")
    );
    // The default session is no new one.
    assert_eq!(
        w.ask(r#"{"type":"nope"}"#).await,
        err("unknown message type: nope")
    );
    fake::with_state(&s.engine, |st| st.sessions.truncate(63));
    assert_eq!(
        w.ask(r#"{"type":"nope","session":"fresh"}"#).await,
        err("unknown message type: nope")
    );
}

#[tokio::test]
async fn the_first_real_message_binds_the_sockets_session_once() {
    let s = Server::start(cfg());
    let mut probe = s.connect().await;
    probe.ask(r#"{"type":"whoami","session":"p"}"#).await;
    probe.ask(r#"{"type":"list-sessions"}"#).await;
    let mut over = s.connect().await;
    over.ask(r#"{"type":"render","session":"elsewhere"}"#).await;
    let mut real = s.connect().await;
    real.ask(r#"{"type":"nope","session":"a"}"#).await;
    real.ask(r#"{"type":"nope","session":"b"}"#).await;
    {
        let b = s.ctx.bindings();
        assert_eq!(
            b.subscribers.len(),
            1,
            "a probe and an override bind nothing"
        );
        assert_eq!(
            b.subscribers.values().next().unwrap(),
            "a",
            "and it is bound once"
        );
    }
    // A state frame goes to the socket that asked about its project, and to no other.
    assert_eq!(s.ctx.state_targets("b").len(), 0);
    let targets = s.ctx.state_targets("a");
    assert_eq!(targets.len(), 1);
    assert!(targets[0].send_text(&"{\"type\":\"state\"}".into(), true));
    assert_eq!(real.text().await, "{\"type\":\"state\"}");
    drop(real);
    eventually("the subscription leaves with its socket", || {
        s.ctx.bindings().subscribers.is_empty()
    })
    .await;
    assert!(s.ctx.state_targets("a").is_empty());
}

#[tokio::test]
async fn a_page_from_a_site_not_allowed_is_closed_at_its_handshake_and_heard_no_further() {
    let mut c = cfg();
    c.allowed_origins = vec!["https://studio.example.org/".into()];
    let s = Server::start(c);
    let mut bad = Wire::connect(s.addr, Some("https://evil.example")).await;
    bad.send(WHOAMI).await;
    let frame = bad.closed_with().await.expect("a close frame");
    assert_eq!(u16::from(frame.code), 1008);
    assert_eq!(frame.reason.as_str(), "origin not allowed");
    for ok in [
        "http://localhost:5173",
        "https://app.k7.rocks",
        "https://studio.example.org",
        "ws://127.0.0.1:1",
    ] {
        let mut w = Wire::connect(s.addr, Some(ok)).await;
        assert!(w.ask(WHOAMI).await.contains("whoami"), "{ok}");
    }
    // No Origin at all is a native client.
    let mut native = s.connect().await;
    assert!(native.ask(WHOAMI).await.contains("whoami"));
}

#[tokio::test]
async fn a_machine_wide_reply_reaches_every_trusted_socket_and_a_projects_is_stamped() {
    let s = Server::start(cfg());
    let mut a = s.connect().await;
    let mut b = s.connect().await;
    a.ask(WHOAMI).await;
    b.ask(WHOAMI).await;
    s.ctx.broadcast(&json!({"type": "plugins", "list": []}));
    s.ctx.broadcast_to("k7", json!({"type": "recorded"}));
    for w in [&mut a, &mut b] {
        assert_eq!(w.text().await, r#"{"list":[],"type":"plugins"}"#);
        assert_eq!(w.text().await, r#"{"session":"k7","type":"recorded"}"#);
    }
}

#[tokio::test]
async fn a_quit_ends_the_loop_closes_every_socket_and_frees_the_port() {
    let mut s = Server::start(cfg());
    let mut w = s.connect().await;
    w.ask(WHOAMI).await;
    let addr = s.addr;
    assert_eq!(s.stop(), 0);
    let frame = w.closed_with().await.expect("going away");
    assert_eq!(u16::from(frame.code), 1001);
    assert!(tokio::net::TcpStream::connect(addr).await.is_err());
}

#[tokio::test]
async fn a_port_already_bound_is_a_failed_start() {
    let held = Server::start(cfg());
    let mut c = cfg();
    c.port = held.addr.port();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let api = k7_runtime_server::engine::Api::new(fake::api()).unwrap();
        tx.send(k7_runtime_server::server::serve(api, c)).unwrap();
    });
    assert_eq!(
        rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap(),
        1
    );
}

#[cfg(unix)]
#[tokio::test]
async fn sigterm_ends_the_message_loop() {
    let mut c = cfg();
    c.handle_signals = true;
    let mut s = Server::start(c);
    let status = std::process::Command::new("kill")
        .args(["-TERM", &std::process::id().to_string()])
        .status()
        .unwrap();
    assert!(status.success());
    // The loop returned on its own: stop only joins the thread.
    assert_eq!(std::thread::spawn(move || s.stop()).join().unwrap(), 0);
}
