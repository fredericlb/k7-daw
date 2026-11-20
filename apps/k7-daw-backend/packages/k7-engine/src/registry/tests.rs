//! Where the server dials, what is this machine, as `relayGate.test.ts` and `@studio/web-protocol` hold them.

use super::*;
use crate::tunnel::backpressure::{Admit, HARD_LIMIT_BYTES, SOFT_LIMIT_BYTES, admit};
use crate::tunnel::hub::protocol_of;

#[test]
fn the_server_dials_the_heartbeats_own_source_never_one_it_names() {
    assert_eq!(
        hosts_for(Some("::ffff:192.168.1.9"), false),
        ["192.168.1.9"]
    );
    assert!(hosts_for(Some("127.0.0.1"), false).is_empty());
    assert!(hosts_for(Some("::1"), false).is_empty());
    assert!(hosts_for(Some("2001:db8::1"), false).is_empty());
    assert!(hosts_for(None, false).is_empty());
    assert_eq!(hosts_for(Some("203.0.113.5"), true), ["127.0.0.1"]);
}

#[test]
fn loopback_is_127_or_1_whatever_the_mapping() {
    for a in ["127.0.0.1", "::ffff:127.0.0.1", "::FFFF:127.1.2.3", "::1"] {
        assert!(is_loopback_address(Some(a)), "{a}");
    }
    for a in [
        "",
        "front-unverified",
        "10.0.0.1",
        "127.0.0.1.evil",
        "::ffff:10.0.0.1",
        "1270.0.0.1",
    ] {
        assert!(!is_loopback_address(Some(a)), "{a}");
    }
}

#[test]
fn a_stated_protocol_reads_as_engine_protocol_of() {
    assert_eq!(protocol_of(None), 1);
    assert_eq!(protocol_of(Some("2")), 2);
    assert_eq!(protocol_of(Some(" 3 ")), 3);
    for bad in ["0", "-1", "1.5", "x", ""] {
        assert_eq!(protocol_of(Some(bad)), 1, "{bad}");
    }
}

#[test]
fn a_slow_peer_loses_what_a_newer_frame_replaces_then_is_closed() {
    let state = crate::frame::Frame::text(r#"{"type":"state"}"#);
    let reply = crate::frame::Frame::text(r#"{"type":"project-loaded"}"#);
    assert_eq!(admit(0, &state), Admit::Send);
    assert_eq!(admit(SOFT_LIMIT_BYTES, &state), Admit::Drop);
    assert_eq!(admit(SOFT_LIMIT_BYTES, &reply), Admit::Send);
    assert_eq!(admit(HARD_LIMIT_BYTES, &reply), Admit::Close);
}

#[test]
fn a_loopback_hint_without_a_loopback_socket_is_not_this_machine() {
    let reg = Registry::new(
        Options::default(),
        std::sync::Arc::new(|_: &str| {}),
        crate::clock::Clock::default(),
    );
    let beat = |id: &str| {
        heartbeat::read_heartbeat(&serde_json::json!({
            "id": id, "token": "t", "hostname": "h", "addresses": { "port": 9000 }
        }))
        .expect("a heartbeat")
    };
    let forged = HeartbeatInput {
        beat: beat("forged"),
        public_ip_hint: Some("127.0.0.1".into()),
        from_loopback: false,
    };
    reg.upsert(forged).unwrap();
    assert!(!reg.on_this_machine("forged"));
    assert!(!reg.may_use("forged", "someone@example.org"));
    let local = HeartbeatInput {
        beat: beat("local"),
        public_ip_hint: Some("127.0.0.1".into()),
        from_loopback: true,
    };
    reg.upsert(local).unwrap();
    assert!(reg.on_this_machine("local"));
    assert!(reg.may_use("local", "someone@example.org"));
    assert!(!reg.may_use("local", ""));
}

#[test]
fn an_engines_chains_are_kept_and_its_junk_is_said() {
    let read = |chains: serde_json::Value| {
        heartbeat::read_heartbeat(&serde_json::json!({
            "id": "e1", "token": "t", "hostname": "h", "addresses": { "port": 9000 }, "chains": chains
        }))
        .expect("a heartbeat")
    };
    // Kept, trimmed, per `want` — the machine's own settings, not the caller's.
    let both = read(
        serde_json::json!({ "stems": " @k7-core/chains/stems.k7x ", "instrumental": "@k7-core/chains/instrumental.k7x" }),
    );
    let chains = both.chains.expect("chains");
    assert_eq!(chains.stems.as_deref(), Some("@k7-core/chains/stems.k7x"));
    assert_eq!(
        chains.instrumental.as_deref(),
        Some("@k7-core/chains/instrumental.k7x")
    );
    assert!(both.cleaned.is_empty(), "{:?}", both.cleaned);
    // Nothing named is nothing to say, and an older supervisor sends no `chains` at all.
    assert!(read(serde_json::json!({ "stems": "  " })).chains.is_none());
    assert!(read(serde_json::Value::Null).chains.is_none());
    // A name that is not a string costs that chain and is said; the other stays.
    let odd =
        read(serde_json::json!({ "stems": 4, "instrumental": "@k7-core/chains/instrumental.k7x" }));
    let odd_chains = odd.chains.expect("the instrumental survived");
    assert_eq!(odd_chains.stems, None);
    assert_eq!(
        odd_chains.instrumental.as_deref(),
        Some("@k7-core/chains/instrumental.k7x")
    );
    assert_eq!(odd.cleaned, ["chains.stems: not a name, left out"]);
    // The object itself wrong is the whole field lost, said once.
    let wrong = read(serde_json::json!("@k7-core/chains/stems.k7x"));
    assert!(wrong.chains.is_none());
    assert_eq!(wrong.cleaned, ["chains: not an object, left out"]);
}
