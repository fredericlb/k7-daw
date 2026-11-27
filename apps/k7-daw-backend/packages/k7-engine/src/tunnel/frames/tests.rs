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
fn refuses_a_message_that_is_json_but_not_a_message() {
    for bad in [
        "null",
        "1",
        r#""x""#,
        r#"{"t":1}"#,
        "[]",
        r#"{"t":"data"}"#,
        "not json",
        r#"{"t":"stat","req":1,"ch":1,"paths":"a"}"#,
        r#"{"t":"stat","req":1,"ch":1,"paths":[1]}"#,
    ] {
        assert_eq!(parse_agent_message(bad), None, "{bad}");
    }
}

#[test]
fn takes_every_well_formed_message() {
    for m in [
        r#"{"t":"opened","ch":1}"#,
        r#"{"t":"close","ch":1,"reason":"bye"}"#,
        r#"{"t":"data","ch":1,"text":"{}"}"#,
        r#"{"t":"stat","req":1,"ch":1,"paths":["a"]}"#,
        r#"{"t":"get","req":1,"ch":1,"path":"a"}"#,
    ] {
        assert!(parse_agent_message(m).is_some(), "{m}");
    }
}

#[test]
fn refuses_a_binary_header_that_is_not_one_and_round_trips_every_kind() {
    let raw = |header: &str| {
        let mut f = (header.len() as u32).to_be_bytes().to_vec();
        f.extend_from_slice(header.as_bytes());
        f.push(b'x');
        f
    };
    for h in [
        "null",
        "1",
        r#""x""#,
        r#"{"t":1}"#,
        r#"{"t":"put","req":1}"#,
    ] {
        assert_eq!(decode_binary(&raw(h)), None, "{h}");
    }
    for h in [
        json!({ "t": "data", "ch": 2 }),
        json!({ "t": "chunk", "req": 3, "last": true }),
        json!({ "t": "put", "req": 4, "ch": 2, "path": "/a", "last": false }),
    ] {
        let f = encode_binary(&h, b"p");
        let (_, at) = decode_binary(&f).expect("a header of this kind");
        assert_eq!(&f[at..], b"p", "{h}");
    }
}

#[test]
fn names_a_frame_nobody_reads_by_its_type_or_says_why_it_has_none() {
    assert_eq!(text_frame_type(r#"{"t":"future"}"#), "future");
    assert_eq!(text_frame_type(r#"{"x":1}"#), "(no type)");
    let later = {
        let h = r#"{"t":"later"}"#;
        let mut f = (h.len() as u32).to_be_bytes().to_vec();
        f.extend_from_slice(h.as_bytes());
        f.push(b'x');
        f
    };
    assert_eq!(binary_frame_type(&later), "later");
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
