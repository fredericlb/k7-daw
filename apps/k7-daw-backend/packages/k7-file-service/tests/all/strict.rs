//! The comparison (1006): JavaScript's own-key order, numbers as `f64`, `null` against absent, a budget, a cut.

use std::time::{Duration, Instant};

use k7_file_service::strict::{Compared, first_difference};
use serde_json::{Value, json};

const BOUND: Duration = Duration::from_secs(if cfg!(target_env = "musl") { 60 } else { 15 });

fn parse(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}

fn differs(ts: &Value, native: &Value) -> (String, String, String) {
    match first_difference(ts, native, 4_000_000) {
        Compared::Differs {
            pointer,
            ts,
            native,
        } => (pointer, ts, native),
        other => panic!("{other:?}"),
    }
}

#[test]
fn equal_values_are_the_same_and_the_first_difference_is_named_by_its_pointer() {
    let a = json!({"a": [1, 2, {"b": "x"}], "c": null});
    assert_eq!(first_difference(&a, &a.clone(), 100), Compared::Same);
    let b = json!({"a": [1, 2, {"b": "y"}], "c": null});
    assert_eq!(
        differs(&a, &b),
        ("/a/2/b".into(), "\"x\"".into(), "\"y\"".into())
    );
    assert_eq!(
        differs(&json!({"a/b": {"~": 1}}), &json!({"a/b": {"~": 2}})).0,
        "/a~1b/~0"
    );
    assert_eq!(
        differs(&json!(1), &json!("1")),
        ("".into(), "1".into(), "\"1\"".into())
    );
}

#[test]
fn numbers_are_compared_as_f64() {
    assert_eq!(
        first_difference(&parse("1"), &parse("1.0"), 10),
        Compared::Same
    );
    assert_eq!(
        first_difference(&parse("[0.1,2e2]"), &parse("[0.1,200]"), 10),
        Compared::Same
    );
    assert_ne!(
        first_difference(&parse("1"), &parse("1.5"), 10),
        Compared::Same
    );
}

#[test]
fn null_and_absent_differ_and_so_do_a_shorter_array_and_a_missing_key() {
    assert_eq!(
        differs(&json!({"a": null}), &json!({})),
        ("/a".into(), "null".into(), "absent".into())
    );
    assert_eq!(
        differs(&json!({}), &json!({"a": null})),
        ("/a".into(), "absent".into(), "null".into())
    );
    assert_eq!(
        differs(&json!([1, 2]), &json!([1])),
        ("/1".into(), "2".into(), "absent".into())
    );
    assert_eq!(
        differs(&json!([null]), &json!([])),
        ("/0".into(), "null".into(), "absent".into())
    );
}

#[test]
fn keys_are_compared_in_javascripts_order_not_by_name() {
    // Integer-like keys list first and ascending, whatever order they were inserted in.
    let a = parse(r#"{"b":1,"10":2,"9":3,"a":4}"#);
    let b = parse(r#"{"9":3,"10":2,"b":1,"a":4}"#);
    assert_eq!(first_difference(&a, &b, 100), Compared::Same);
    // Other keys keep their sequence: the same keys in another order are a difference a client could read.
    let (pointer, ts, native) = differs(&parse(r#"{"a":1,"b":2}"#), &parse(r#"{"b":2,"a":1}"#));
    assert_eq!(
        (pointer.as_str(), ts.as_str(), native.as_str()),
        ("/a", "key \"a\" at 0", "key \"b\" at 0")
    );
    // "07" and "4294967295" are not array indexes: they stay where they were inserted.
    let c = parse(r#"{"b":1,"07":2,"1":3}"#);
    let d = parse(r#"{"1":3,"b":1,"07":2}"#);
    assert_eq!(first_difference(&c, &d, 100), Compared::Same);
    let e = parse(r#"{"07":2,"b":1,"1":3}"#);
    assert_ne!(first_difference(&c, &e, 100), Compared::Same);
}

#[test]
fn two_hundred_thousand_rows_differing_at_the_end_are_found_fast() {
    let rows = |last: u64| {
        Value::Array(
            (0..100_000)
                .map(|i| json!({"i": i, "v": if i == 99_999 { last } else { i }}))
                .collect(),
        )
    };
    let t0 = Instant::now();
    let (pointer, ts, native) = differs(&rows(1), &rows(2));
    assert_eq!(
        (pointer.as_str(), ts.as_str(), native.as_str()),
        ("/99999/v", "1", "2")
    );
    assert!(t0.elapsed() < BOUND);
}

#[test]
fn a_budget_of_nodes_ends_a_comparison_as_capped() {
    let a = Value::Array((0..1000).map(|i| json!(i)).collect());
    assert_eq!(first_difference(&a, &a.clone(), 500), Compared::Capped);
    assert_eq!(first_difference(&a, &a.clone(), 1001), Compared::Same);
}

#[test]
fn a_huge_string_is_cut_to_a_line_without_being_written() {
    let big = "é".repeat(2_500_000);
    let a = json!({"k": big});
    let b = json!({"k": "other"});
    let (_, ts, native) = differs(&a, &b);
    assert!(ts.len() <= 170 && ts.ends_with("...\""), "{}", ts.len());
    assert_eq!(native, "\"other\"");
    // A container is cut too, at the byte, not at an element.
    let many = Value::Array((0..100_000).map(|i| json!(i)).collect());
    let (_, ts, _) = differs(&json!({"a": many}), &json!({"a": 1}));
    assert!(ts.len() <= 175, "{}", ts.len());
}

#[test]
fn a_document_nested_past_the_limit_is_capped_not_overflowed() {
    let mut v = json!(1);
    for _ in 0..2_000 {
        v = Value::Array(vec![v]);
    }
    assert_eq!(
        first_difference(&v, &v.clone(), 1_000_000),
        Compared::Capped
    );
}
