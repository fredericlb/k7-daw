// compare.rs — the generic rules of "equal enough", each on its own: keys as a set, arrays in order or as sets, tolerance, allow.
// A reader would get wrong that an absent key and a null are different: a Difference holds None for the side that lacks it.

use serde_json::json;
use song::compare::{Allow, Rules, SetRule, compare, compare_all, matches};

fn rules() -> Rules {
    Rules::default()
}

#[test]
fn key_order_does_not_count() {
    assert!(compare(&json!({"a": 1, "b": [1, 2]}), &json!({"b": [1, 2], "a": 1}), &rules()).is_empty());
}

#[test]
fn a_missing_key_is_a_difference_on_the_side_that_lacks_it() {
    let d = compare(&json!({"a": 1, "b": 2}), &json!({"a": 1, "c": 3}), &rules());
    assert_eq!(d.len(), 2);
    assert_eq!((d[0].pointer.as_str(), d[0].right.is_none()), ("/b", true));
    assert_eq!((d[1].pointer.as_str(), d[1].left.is_none()), ("/c", true));
    let null = compare(&json!({"a": null}), &json!({}), &rules());
    assert_eq!(null[0].left, Some(json!(null)));
}

#[test]
fn an_array_is_compared_in_order() {
    let d = compare(&json!({"x": [1, 2, 3]}), &json!({"x": [1, 3, 2]}), &rules());
    assert_eq!(
        d.iter().map(|d| d.pointer.as_str()).collect::<Vec<_>>(),
        ["/x/1", "/x/2"]
    );
    let longer = compare(&json!([1]), &json!([1, 2]), &rules());
    assert_eq!((longer[0].pointer.as_str(), longer[0].left.is_none()), ("/1", true));
}

#[test]
fn a_named_array_is_a_set_by_the_identity_of_its_items() {
    let mut r = rules();
    r.sets.push(SetRule {
        pointer: "/items".into(),
        key: "id".into(),
    });
    let left = json!({"items": [{"id": "a", "n": 1}, {"id": "b", "n": 2}]});
    assert!(compare(&left, &json!({"items": [{"id": "b", "n": 2}, {"id": "a", "n": 1}]}), &r).is_empty());
    let d = compare(&left, &json!({"items": [{"id": "b", "n": 9}, {"id": "c"}]}), &r);
    let at: Vec<&str> = d.iter().map(|d| d.pointer.as_str()).collect();
    assert_eq!(at, ["/items/\"a\"", "/items/\"b\"/n", "/items/\"c\""]);
}

#[test]
fn a_set_pattern_can_use_a_wildcard_segment() {
    let mut r = rules();
    r.sets.push(SetRule {
        pointer: "/p/*/diagnostics".into(),
        key: "file".into(),
    });
    let l = json!({"p": {"one": {"diagnostics": [{"file": "a"}, {"file": "b"}]}}});
    let r2 = json!({"p": {"one": {"diagnostics": [{"file": "b"}, {"file": "a"}]}}});
    assert!(compare(&l, &r2, &r).is_empty());
}

#[test]
fn numbers_are_equal_within_the_tolerance() {
    assert!(compare(&json!(1.0), &json!(1), &rules()).is_empty());
    assert!(compare(&json!(1000.0), &json!(1000.0000005), &rules()).is_empty());
    assert!(compare(&json!(0.0), &json!(5e-10), &rules()).is_empty());
    assert_eq!(compare(&json!(1.0), &json!(1.001), &rules()).len(), 1);
    assert_eq!(compare(&json!(0.0), &json!(1e-6), &rules()).len(), 1);
    let loose = Rules {
        tolerance: 1e-2,
        ..Rules::default()
    };
    assert!(compare(&json!(1.0), &json!(1.001), &loose).is_empty());
}

#[test]
fn types_and_scalars_differ_plainly() {
    assert_eq!(compare(&json!("a"), &json!("b"), &rules()).len(), 1);
    assert_eq!(compare(&json!(1), &json!("1"), &rules()).len(), 1);
    assert_eq!(compare(&json!([]), &json!({}), &rules()).len(), 1);
    assert_eq!(compare(&json!(true), &json!(null), &rules()).len(), 1);
}

#[test]
fn escapes_a_pointer() {
    let d = compare(&json!({"a/b": 1, "c~d": 1}), &json!({"a/b": 2, "c~d": 2}), &rules());
    assert_eq!(
        d.iter().map(|d| d.pointer.as_str()).collect::<Vec<_>>(),
        ["/a~1b", "/c~0d"]
    );
}

#[test]
fn collects_at_most_max() {
    let left: Vec<i32> = (0..100).collect();
    let right: Vec<i32> = (100..200).collect();
    let r = Rules {
        max: 5,
        ..Rules::default()
    };
    assert_eq!(compare(&json!(left), &json!(right), &r).len(), 5);
}

#[test]
fn an_allowed_difference_is_set_aside_with_its_reason() {
    let mut r = rules();
    r.allow.push(Allow {
        pattern: "/a/*/when".into(),
        reason: "the clock".into(),
    });
    r.allow.push(Allow {
        pattern: "/deep/**".into(),
        reason: "a whole subtree".into(),
    });
    let l = json!({"a": {"x": {"when": 1, "k": 1}}, "deep": {"p": {"q": 1}}});
    let r2 = json!({"a": {"x": {"when": 2, "k": 2}}, "deep": {"p": {"q": 2}}});
    let out = compare_all(&l, &r2, &r);
    assert_eq!(out.differences.len(), 1);
    assert_eq!(out.differences[0].pointer, "/a/x/k");
    let reasons: Vec<&str> = out.allowed.iter().map(|(_, why)| why.as_str()).collect();
    assert_eq!(reasons, ["the clock", "a whole subtree"]);
}

#[test]
fn matches_a_pointer_against_a_pattern() {
    assert!(matches("/a/*/c", "/a/b/c"));
    assert!(!matches("/a/*/c", "/a/b/d"));
    assert!(!matches("/a/*", "/a/b/c"));
    assert!(matches("/a/**", "/a/b/c"));
    assert!(matches("", ""));
    assert!(!matches("/a", ""));
}

fn functions() -> Rules {
    Rules {
        functions: vec!["/a/*/points".into()],
        ..Rules::default()
    }
}

fn lane(points: serde_json::Value) -> serde_json::Value {
    json!({ "a": [{ "points": points }] })
}

// Decision 355: the same curve with other breakpoints is equal, because both are sampled at the union of their breakpoints.
#[test]
fn a_function_with_other_breakpoints_is_the_same_function() {
    let line = lane(json!([[0, 0], [4, 4]]));
    let dotted = lane(json!([[0, 0], [1, 1], [2, 2], [4, 4]]));
    assert!(compare(&line, &dotted, &functions()).is_empty());
    // Not a function rule: the arrays differ plainly.
    assert!(!compare(&line, &dotted, &rules()).is_empty());
}

// Decision 355: the last value is held after the last breakpoint and the first before the first.
#[test]
fn a_function_holds_its_ends() {
    let short = lane(json!([[1, 2], [3, 5]]));
    let held = lane(json!([[0, 2], [1, 2], [3, 5], [9, 5]]));
    assert!(compare(&short, &held, &functions()).is_empty());
}

// Decision 355: a function that goes elsewhere between two shared breakpoints is a difference.
#[test]
fn a_wrong_function_is_a_difference() {
    let a = lane(json!([[0, 0], [4, 4]]));
    for wrong in [
        json!([[0, 0], [2, 3], [4, 4]]),
        json!([[0, 0], [4, 5]]),
        json!([[0, 1], [4, 4]]),
        json!([[0, 0]]),
    ] {
        let d = compare(&a, &lane(wrong.clone()), &functions());
        assert_eq!(d.len(), 1, "{wrong}");
        assert_eq!(d[0].pointer, "/a/0/points");
    }
}

// Decision 355: the tolerance is absolute and the stated one, narrow enough to tell a wrong curve from float noise.
#[test]
fn a_function_is_equal_within_the_tolerance_and_no_wider() {
    let r = functions();
    let a = lane(json!([[0, 1], [4, 2]]));
    assert!(compare(&a, &lane(json!([[0, 1.0 + r.function_tolerance / 2.0], [4, 2]])), &r).is_empty());
    assert!(!compare(&a, &lane(json!([[0, 1.0 + r.function_tolerance * 4.0], [4, 2]])), &r).is_empty());
    // A value of 1e6 is not excused by being large.
    let big = lane(json!([[0, 1e6], [4, 1e6]]));
    assert!(!compare(&big, &lane(json!([[0, 1e6 + 1e-6], [4, 1e6]])), &r).is_empty());
}

// Decision 355: points that are not pairs of numbers cannot be sampled, so they differ.
#[test]
fn a_function_that_is_not_pairs_of_numbers_is_a_difference() {
    let a = lane(json!([[0, 0], [1, 1]]));
    for bad in [json!([[0, 0], ["x", 1]]), json!([[0], [1, 1]]), json!([1, 2])] {
        assert_eq!(compare(&a, &lane(bad.clone()), &functions()).len(), 1, "{bad}");
    }
}

fn masks() -> Rules {
    Rules {
        masks: vec!["/clips/*/source".into()],
        ..Rules::default()
    }
}

fn source(s: &str) -> serde_json::Value {
    json!({ "clips": [{ "source": s }] })
}

// Decision 355: a render's name differs by its fingerprint alone, and only there.
#[test]
fn a_render_is_equal_whatever_its_fingerprint() {
    let a = source("/r/gtr-0123456789abcdef.wav");
    let b = source("/r/gtr-fedcba9876543210.wav");
    assert!(compare(&a, &b, &masks()).is_empty());
    assert!(!compare(&a, &b, &rules()).is_empty());
}

// Decision 355: a mask hides the 16 hex digits and nothing else: another module, folder or extension is a difference.
#[test]
fn a_wrong_mask_is_a_difference() {
    let a = source("/r/gtr-0123456789abcdef.wav");
    for other in [
        "/r/bass-0123456789abcdef.wav",
        "/q/gtr-0123456789abcdef.wav",
        "/r/gtr-0123456789abcdef.flac",
        "/r/gtr-0123456789abcde.wav",
        "/r/gtr-0123456789abcdeg.wav",
    ] {
        let hidden = compare(&a, &source(other), &masks());
        assert_eq!(hidden.len(), 1, "{other}");
    }
    // The mask is by pattern: a source at another pointer is compared plainly.
    let elsewhere = json!({ "other": [{ "source": "/r/gtr-0123456789abcdef.wav" }] });
    let elsewhere2 = json!({ "other": [{ "source": "/r/gtr-fedcba9876543210.wav" }] });
    assert!(!compare(&elsewhere, &elsewhere2, &masks()).is_empty());
}
