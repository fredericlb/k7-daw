//! server_units.rs — the socket core's pure parts: the screens' rules, the parse bound, the field reads and
//! the outbox, held to what the server's docs say.

use k7_runtime_server::server::msg::{self, Msg};
use k7_runtime_server::server::outbox::{
    Frame, MAX_QUEUED_REPLY_BYTES, MAX_SOCKET_BACKLOG, Outbox,
};
use serde_json::json;
use std::sync::Arc;

fn nested(levels: usize) -> String {
    format!("{}{}", "[".repeat(levels), "]".repeat(levels))
}

#[test]
fn nesting_is_refused_past_64_levels_before_it_is_parsed() {
    assert!(msg::parse(&nested(64)).is_ok());
    let refused = msg::parse(&nested(65)).unwrap_err();
    assert_eq!(refused, "message nested deeper than 64 levels");
    // Strings holding brackets are not nesting.
    let text = format!("{{\"a\":\"{}\"}}", "[".repeat(500));
    assert!(msg::parse(&text).is_ok());
    let escaped = format!("{{\"a\":\"\\\"{}\"}}", "[".repeat(500));
    assert!(msg::parse(&escaped).is_ok());
    // A depth that would overflow a stack is answered, not walked.
    assert!(msg::parse(&nested(200_000)).is_err());
}

#[test]
fn a_field_is_read_as_nlohmanns_value_read_it() {
    let m = Msg(json!({"type": "x", "n": 3, "f": 1.5, "b": true, "z": null}));
    assert_eq!(m.str("type").unwrap(), "x");
    assert_eq!(m.str("absent").unwrap(), "");
    assert_eq!(m.i64_or("n", 0).unwrap(), 3);
    assert_eq!(m.f64_or("n", 0.0).unwrap(), 3.0);
    assert_eq!(m.i64_or("f", 0).unwrap(), 1);
    assert!(m.bool_or("b", false).unwrap());
    assert_eq!(
        m.str("n").unwrap_err().0,
        "[json.exception.type_error.302] type must be string, but is number"
    );
    assert_eq!(
        m.str("z").unwrap_err().0,
        "[json.exception.type_error.302] type must be string, but is null"
    );
    assert_eq!(
        m.bool_or("type", false).unwrap_err().0,
        "[json.exception.type_error.302] type must be boolean, but is string"
    );
    for (value, name) in [
        (json!([]), "array"),
        (json!(null), "null"),
        (json!("hi"), "string"),
        (json!(5), "number"),
        (json!(true), "boolean"),
    ] {
        assert_eq!(
            Msg(value).str("type").unwrap_err().0,
            format!("[json.exception.type_error.306] cannot use value() with {name}")
        );
    }
}

fn text(s: &str) -> Arc<str> {
    Arc::from(s)
}

#[tokio::test]
async fn replies_are_kept_in_order_and_a_state_frame_is_dropped_while_the_socket_is_behind() {
    let o = Outbox::default();
    assert!(o.post(&text("one"), false));
    assert!(o.post(&text("two"), true));
    assert_eq!(o.next().await, Some(Frame::Text(text("one"))));
    assert_eq!(o.next().await, Some(Frame::Text(text("two"))));
    assert_eq!(o.queued_bytes(), 0);

    let big = Arc::<str>::from("x".repeat(MAX_SOCKET_BACKLOG + 1));
    assert!(o.post(&big, false));
    assert!(
        !o.post(&text("state"), true),
        "a droppable frame is skipped behind a backlog"
    );
    assert!(o.post(&text("reply"), false), "a reply is not");
    o.next().await;
    assert!(o.post(&text("state"), true), "the backlog drained");
}

#[tokio::test]
async fn a_reply_is_refused_only_past_its_cap() {
    let o = Outbox::default();
    let mib = Arc::<str>::from("x".repeat(1024 * 1024));
    let fits = MAX_QUEUED_REPLY_BYTES / mib.len();
    for _ in 0..fits {
        assert!(o.post(&mib, false));
    }
    assert!(!o.post(&mib, false));
    assert!(o.next().await.is_some());
    assert!(o.post(&mib, false));
}

#[tokio::test]
async fn a_close_ends_what_is_taken_and_a_shutdown_ends_the_writer() {
    let o = Outbox::default();
    assert!(o.post(&text("last words"), false));
    assert!(o.ping());
    assert!(o.close(1008, "origin not allowed"));
    assert!(!o.post(&text("late"), false));
    assert_eq!(o.next().await, Some(Frame::Text(text("last words"))));
    assert_eq!(o.next().await, Some(Frame::Ping));
    assert_eq!(
        o.next().await,
        Some(Frame::Close(1008, "origin not allowed"))
    );
    o.shutdown();
    assert!(o.is_gone());
    assert_eq!(o.next().await, None);
    o.gone().await;
}
