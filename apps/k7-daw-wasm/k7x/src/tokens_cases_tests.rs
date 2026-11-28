// tokens_cases_tests.rs — the values with their fields and the ▶ lines, case by case; the cases are the old TypeScript tests' inputs (entityValues, previewTriggers).

use super::*;
use crate::{Bump, parse_tree};

fn pairs(text: &str) -> Vec<String> {
    let bump = Bump::new();
    let Ok(root) = parse_tree(text, &bump) else {
        return Vec::new();
    };
    value_tokens(&root, text)
        .iter()
        .map(|t| format!("{}={}", t.field.as_deref().unwrap_or(""), t.value))
        .collect()
}

fn triggers(text: &str) -> Vec<(usize, PreviewReq)> {
    let bump = Bump::new();
    let root = parse_tree(text, &bump).unwrap();
    let line = |from: usize| text[..from].matches('\n').count() + 1;
    let mut v: Vec<_> = preview_triggers(&root, text)
        .into_iter()
        .map(|t| (line(t.from), t.req))
        .collect();
    v.sort_by_key(|(l, _)| *l);
    v
}

fn strs(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn a_flat_file_reads_each_value_under_its_call() {
    assert_eq!(
        pairs("extends(\"audio/verb\")\nir(\"ir/marshall.wav\")\n"),
        strs(&["extends=audio/verb", "ir=ir/marshall.wav"])
    );
}

#[test]
fn a_key_is_never_taken_for_a_value() {
    // `module("module")` would otherwise link the wrong half of the line.
    assert_eq!(pairs("module(\"module\")\n"), strs(&["module=module"]));
    let text = "module(\"module\")\n";
    let bump = Bump::new();
    let root = parse_tree(text, &bump).unwrap();
    let t = &value_tokens(&root, text)[0];
    // The span keeps its quotes: the page takes them off once it has mapped it.
    assert_eq!((t.from, t.quoted), ("module(".len(), true));
}

#[test]
fn a_list_hands_its_field_to_each_value_in_either_call_form() {
    assert_eq!(pairs("in([\"pad\", \"bass\"])\n"), strs(&["in=pad", "in=bass"]));
    assert_eq!(pairs("in(\"pad\", \"bass\")\n"), strs(&["in=pad", "in=bass"]));
}

#[test]
fn a_chain_inside_a_list_reads_with_the_colons_in_its_values() {
    assert_eq!(
        pairs("chains([[track(\"melody\").as(\"lead\"), module(\"synth:lead\")]])\n"),
        strs(&["track=melody", "as=lead", "module=synth:lead"])
    );
}

#[test]
fn a_nested_value_belongs_to_its_own_key_not_its_parents() {
    assert_eq!(
        pairs("ui({ widgets: { w1: { type: \"knob\", bind: \"Depth\" } } })\n"),
        strs(&["type=knob", "bind=Depth"])
    );
}

#[test]
fn comments_numbers_and_empty_strings_are_no_value() {
    assert_eq!(
        pairs("// module(\"not-a-value\")\nir(\"ir/a.wav\")\ngain(3)\nx(\"\")\n"),
        strs(&["ir=ir/a.wav"])
    );
}

#[test]
fn a_buffer_that_never_parsed_answers_nothing() {
    assert!(pairs("module(\"reverb\"\n").is_empty());
}

#[test]
fn a_map_that_is_no_symbol_earns_no_play_button() {
    // A widget has `col`/`row` numbers and no `note`.
    assert!(
        triggers("ui({ widgets: { w1: { type: \"knob\", col: 0, row: 0, min: 0, max: 1 } } })\n")
            .is_empty()
    );
}

#[test]
fn a_slice_symbol_with_no_source_anywhere_earns_nothing() {
    assert!(triggers("symbols({ BD: { note: 36, from: 0, to: 0.5 } })\n").is_empty());
}

#[test]
fn a_symbols_own_clip_wins_over_the_racks_source() {
    let text = "rack({\n  source: \"break.wav\",\n  symbols: { BD: { note: 36, clip: \"kick\", from: 0, to: 1 } },\n})\n";
    let got = triggers(text);
    let at3 = got.iter().find(|(l, _)| *l == 3).map(|(_, r)| r.clone());
    assert_eq!(
        at3,
        Some(PreviewReq::Clip {
            clip: "kick".into()
        })
    );
}
