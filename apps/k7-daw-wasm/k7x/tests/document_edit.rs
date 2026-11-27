// document_edit.rs — what an edit removes or writes: prune, the neighbours' comments, set_in, chains and empty containers.
// A reader would get wrong that these are hand-written expectations: the frozen vectors (edit_vectors.rs) are the machine-made ones.

#[macro_use]
mod common;

use common::{PLAIN, data, edit, set, v};
use k7x::edit::{Document, PathSeg, Value};

const TEXT: &str =
    "a(1)\nengines({\n  tracktion: {\n    params: { Ratio: 0.5 }, // the only one\n  },\n})\n";

#[test]
fn prune_removes_every_ancestor_map_the_deletion_emptied() {
    let ratio = p!["engines", "tracktion", "params", "Ratio"];
    assert_eq!(
        edit(TEXT, |d| assert!(d.delete_in(&ratio, true).unwrap())),
        "a(1)\n"
    );
    let out = edit(TEXT, |d| assert!(d.delete_in(&ratio, false).unwrap()));
    assert_eq!(
        data(&out).get("engines").unwrap(),
        &v(r#"{"tracktion":{"params":{}}}"#)
    );
}

#[test]
fn prune_stops_at_a_map_that_still_holds_something_and_spares_the_root_and_list_items() {
    assert_eq!(
        edit("e({ x: { y: 1 }, z: 2 })", |d| assert!(
            d.delete_in(&p!["e", "x", "y"], true).unwrap()
        )),
        "e({ z: 2 })"
    );
    assert_eq!(
        edit("a({ b: 1 })", |d| assert!(
            d.delete_in(&p!["a", "b"], true).unwrap()
        )),
        ""
    );
    assert_eq!(
        edit("l([{ b: 1 }, 2])", |d| assert!(
            d.delete_in(&p!["l", 0usize, "b"], true).unwrap()
        )),
        "l([{}, 2])"
    );
    assert!(!Document::new("a(1)").delete_in(&[], false).unwrap());
}

#[test]
fn a_deletion_keeps_the_neighbours_comments() {
    let text = "o({\n  a: 1,\n  b: 2, // tail\n  /* c */ c: 3,\n})\n";
    let out = edit(text, |d| {
        assert!(d.delete_in(&p!["o", "c"], false).unwrap())
    });
    assert!(out.contains("b: 2, // tail\n"));
    assert_eq!(data(&out).get("o").unwrap(), &v(r#"{"a":1,"b":2}"#));
    assert_eq!(
        edit("o({ x: 0, a: 1, // c\n  b: 2 })", |d| assert!(
            d.delete_in(&p!["o", "a"], false).unwrap()
        )),
        "o({ x: 0, // c\n  b: 2 })"
    );
}

#[test]
fn a_comma_written_first_on_the_next_line_goes_with_the_item() {
    assert_eq!(
        edit("l([t(0).d(1) // one\n  , t(2)])\n", |d| assert!(
            d.delete_in(&p!["l", 0usize], false).unwrap()
        )),
        "l([ // one\n  t(2)])\n"
    );
    assert_eq!(
        edit("l([\n  1\n  , 2\n])\n", |d| assert!(
            d.delete_in(&p!["l", 0usize], false).unwrap()
        )),
        "l([\n  2\n])\n"
    );
}

#[test]
fn a_list_emptied_of_its_last_line_is_brackets_and_one_holding_a_comment_is_kept() {
    assert_eq!(
        edit("l([\n  1, // one\n])\n", |d| assert!(
            d.delete_in(&p!["l", 0usize], false).unwrap()
        )),
        "l([])\n"
    );
    let kept = "l([\n  // keep\n  1,\n])\n";
    assert_eq!(
        edit(kept, |d| assert!(
            d.delete_in(&p!["l", 0usize], false).unwrap()
        )),
        "l([\n  // keep\n])\n"
    );
    assert_eq!(
        edit(kept, |d| d
            .apply_in(&p!["l"], &Value::List(vec![]))
            .unwrap()),
        "l([\n  // keep\n])\n"
    );
    assert_eq!(
        edit("l([1, 2])\n", |d| d
            .apply_in(&p!["l"], &Value::List(vec![]))
            .unwrap()),
        "l([])\n"
    );
}

const EDITED: &str = "// synth-pad\nextends(\"@workspace/instruments/synth\")\nparams({\n  \"Filter Freq\": 40, // darker than the lead\n  Level: -12,\n})\nautomations([])\n";

#[test]
fn set_in_rewrites_a_scalar_in_place_adds_entries_and_creates_the_maps_on_the_way() {
    assert_eq!(
        edit(EDITED, |d| set(d, &p!["params", "Filter Freq"], "55")),
        EDITED.replace("40,", "55,")
    );
    assert_eq!(
        edit(EDITED, |d| set(d, &p!["params", "Level"], "-12")),
        EDITED
    );
    assert_eq!(
        edit(EDITED, |d| set(d, &p!["params", "Detune"], "3")),
        EDITED.replace("  Level: -12,\n", "  Level: -12,\n  Detune: 3,\n")
    );
    assert_eq!(
        edit(EDITED, |d| set(d, &p!["bypass"], "true")),
        format!("{EDITED}bypass(true)\n")
    );
    assert_eq!(
        edit("a(1)\n", |d| set(
            d,
            &p!["engines", "tracktion", "params", "Ratio"],
            "0.5"
        )),
        "a(1)\nengines({ tracktion: { params: { Ratio: 0.5 } } })\n"
    );
}

#[test]
fn a_chain_is_extended_inline_or_on_a_new_line_when_broken() {
    assert_eq!(
        edit("steps([id(1).x(2)])", |d| set(
            d,
            &p!["steps", 0usize, "y"],
            "3"
        )),
        "steps([id(1).x(2).y(3)])"
    );
    assert_eq!(
        edit("steps([\n  id(1)\n    .x(2), // two\n])\n", |d| set(
            d,
            &p!["steps", 0usize, "y"],
            "3"
        )),
        "steps([\n  id(1)\n    .x(2)\n    .y(3), // two\n])\n"
    );
}

#[test]
fn empty_containers_are_filled_and_lists_appended_to() {
    assert_eq!(
        edit("markers({})", |d| set(
            d,
            &p!["markers", "1:1"],
            "\"intro\""
        )),
        "markers({ \"1:1\": \"intro\" })"
    );
    assert_eq!(
        edit("tags([])", |d| d
            .add_in(&p!["tags"], &Value::Str("a".into()), PLAIN)
            .unwrap()),
        "tags([\"a\"])"
    );
    assert_eq!(
        edit("tags([1, 2])", |d| d
            .add_in(&p!["tags"], &Value::Num(3.0), PLAIN)
            .unwrap()),
        "tags([1, 2, 3])"
    );
    assert_eq!(
        edit("notes([\n  t(0).k(60),\n  t(1).k(62)\n])", |d| d
            .add_in(&p!["notes"], &v(r#"{"t":2,"k":64}"#), PLAIN)
            .unwrap()),
        "notes([\n  t(0).k(60),\n  t(1).k(62),\n  t(2).k(64),\n])"
    );
}

#[test]
fn a_calls_bare_argument_list_stays_bare_while_it_stays_one_and_an_empty_call_is_set() {
    assert_eq!(
        edit("to(1, 2)", |d| set(d, &p!["to"], "[1,2,3]")),
        "to(1, 2, 3)"
    );
    assert_eq!(edit("to(1, 2)", |d| set(d, &p!["to"], "[1]")), "to([1])");
    assert_eq!(
        edit("to(1, 2)", |d| d
            .add_in(&p!["to"], &Value::Num(3.0), PLAIN)
            .unwrap()),
        "to(1, 2, 3)"
    );
    assert_eq!(
        edit("bypass() x(1)", |d| set(d, &p!["bypass"], "false")),
        "bypass(false) x(1)"
    );
}

#[test]
fn delete_in_removes_an_entrys_whole_line_a_statement_and_a_chain_call() {
    assert_eq!(
        edit(EDITED, |d| assert!(
            d.delete_in(&p!["params", "Filter Freq"], false).unwrap()
        )),
        EDITED.replace("  \"Filter Freq\": 40, // darker than the lead\n", "")
    );
    assert_eq!(
        edit(EDITED, |d| assert!(
            d.delete_in(&p!["automations"], false).unwrap()
        )),
        EDITED.replace("automations([])\n", "")
    );
    let del = |text: &str, path: Vec<PathSeg>| {
        edit(text, |d| assert!(d.delete_in(&path, false).unwrap()))
    };
    assert_eq!(
        del("s([a(1).b(2).c(3)])", p!["s", 0usize, "b"]),
        "s([a(1).c(3)])"
    );
    assert_eq!(
        del("s([a(1).b(2).c(3)])", p!["s", 0usize, "a"]),
        "s([b(2).c(3)])"
    );
    assert_eq!(
        del(
            "s([\n  a(1)\n    .b(2)\n    .c(3),\n])",
            p!["s", 0usize, "b"]
        ),
        "s([\n  a(1)\n    .c(3),\n])"
    );
    assert_eq!(
        del("s([a(1), b(2)])", p!["s", 1usize, "b"]),
        "s([a(1), {}])"
    );
}

#[test]
fn delete_in_removes_list_items_with_their_commas_and_says_when_nothing_was_there() {
    let del = |text: &str, path: Vec<PathSeg>| {
        edit(text, |d| assert!(d.delete_in(&path, false).unwrap()))
    };
    assert_eq!(del("l([1, 2, 3])", p!["l", 1usize]), "l([1, 3])");
    assert_eq!(del("l([1, 2, 3])", p!["l", 2usize]), "l([1, 2])");
    assert_eq!(del("l([1])", p!["l", 0usize]), "l([])");
    assert_eq!(del("l(1, 2)", p!["l", 0usize]), "l([2])");
    let mut d = Document::new(EDITED);
    assert!(!d.delete_in(&p!["nope"], false).unwrap());
    assert_eq!(d.text(), EDITED);
}

#[test]
fn a_document_that_does_not_read_records_the_error_and_refuses_edits() {
    let mut d = Document::new("a(");
    assert_eq!(d.errors().len(), 1);
    assert!(d.set_in(&p!["a"], &Value::Num(1.0), PLAIN).is_err());
}

#[test]
fn a_document_reads_values_at_paths() {
    let d = Document::new(EDITED);
    assert_eq!(
        d.get_in(&p!["params", "Level"]).unwrap(),
        Some(Value::Num(-12.0))
    );
    assert!(!d.has_in(&p!["params", "Nope"]).unwrap());
    assert_eq!(
        d.to_js().unwrap().get("automations").unwrap(),
        &Value::List(vec![])
    );
}
