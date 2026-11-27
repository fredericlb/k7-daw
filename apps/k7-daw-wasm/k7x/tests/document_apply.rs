// document_apply.rs — the header, the change log, a scoped view and apply_in's fewest edits.
// A reader would get wrong that these are hand-written expectations: the frozen vectors (edit_vectors.rs) are the machine-made ones.

#[macro_use]
mod common;

use common::{PLAIN, lines, set, v};
use k7x::edit::{Document, PathSeg, Value};

const HEADED: &str = "// synth-pad\n//\n// a pad\nextends(\"@workspace/instruments/synth\")\nparams({\n  \"Filter Freq\": 40, // darker than the lead\n  Level: -12,\n})\n";
const HEAD: &str = "// synth-pad\n//\n// a pad\n";

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
