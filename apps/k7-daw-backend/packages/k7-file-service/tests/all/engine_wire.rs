//! The wire of a session's socket (2213, 2214, 2217): what is stamped going out, what is read coming in, the outbox.

use std::time::{Duration, Instant};

use k7_file_service::engine::wire::{In, Outbox, decode_in, for_session, stamp};
use serde_json::json;

#[test]
fn a_frame_names_its_session_first_and_the_bare_session_names_none() {
    let out = stamp("k7:ann", json!({ "type": "flush" }));
    assert_eq!(out.to_string(), r#"{"session":"k7:ann","type":"flush"}"#);
    assert_eq!(
        stamp("", json!({ "type": "flush" })),
        json!({ "type": "flush" })
    );
}

#[test]
fn only_the_disagreeing_session_is_dropped() {
    assert!(for_session(&json!({ "type": "plugins" }), "k7"));
    assert!(for_session(&json!({ "session": "k7" }), "k7"));
    assert!(!for_session(&json!({ "session": "banc" }), "k7"));
    // A session that is no string is no disagreement.
    assert!(for_session(&json!({ "session": 4 }), "k7"));
}

#[test]
fn a_state_frame_keeps_the_transport_and_nothing_else() {
    let got = decode_in(
        r#"{"type":"state","session":"k7","playing":1,"recordEnabled":true,"time":3}"#,
        "k7",
    );
    assert_eq!(
        got,
        In::State {
            playing: true,
            record_enabled: true
        }
    );
    assert_eq!(
        decode_in(r#"{"type":"state"}"#, "k7"),
        In::State {
            playing: false,
            record_enabled: false
        }
    );
    assert_eq!(
        decode_in(r#"{"type":"state","session":"other"}"#, "k7"),
        In::Ignored
    );
}

#[test]
fn the_scan_the_sessions_and_the_error_are_read_defensively() {
    let scan = decode_in(
        r#"{"type":"plugin-scan","scanning":true,"done":-4,"total":"x","current":9,"version":2}"#,
        "k7",
    );
    let In::Scan(s) = scan else {
        panic!("{scan:?}")
    };
    assert!(s.scanning && s.done == 0 && s.total == 0 && s.current.is_empty() && s.version == 2);
    let In::Sessions(rows) = decode_in(
        r#"{"type":"sessions","sessions":[{"id":"k7","loaded":true},{"id":5}]}"#,
        "k7",
    ) else {
        panic!()
    };
    assert_eq!(
        (rows[0].loaded, rows[1].id.as_str(), rows[1].loaded),
        (true, "5", false)
    );
    assert_eq!(
        decode_in(r#"{"type":"error"}"#, "k7"),
        In::Error("unknown error".into())
    );
}

#[test]
fn the_outbox_drops_the_oldest_and_what_is_stale() {
    let t0 = Instant::now();
    let mut o = Outbox::new(3, Duration::from_secs(30));
    for i in 0..5 {
        o.push(json!(i), t0);
    }
    assert_eq!(o.len(), 3);
    assert_eq!(
        o.drain(t0 + Duration::from_secs(1)),
        vec![json!(2), json!(3), json!(4)]
    );
    o.push(json!("old"), t0);
    o.push(json!("new"), t0 + Duration::from_secs(25));
    assert_eq!(o.drain(t0 + Duration::from_secs(31)), vec![json!("new")]);
    assert!(o.is_empty());
}
