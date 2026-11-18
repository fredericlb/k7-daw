//! The wire's shapes, as `frames.test.ts` holds them.

use super::*;

#[test]
fn reads_only_the_shapes_it_knows() {
    assert_eq!(
        parse_agent_message(r#"{"t":"opened","ch":1}"#),
        Some(AgentMessage::Opened { ch: 1 })
    );
    assert_eq!(
        parse_agent_message(r#"{"t":"opened","ch":1.0}"#),
        Some(AgentMessage::Opened { ch: 1 })
    );
    for bad in [
        r#"{"t":"opened","ch":"1"}"#,
        r#"{"t":"opened","ch":1.5}"#,
        r#"{"t":"close","ch":1,"reason":null}"#,
        r#"{"t":"stat","req":1,"ch":1,"paths":["a",2]}"#,
        r#"{"t":"nope"}"#,
        r#"[1]"#,
        "null",
        "not json",
    ] {
        assert_eq!(parse_agent_message(bad), None, "{bad}");
    }
    assert_eq!(text_frame_type("[1]"), "(not a JSON object)");
    assert_eq!(text_frame_type("{}"), "(no type)");
    assert_eq!(text_frame_type("{"), "(not JSON)");
}

#[test]
fn binary_frames_round_trip_and_refuse_a_short_header() {
    let f = encode_binary(&json!({ "t": "data", "ch": 3 }), b"abc");
    let (h, at) = decode_binary(&f).unwrap();
    assert_eq!(h, BinaryHeader::Data { ch: 3 });
    assert_eq!(&f[at..], b"abc");
    assert_eq!(decode_binary(&[0, 0, 0, 9, b'{']), None);
    assert_eq!(binary_frame_type(&[0, 0]), "(too short)");
    assert_eq!(
        binary_frame_type(&[0, 0, 0, 9, b'{']),
        "(header past the end)"
    );
    let put = encode_binary(
        &json!({ "t": "put", "req": 1, "ch": 2, "path": "/x", "last": "no" }),
        b"",
    );
    assert_eq!(decode_binary(&put), None);
}
