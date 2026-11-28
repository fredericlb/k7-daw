// document_source.rs — the source of a node read and replaced, items moved, keys renamed, inline layout.
// A reader would get wrong that these are hand-written expectations: the frozen vectors (edit_vectors.rs) are the machine-made ones.

#[macro_use]
mod common;

use common::{INLINE, PLAIN, edit, set, v};
use k7x::edit::{Document, Value};

const NODE: &str = "// synth-pad\nextends(\"@workspace/instruments/synth\")\nparams({\n  \"Filter Freq\": 40, // darker than the lead\n  Level: -12,\n})\nnotes([\n  t(0).k(60), // first\n  t(1).k(62),\n  t(2).k(64),\n])\n";

#[test]
fn a_nodes_exact_source_is_read_and_replaced_as_written() {
    let d = Document::new(NODE);
    assert_eq!(
        d.get_source(&p!["notes", 1usize]).unwrap().as_deref(),
        Some("t(1).k(62)")
    );
    assert!(
        d.get_source(&p!["params"])
            .unwrap()
            .unwrap()
            .contains("// darker than the lead")
    );
    assert_eq!(d.get_source(&p!["nope"]).unwrap(), None);
    assert_eq!(
        Document::new("a() b(1, 2)")
            .get_source(&p!["b"])
            .unwrap()
            .as_deref(),
        Some("1, 2")
    );
    assert_eq!(
        edit(NODE, |d| d
            .set_source_in(&p!["params", "Level"], "-6 /* dB */")
            .unwrap()),
        NODE.replace("Level: -12,", "Level: -6 /* dB */,")
    );
    assert_eq!(
        edit("a(1)\n", |d| d
            .set_source_in(&p!["b", "c"], "x(1).y(2)")
            .unwrap()),
        "a(1)\nb({ c: x(1).y(2) })\n"
    );
    assert_eq!(
        edit("flag()", |d| d.set_source_in(&p!["flag"], "false").unwrap()),
        "flag(false)"
    );
}

#[test]
fn source_that_is_not_one_value_is_refused_and_the_text_kept() {
    let mut d = Document::new(NODE);
    assert!(d.set_source_in(&p!["params", "Level"], "1, 2").is_err());
    assert!(d.set_source_in(&p!["params", "Level"], "bare").is_err());
    assert_eq!(d.text(), NODE);
}

#[test]
fn list_items_are_inserted_from_source_on_a_line_of_their_own_where_items_have_one() {
    assert_eq!(
        edit(NODE, |d| d
            .insert_source_in(&p!["notes"], 1, "t(0.5).k(61)")
            .unwrap()),
        NODE.replace("  t(1).k(62),", "  t(0.5).k(61),\n  t(1).k(62),")
    );
    assert_eq!(
        edit("l([1, 3])", |d| d
            .insert_source_in(&p!["l"], 1, "2")
            .unwrap()),
        "l([1, 2, 3])"
    );
    assert_eq!(
        edit("l([1])", |d| d
            .insert_source_in(&p!["l"], 5, "{ a: 1 }")
            .unwrap()),
        "l([1, { a: 1 }])"
    );
}

#[test]
fn a_comment_ending_the_source_is_refused_since_it_would_swallow_its_line() {
    let mut d = Document::new("a([\n  0, 5,\n])\n");
    let e = d
        .set_source_in(&p!["a", 0usize], "1 // one")
        .unwrap_err()
        .to_string();
    assert!(e.contains("comment"), "{e}");
    let e = d
        .insert_source_in(&p!["a"], 1, "1 // one")
        .unwrap_err()
        .to_string();
    assert!(e.contains("comment"), "{e}");
    d.set_source_in(&p!["a", 0usize], "{\n  b: 1, // kept\n}")
        .unwrap();
    assert_eq!(d.get_in(&p!["a"]).unwrap().unwrap(), v(r#"[{"b":1},5]"#));
}

#[test]
fn an_item_moves_with_its_source_and_its_line_comment() {
    let out = edit(NODE, |d| d.move_in(&p!["notes"], 2, 0).unwrap());
    assert_eq!(
        Document::new(&out).get_in(&p!["notes"]).unwrap().unwrap(),
        v(r#"[{"t":2,"k":64},{"t":0,"k":60},{"t":1,"k":62}]"#)
    );
    assert!(out.contains("  t(2).k(64),\n  t(0).k(60), // first\n"));
    assert_eq!(
        edit("l([1, 2, 3])", |d| d.move_in(&p!["l"], 0, 2).unwrap()),
        "l([2, 3, 1])"
    );
    assert_eq!(
        edit("l(1, 2, 3)", |d| d.move_in(&p!["l"], 2, 0).unwrap()),
        "l(3, 1, 2)"
    );
    assert!(Document::new("l([1])").move_in(&p!["l"], 0, 1).is_err());
    assert_eq!(
        edit("l([\n  1, // one\n  2,\n  3\n])\n", |d| d
            .move_in(&p!["l"], 0, 2)
            .unwrap()),
        "l([\n  2,\n  3,\n  1, // one\n])\n"
    );
    assert_eq!(
        edit("l([\n  1,\n  2,\n  3, // three\n])\n", |d| d
            .move_in(&p!["l"], 2, 0)
            .unwrap()),
        "l([\n  3, // three\n  1,\n  2,\n])\n"
    );
}

#[test]
fn a_key_is_renamed_with_its_value_and_comment_kept() {
    assert_eq!(
        edit(NODE, |d| d
            .rename_key(&p!["params", "Filter Freq"], "Cutoff")
            .unwrap()),
        NODE.replace("\"Filter Freq\": 40", "Cutoff: 40")
    );
    assert_eq!(
        edit(NODE, |d| d.rename_key(&p!["extends"], "base").unwrap()),
        NODE.replace("extends(", "base(")
    );
    assert_eq!(
        edit(NODE, |d| d
            .rename_key(&p!["notes", 1usize, "k"], "key note")
            .unwrap()),
        NODE.replace("t(1).k(62)", "t(1).\"key note\"(62)")
    );
}

#[test]
fn a_rename_onto_an_existing_key_or_from_a_missing_entry_is_refused() {
    let mut d = Document::new(NODE);
    assert!(d.rename_key(&p!["params", "Level"], "Filter Freq").is_err());
    assert!(d.rename_key(&p!["params", "Nope"], "x").is_err());
    d.rename_key(&p!["params", "Level"], "Level").unwrap();
    assert_eq!(d.text(), NODE);
}

fn wide() -> Value {
    Value::List(
        (0..12)
            .map(|i| v(&format!(r#"{{"t":{i},"d":1,"k":60}}"#)))
            .collect(),
    )
}

#[test]
fn a_wide_value_breaks_by_default_and_stays_on_one_line_when_asked() {
    let broken = edit("a(1)\n", |d| d.set_in(&p!["s"], &wide(), PLAIN).unwrap());
    assert_eq!(broken.split('\n').count(), 16);
    let one = edit("a(1)\n", |d| d.set_in(&p!["s"], &wide(), INLINE).unwrap());
    assert_eq!(one.split('\n').count(), 3);
    assert_eq!(
        Document::new(&one).get_in(&p!["s"]).unwrap().unwrap(),
        wide()
    );
}

#[test]
fn inline_applies_to_add_in_and_insert_in() {
    let (a, b) = ("x".repeat(60), "y".repeat(60));
    let big = Value::object(vec![
        ("a".into(), Value::Str(a.clone())),
        ("b".into(), Value::Str(b.clone())),
    ]);
    let added = edit("l([\n  1,\n])", |d| {
        d.add_in(&p!["l"], &big, INLINE).unwrap()
    });
    assert_eq!(added, format!("l([\n  1,\n  a(\"{a}\").b(\"{b}\"),\n])"));
    let inserted = edit("l([\n  1,\n])", |d| {
        d.insert_in(&p!["l"], 0, &big, INLINE).unwrap()
    });
    assert_eq!(inserted.split('\n').count(), 4);
}

#[test]
fn a_string_of_digits_is_an_index_in_set_in_and_delete_in() {
    assert_eq!(
        edit("l([1, 2, 3])", |d| set(d, &p!["l", "1"], "5")),
        "l([1, 5, 3])"
    );
    assert_eq!(
        edit("l([1, 2, 3])", |d| assert!(
            d.delete_in(&p!["l", "1"], false).unwrap()
        )),
        "l([1, 3])"
    );
}

#[test]
fn writing_into_an_empty_container_holding_a_comment_keeps_the_comment() {
    assert_eq!(
        edit("x({\n  // params here\n})\n", |d| set(
            d,
            &p!["x", "a"],
            "1"
        )),
        "x({\n  // params here\n  a: 1,\n})\n"
    );
    assert_eq!(
        edit("x([\n  // items here\n])\n", |d| d
            .add_in(&p!["x"], &Value::Num(1.0), PLAIN)
            .unwrap()),
        "x([\n  // items here\n  1,\n])\n"
    );
    assert_eq!(
        edit("x([ /* none */ ])\n", |d| d
            .insert_in(&p!["x"], 0, &Value::Num(1.0), PLAIN)
            .unwrap()),
        "x([ /* none */ 1 ])\n"
    );
}
