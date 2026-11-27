// document_api.rs — the document editor's exact texts, header, change log and scoped views: what the page's tests pinned over the module.
// A reader would get wrong that these are hand-written expectations: the frozen vectors (edit_vectors.rs) are the machine-made ones.

mod common;

use common::{parse_json, value_of};
use k7x::edit::{Document, PathSeg, StringifyOptions, Value, WriteOptions, parse, stringify};

trait Seg {
    fn seg(self) -> PathSeg;
}
impl Seg for &str {
    fn seg(self) -> PathSeg {
        PathSeg::Key(self.to_owned())
    }
}
impl Seg for usize {
    fn seg(self) -> PathSeg {
        PathSeg::Index(self)
    }
}
macro_rules! p {
    ($($x:expr),* $(,)?) => { vec![$(Seg::seg($x)),*] };
}

fn v(json: &str) -> Value {
    value_of(&parse_json(json))
}

fn edit(text: &str, f: impl FnOnce(&mut Document)) -> String {
    let mut d = Document::new(text);
    f(&mut d);
    d.text().to_owned()
}

const INLINE: WriteOptions = WriteOptions { inline: true };
const PLAIN: WriteOptions = WriteOptions { inline: false };

fn set(d: &mut Document, path: &[PathSeg], value: &str) {
    d.set_in(path, &v(value), PLAIN).unwrap();
}

fn data(text: &str) -> Value {
    parse(text).unwrap()
}

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

const HEADED: &str = "// synth-pad\n//\n// a pad\nextends(\"@workspace/instruments/synth\")\nparams({\n  \"Filter Freq\": 40, // darker than the lead\n  Level: -12,\n})\n";
const HEAD: &str = "// synth-pad\n//\n// a pad\n";

fn lines(l: &[&str]) -> Vec<String> {
    l.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn the_header_is_the_comment_lines_the_file_starts_with() {
    assert_eq!(
        Document::new(HEADED).header(),
        lines(&["synth-pad", "", "a pad"])
    );
    assert!(Document::new("a(1) // x\n").header().is_empty());
    assert!(Document::new("\n// late\na(1)").header().is_empty());
}

#[test]
fn the_header_is_replaced_added_and_removed_the_rest_kept() {
    let mut d = Document::new(HEADED);
    d.set_header(&lines(&["synth pad", "v2"])).unwrap();
    assert_eq!(d.text(), HEADED.replace(HEAD, "// synth pad\n// v2\n"));
    assert_eq!(d.header(), lines(&["synth pad", "v2"]));
    d.set_header(&[]).unwrap();
    assert_eq!(d.text(), HEADED.replace(HEAD, ""));
    d.set_header(&lines(&["", "back"])).unwrap();
    assert_eq!(d.text(), HEADED.replace(HEAD, "//\n// back\n"));
}

#[test]
fn what_set_header_writes_the_header_reads_back_and_the_data_is_untouched() {
    for l in [&["a"][..], &["", "b", ""], &["x  y", "// nested"]] {
        let mut d = Document::new("a(1)\n");
        d.set_header(&lines(l)).unwrap();
        assert_eq!(d.header(), lines(l));
        assert_eq!(d.to_js().unwrap(), v(r#"{"a":1}"#));
    }
}

#[test]
fn a_header_goes_on_an_empty_file_and_on_a_file_that_is_only_a_header() {
    let mut d = Document::new("");
    d.set_header(&lines(&["new"])).unwrap();
    assert_eq!(d.text(), "// new\n");
    let mut only = Document::new("// old");
    only.set_header(&lines(&["new"])).unwrap();
    assert_eq!(only.text(), "// new");
}

#[test]
fn a_header_line_holding_a_newline_is_several_comment_lines_never_data() {
    let mut d = Document::new("a(1)\n");
    d.set_header(&lines(&["x\nb(2)"])).unwrap();
    assert_eq!(d.to_js().unwrap(), v(r#"{"a":1}"#));
    assert_eq!(d.header(), lines(&["x", "b(2)"]));
}

#[test]
fn a_header_goes_after_a_bom_and_keeps_an_authors_spacing_when_the_lines_read_the_same() {
    let mut d = Document::new("\u{feff}//a\r\n// b\r\na(1)\r\n");
    assert_eq!(d.header(), lines(&["a", "b"]));
    d.set_header(&lines(&["a", "b"])).unwrap();
    assert!(d.changes().is_empty());
    d.set_header(&lines(&["c"])).unwrap();
    assert_eq!(d.text(), "\u{feff}// c\na(1)\r\n");
}

#[test]
fn position_of_gives_a_keys_or_an_items_first_character_one_based() {
    let d = Document::new(HEADED);
    assert_eq!(d.position_of(&p!["extends"]).unwrap(), Some((4, 1)));
    assert_eq!(d.position_of(&p!["params", "Level"]).unwrap(), Some((7, 3)));
    assert_eq!(
        d.position_of(&p!["params", "Filter Freq"]).unwrap(),
        Some((6, 3))
    );
    assert_eq!(d.position_of(&[]).unwrap(), Some((1, 1)));
    assert_eq!(d.position_of(&p!["nope"]).unwrap(), None);
    let list = Document::new("l([\n  1,\n  2,\n])");
    assert_eq!(list.position_of(&p!["l", 1usize]).unwrap(), Some((3, 3)));
}

fn replay(text: &str, d: &Document) -> String {
    let mut units: Vec<u16> = text.encode_utf16().collect();
    for c in d.changes() {
        units.splice(c.start16..c.end16, c.insert.encode_utf16());
    }
    String::from_utf16(&units).unwrap()
}

#[test]
fn the_changes_replay_every_edit_in_order_onto_the_original_text() {
    let mut d = Document::new(HEADED);
    set(&mut d, &p!["params", "Level"], "-6");
    set(&mut d, &p!["params", "Detune"], "3");
    d.delete_in(&p!["params", "Filter Freq"], false).unwrap();
    set(&mut d, &p!["bypass"], "true");
    d.rename_key(&p!["extends"], "base").unwrap();
    d.set_header(&lines(&["renamed"])).unwrap();
    d.apply_in(&p!["tags"], &v(r#"["a","b"]"#)).unwrap();
    d.move_in(&p!["tags"], 1, 0).unwrap();
    d.set_source_in(&p!["params", "Level"], "-3").unwrap();
    assert!(d.changes().len() >= 9);
    assert_eq!(replay(HEADED, &d), d.text());
}

#[test]
fn an_edit_that_changes_nothing_or_is_refused_records_nothing() {
    let mut d = Document::new(HEADED);
    set(&mut d, &p!["params", "Level"], "-12");
    assert!(d.set_source_in(&p!["params", "Level"], "(").is_err());
    assert!(d.changes().is_empty());
}

#[test]
fn a_whole_file_write_is_one_splice() {
    let mut d = Document::new(HEADED);
    set(&mut d, &[], r#"{"a":1}"#);
    assert_eq!(d.changes().len(), 1);
    let c = &d.changes()[0];
    assert_eq!(
        (c.start16, c.end16, c.insert.as_str()),
        (0, HEADED.encode_utf16().count(), "a(1)\n")
    );
}

const SCOPED: &str = "graph({\n  osc: { type: \"saw\", level: 1 }, // the source\n})\nsteps([\n  id(1).x(2), // a chain\n])\ntempo(120)\n";

#[test]
fn a_scoped_view_reads_relative_to_an_object_base() {
    let mut d = Document::new(SCOPED);
    let s = d.at(&p!["graph", "osc"]);
    assert_eq!(
        s.get_in(&p!["type"]).unwrap(),
        Some(Value::Str("saw".into()))
    );
    assert!(s.has_in(&p!["level"]).unwrap());
    assert!(!s.has_in(&p!["nope"]).unwrap());
    assert!(
        s.node_at(&p!["level"], |n| matches!(
            n,
            Some(k7x::edit::Tree::Scalar { .. })
        ))
        .unwrap()
    );
    assert_eq!(
        s.get_source(&p!["type"]).unwrap().as_deref(),
        Some("\"saw\"")
    );
    assert_eq!(s.to_js().unwrap(), Some(v(r#"{"type":"saw","level":1}"#)));
}

#[test]
fn a_scoped_view_edits_an_object_base_comments_kept() {
    let mut d = Document::new(SCOPED);
    let mut s = d.at(&p!["graph", "osc"]);
    s.set_in(&p!["level"], &Value::Num(0.5), PLAIN).unwrap();
    s.add_in(&p!["tags"], &Value::Str("warm".into()), PLAIN)
        .unwrap();
    s.rename_key(&p!["type"], "wave").unwrap();
    s.delete_in(&p!["tags"], false).unwrap();
    assert_eq!(
        d.text(),
        SCOPED.replace("type: \"saw\", level: 1", "wave: \"saw\", level: 0.5")
    );
}

#[test]
fn a_scoped_view_edits_a_chain_base() {
    let mut d = Document::new(SCOPED);
    let mut s = d.at(&p!["steps", 0usize]);
    s.set_in(&p!["y"], &Value::Num(3.0), PLAIN).unwrap();
    s.apply_in(&p!["x"], &Value::Num(4.0)).unwrap();
    s.rename_key(&p!["id"], "n").unwrap();
    assert_eq!(s.to_js().unwrap(), Some(v(r#"{"n":1,"x":4,"y":3}"#)));
    s.delete_in(&p!["y"], false).unwrap();
    assert_eq!(s.to_js().unwrap(), Some(v(r#"{"n":1,"x":4}"#)));
}

#[test]
fn a_scoped_view_edits_the_root_as_a_base_and_a_list_through_insert_in() {
    let mut d = Document::new(SCOPED);
    {
        let mut root = d.at(&[]);
        root.set_in(&p!["tempo"], &Value::Num(90.0), PLAIN).unwrap();
        root.insert_in(&p!["steps"], 0, &v(r#"{"id":0}"#), PLAIN)
            .unwrap();
    }
    let mut whole = d.to_js().unwrap();
    if let Value::Map(m) = &mut whole {
        m.push(("swing".into(), Value::Num(0.1)));
    }
    d.at(&[]).apply_in(&[], &whole).unwrap();
    assert_eq!(
        d.text(),
        SCOPED
            .replace("tempo(120)", "tempo(90)\nswing(0.1)")
            .replace("  id(1)", "  id(0),\n  id(1)")
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

const MELODY: &str = "// the melody\nnotes([\n  t(0).d(1).k(60),\n  t(1).d(1).k(62), // the second\n  t(2).d(1).k(64),\n])\nparams({ gain: 1, pan: 0 }) // mix\n";

fn applied(path: &[PathSeg], value: &Value) -> String {
    let mut d = Document::new(MELODY);
    d.apply_in(path, value).unwrap();
    assert_eq!(d.get_in(path).unwrap().as_ref(), Some(value));
    d.text().to_owned()
}

fn note(t: f64, d: f64, k: f64) -> String {
    format!(r#"{{"t":{t},"d":{d},"k":{k}}}"#)
}

#[test]
fn apply_in_changes_nothing_when_equal_and_a_changed_leaf_only_otherwise() {
    let mut d = Document::new(MELODY);
    let all = d.to_js().unwrap();
    d.apply_in(&[], &all).unwrap();
    assert_eq!(d.text(), MELODY);
    assert!(d.changes().is_empty());
    let changed = v(&format!(
        r#"{{"notes":[{},{},{}],"params":{{"gain":1,"pan":0}}}}"#,
        note(0.0, 1.0, 60.0),
        note(1.0, 1.0, 63.0),
        note(2.0, 1.0, 64.0)
    ));
    assert_eq!(applied(&[], &changed), MELODY.replace("k(62)", "k(63)"));
    assert_eq!(
        applied(&p!["params"], &v(r#"{"gain":1,"width":2}"#)),
        MELODY.replace("{ gain: 1, pan: 0 }", "{ gain: 1, width: 2 }")
    );
}

#[test]
fn apply_in_splices_a_lists_middle_head_and_tail_kept_as_written() {
    let list = |ns: &[String]| v(&format!("[{}]", ns.join(",")));
    let (n0, n1, n15, n2) = (
        note(0.0, 1.0, 60.0),
        note(1.0, 1.0, 62.0),
        note(1.5, 1.0, 63.0),
        note(2.0, 1.0, 64.0),
    );
    assert_eq!(
        applied(
            &p!["notes"],
            &list(&[n0.clone(), n1.clone(), n15, n2.clone()])
        ),
        MELODY.replace(
            "  t(2).d(1).k(64),",
            "  t(1.5).d(1).k(63),\n  t(2).d(1).k(64),"
        )
    );
    assert_eq!(
        applied(&p!["notes"], &list(&[n0, n2])),
        MELODY.replace("  t(1).d(1).k(62), // the second\n", "")
    );
    let emptied = applied(&p!["notes"], &list(&[]));
    assert_eq!(
        emptied,
        MELODY.replace(
            "notes([\n  t(0).d(1).k(60),\n  t(1).d(1).k(62), // the second\n  t(2).d(1).k(64),\n])",
            "notes([])"
        )
    );
}

#[test]
fn apply_in_edits_an_item_in_place_beside_a_splice_keeping_the_comments_of_the_items_it_keeps() {
    let notes = v(&format!(
        "[{},{},{},{}]",
        note(0.0, 1.0, 60.0),
        note(1.0, 0.5, 62.0),
        note(2.0, 1.0, 64.0),
        note(1.5, 0.5, 62.0)
    ));
    assert_eq!(
        applied(&p!["notes"], &notes),
        MELODY
            .replace(
                "t(1).d(1).k(62), // the second",
                "t(1).d(0.5).k(62), // the second"
            )
            .replace(
                "  t(2).d(1).k(64),",
                "  t(2).d(1).k(64),\n  t(1.5).d(0.5).k(62),"
            )
    );
}

#[test]
fn apply_in_creates_a_missing_path_and_replaces_a_value_of_another_shape() {
    assert_eq!(
        applied(&p!["extra", "x"], &v("[1]")),
        format!("{MELODY}extra({{ x: [1] }})\n")
    );
    assert_eq!(
        applied(&p!["params"], &Value::Num(3.0)),
        MELODY.replace("{ gain: 1, pan: 0 }", "3")
    );
}

#[test]
fn apply_in_compares_keys_named_like_object_members() {
    let mut d = Document::new("a(1).b(2)\ntoString(3)\nconstructor(4)\n");
    d.apply_in(&[], &v(r#"{"b":2,"valueOf":5}"#)).unwrap();
    assert_eq!(d.to_js().unwrap(), v(r#"{"b":2,"valueOf":5}"#));
}

#[test]
fn stringify_prints_plain_with_no_options_and_writes_the_header_first() {
    let opts = StringifyOptions::default();
    assert_eq!(
        stringify(&v(r#"{"a":1,"b":[1,2]}"#), &opts).unwrap(),
        "a(1)\nb([1, 2])\n"
    );
    let header = lines(&["synth pad", "", "v2"]);
    let with = StringifyOptions {
        header: &header,
        inline: None,
    };
    let text = stringify(&v(r#"{"a":1}"#), &with).unwrap();
    assert_eq!(text, "// synth pad\n//\n// v2\na(1)\n");
    assert_eq!(data(&text), v(r#"{"a":1}"#));
    let only = lines(&["only"]);
    let alone = StringifyOptions {
        header: &only,
        inline: None,
    };
    assert_eq!(stringify(&v("{}"), &alone).unwrap(), "// only\n");
}

fn wide_steps() -> Value {
    let steps: Vec<String> = (0..12)
        .map(|i| format!(r#"{{"t":{i},"d":1,"k":{}}}"#, 60 + i))
        .collect();
    v(&format!(
        r#"{{"steps":[{}],"params":{{"a":1}}}}"#,
        steps.join(",")
    ))
}

#[test]
fn stringify_keeps_the_nodes_inline_names_on_one_line_whatever_their_width() {
    let value = wide_steps();
    assert!(
        stringify(&value, &StringifyOptions::default())
            .unwrap()
            .split('\n')
            .count()
            > 5
    );
    let inline =
        |path: &[PathSeg], _: &Value| path.len() == 1 && path[0] == PathSeg::Key("steps".into());
    let opts = StringifyOptions {
        header: &[],
        inline: Some(&inline),
    };
    let text = stringify(&value, &opts).unwrap();
    assert!(text.split('\n').next().unwrap().len() > 100);
    assert!(text.starts_with("steps([t(0).d(1).k(60), "));
    assert_eq!(data(&text), value);
}

#[test]
fn stringify_hands_inline_each_nodes_path_and_value() {
    use std::cell::RefCell;
    let seen = RefCell::new(Vec::new());
    let inline = |path: &[PathSeg], value: &Value| {
        let p: Vec<String> = path
            .iter()
            .map(|s| match s {
                PathSeg::Key(k) => k.clone(),
                PathSeg::Index(i) => i.to_string(),
            })
            .collect();
        seen.borrow_mut().push((p.join("."), value.clone()));
        false
    };
    let opts = StringifyOptions {
        header: &[],
        inline: Some(&inline),
    };
    stringify(&v(r#"{"a":{"b":[1]}}"#), &opts).unwrap();
    let keys: Vec<String> = seen.borrow().iter().map(|(p, _)| p.clone()).collect();
    assert_eq!(keys, ["", "a", "a.b", "a.b.0"]);
    assert_eq!(seen.borrow()[0].1, v(r#"{"a":{"b":[1]}}"#));
    assert_eq!(seen.borrow()[3].1, Value::Num(1.0));
}

#[test]
fn numbers_keep_an_integer_past_32_bits_minus_zero_and_an_overflow() {
    let d = data("a(5000000000)\nb(-123456789012)\nc(-0)\nd(1e999)\ne(0.1)");
    assert_eq!(d.get("a"), Some(&Value::Num(5e9)));
    assert_eq!(d.get("b"), Some(&Value::Num(-123456789012.0)));
    assert!(matches!(d.get("c"), Some(Value::Num(n)) if *n == 0.0 && n.is_sign_negative()));
    assert_eq!(d.get("d"), Some(&Value::Num(f64::INFINITY)));
    assert_eq!(d.get("e"), Some(&Value::Num(0.1)));
}

#[test]
fn a_file_nested_past_the_limit_is_a_syntax_error_not_a_stack_overflow() {
    let deep = format!("a({}{})", "[".repeat(100_000), "]".repeat(100_000));
    let e = parse(&deep).unwrap_err().to_string();
    assert!(e.contains("nested deeper than 256"), "{e}");
    let objects = format!("a({}1{})", "{ b: ".repeat(300), " }".repeat(300));
    assert!(parse(&objects).is_err());
    assert!(
        parse(&format!("a({}{})", "[".repeat(200), "]".repeat(200)))
            .unwrap()
            .get("a")
            .is_some()
    );
}

#[test]
fn the_grammar_reads_statements_chains_objects_and_lists_as_one_map() {
    let text = "// a comment\nproject({ name: \"demo\", tempo: 120, video: false })\nsteps([\n  extends(\"@k7-core/audio/compressor\").engine('tracktion'),\n  to([chain(\"@out\")]), /* trailing comma */\n])\nmarkers({})";
    assert_eq!(
        data(text),
        v(
            r#"{"project":{"name":"demo","tempo":120,"video":false},"steps":[{"extends":"@k7-core/audio/compressor","engine":"tracktion"},{"to":[{"chain":"@out"}]}],"markers":{}}"#
        )
    );
    assert_eq!(
        data("a(1)\n  .b(2)\nc(3).d(4)"),
        v(r#"{"a":1,"b":2,"c":3,"d":4}"#)
    );
    assert_eq!(
        data("a() b(1, 2) c([1]) d(null)"),
        v(r#"{"a":true,"b":[1,2],"c":[1],"d":null}"#)
    );
    assert_eq!(
        data("\"Filter Freq\"(40) synth-pad([\"MIX\"]) x({ \"pad#midi\": 1, true: 2 })"),
        v(r##"{"Filter Freq":40,"synth-pad":["MIX"],"x":{"pad#midi":1,"true":2}}"##)
    );
    assert_eq!(data(""), v("{}"));
    assert_eq!(data("// only a comment\n"), v("{}"));
}

#[test]
fn strings_read_escapes_single_quotes_and_raw_multi_line_backticks() {
    let raw = "`line 1\n  \\` ${x} \\\\`";
    let read = "line 1\n  ` ${x} \\";
    let d = data(&format!("a(\"é\\n\\u0041\") b('it\\'s') c({raw})"));
    assert_eq!(d.get("a"), Some(&Value::Str("é\nA".into())));
    assert_eq!(d.get("b"), Some(&Value::Str("it's".into())));
    assert_eq!(d.get("c"), Some(&Value::Str(read.into())));
}

#[test]
fn a_mistake_is_rejected_with_its_sentence_and_reported_at_its_line_and_column() {
    for (text, message) in [
        ("a(1", "expected \",\" or \")\""),
        ("a(foo)", "bare word \"foo\""),
        ("a(1) a(2)", "duplicate key \"a\""),
        ("a({ b 1 })", "expected \":\" after b"),
        ("a(\"x\ny\")", "unterminated string"),
        ("a(01)", "expected \",\" or \")\""),
    ] {
        let e = parse(text).unwrap_err().to_string();
        assert!(e.contains(message), "{text:?}: {e}");
    }
    let e = parse("a(1)\nb(2,, 3)").unwrap_err();
    assert_eq!((e.line, e.column), (2, 5));
}

#[test]
fn stringify_reads_back_what_it_writes_in_the_canonical_layout() {
    let notes: Vec<String> = (0..3)
        .map(|i| format!(r#"{{"t":{i},"d":1,"k":{}}}"#, 60 + i))
        .collect();
    let value = v(&format!(
        r#"{{"project":{{"name":"synth-demo","tempo":120,"sampleRate":48000,"timeSignature":"4/4","video":false,"layers":[]}},"markers":{{}},"notes":[{}],"params":{{"Filter Freq":40,"Level":-12.5}},"content":"two\nlines with a ` and a \\","steps":[{{"extends":"@k7-core/audio/compressor","engine":"tracktion","engines":{{"tracktion":{{"params":{{"Ratio":0.125,"Attack":12,"Release":140,"Knee":3,"Makeup":0}}}}}}}},{{"to":[{{"chain":"@out"}}]}}]}}"#,
        notes.join(",")
    ));
    let text = stringify(&value, &StringifyOptions::default()).unwrap();
    assert_eq!(data(&text), value);
    let want = "project({\n  name: \"synth-demo\",\n  tempo: 120,\n  sampleRate: 48000,\n  timeSignature: \"4/4\",\n  video: false,\n  layers: [],\n})\nmarkers({})\nnotes([t(0).d(1).k(60), t(1).d(1).k(61), t(2).d(1).k(62)])\nparams({ \"Filter Freq\": 40, Level: -12.5 })\ncontent(`two\nlines with a \\` and a \\\\`)\nsteps([\n  extends(\"@k7-core/audio/compressor\")\n    .engine(\"tracktion\")\n    .engines({\n      tracktion: { params: { Ratio: 0.125, Attack: 12, Release: 140, Knee: 3, Makeup: 0 } },\n    }),\n  to([chain(\"@out\")]),\n])\n";
    assert_eq!(text, want);
}
