//! `/logs`' model (2622): the query's numbers, the machine's half however it answers, and the answer's key order.

use k7_file_service::runtime::logs::{Query, answer, machine_half, service_half, to_number};
use k7_file_service::runtime::ring::{Level, Ring};
use serde_json::{Value, json};

#[test]
fn a_query_is_clamped_and_sent_to_the_machine_as_the_route_does() {
    let q = Query::parse(Some("3"), Some(" 7 "), Some("9999"));
    assert_eq!((q.after, q.offset, q.tail), (Some(3.0), Some(7.0), 2000.0));
    assert_eq!(
        q.machine_request(),
        json!({ "type": "log", "offset": 7.0, "tail": 2000.0 })
    );
    let q = Query::parse(None, Some("abc"), Some("0"));
    assert_eq!(q.tail, 300.0);
    assert_eq!(q.machine_request(), json!({ "type": "log", "tail": 300.0 }));
    assert_eq!(Query::parse(None, None, Some("3.7")).count(), 3);
}

#[test]
fn javascript_traps_a_rust_parse_would_miss() {
    for bad in [
        "-0x10", "+0x10", "0x", "1e", "e5", "inf", "nan", "1n", "1_0", "infinity",
    ] {
        assert!(to_number(bad).is_nan(), "{bad}");
    }
    assert_eq!(to_number("0b11"), 3.0);
    assert_eq!(to_number("\u{feff}3\u{2028}"), 3.0);
    assert_eq!(to_number(""), 0.0);
    assert_eq!(to_number("-Infinity"), f64::NEG_INFINITY);
}

#[test]
fn the_machines_half_is_empty_unless_it_is_offset_and_strings() {
    let empty = json!({ "offset": 0, "lines": [] });
    let ok = json!({ "offset": -4, "lines": ["a", "b"], "extra": 1 });
    assert_eq!(
        machine_half(Some(&ok)),
        json!({ "offset": -4, "lines": ["a", "b"] })
    );
    for bad in [
        json!(null),
        json!("x"),
        json!({ "offset": "1", "lines": [] }),
        json!({ "offset": 1, "lines": [1] }),
        json!({ "offset": 1 }),
    ] {
        assert_eq!(machine_half(Some(&bad)), empty, "{bad}");
    }
    assert_eq!(machine_half(None), empty);
}

#[test]
fn the_answer_keys_are_offset_lines_service() {
    let mut r = Ring::new();
    r.push(Level::Info, "hi", 5);
    let a = answer(
        &machine_half(None),
        service_half(&r, &Query::parse(None, None, None)),
    );
    let keys: Vec<&String> = a.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["offset", "lines", "service"]);
    assert_eq!(
        a["service"],
        json!({ "seq": 1, "lines": [{ "seq": 1, "t": 5, "level": "info", "text": "hi" }] })
    );
    let line_keys: Vec<&String> = a["service"]["lines"][0]
        .as_object()
        .unwrap()
        .keys()
        .collect();
    assert_eq!(line_keys, ["seq", "t", "level", "text"]);
    let _: &Value = &a;
}
