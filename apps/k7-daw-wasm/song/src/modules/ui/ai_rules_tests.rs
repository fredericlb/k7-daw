// ai_rules_tests.rs — each structural rule of the AI check on its own widgets; the cases are the old TypeScript tests' inputs.

use super::*;
use crate::modules::ui::geometry::Pairs;
use serde_json::json;

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

fn knob(bind: &str, pair: Option<&str>) -> Value {
    let mut w = json!({ "type": "knob", "col": 0, "row": 0, "bind": bind });
    if let Some(p) = pair {
        w["pair"] = json!(p);
    }
    w
}

fn named(list: &[(&str, Value)]) -> Vec<(String, Value)> {
    list.iter().map(|(id, w)| ((*id).to_owned(), w.clone())).collect()
}

fn pairs(owned: &[(String, Value)]) -> Pairs<'_> {
    owned.iter().map(|(id, w)| (id, w)).collect()
}

#[test]
fn the_crossfade_check_takes_one_knob_and_asks_nothing_of_a_module_with_a_mix() {
    let names_dw = names(&["@pan", "Delay", "Dry Level", "Wet Level"]);
    let apart = named(&[("dry", knob("Dry Level", None)), ("wet", knob("Wet Level", None))]);
    assert_eq!(
        crossfade_issues(&pairs(&apart), &names_dw),
        vec!["`dry` and `wet` set `Dry Level` and `Wet Level` apart: blend them with one crossfade knob — `bind: Wet Level, pair: Dry Level`".to_owned()]
    );
    let right = named(&[("mix", knob("Wet Level", Some("Dry Level")))]);
    assert!(crossfade_issues(&pairs(&right), &names_dw).is_empty());
    let wrong = named(&[("mix", knob("Dry Level", Some("Wet Level")))]);
    assert_eq!(crossfade_issues(&pairs(&wrong), &names_dw).len(), 1);
    for mix in ["Mix", "Dry/Wet"] {
        let mut with = names_dw.clone();
        with.push(mix.to_owned());
        assert!(crossfade_issues(&pairs(&apart), &with).is_empty(), "{mix}");
    }
}

#[test]
fn a_group_frames_a_section_never_one_widget() {
    let k = |col: u32| json!({ "type": "knob", "col": col, "row": 0, "colSpan": 2, "rowSpan": 2 });
    let g = |span: u32| json!({ "type": "group", "col": 0, "row": 0, "colSpan": span, "rowSpan": 2 });
    let lone = named(&[("g", g(2)), ("a", k(0))]);
    assert_eq!(
        lone_group_issues(&pairs(&lone)),
        vec![
            "group `g` frames a single widget: a group holds two or more — drop it and leave the widget bare"
                .to_owned()
        ]
    );
    let empty = named(&[("g", g(2))]);
    assert_eq!(
        lone_group_issues(&pairs(&empty)),
        vec!["group `g` frames nothing: a group holds two or more — drop it".to_owned()]
    );
    let section = named(&[("g", g(4)), ("a", k(0)), ("b", k(2))]);
    assert!(lone_group_issues(&pairs(&section)).is_empty());
}

#[test]
fn the_engines_blend_leaves_the_plugins_own_mix_dry_and_wet_off() {
    let footer = named(&[("mix", knob("@mix", None))]);
    assert!(blend_issues(&pairs(&footer)).is_empty());
    let own = named(&[
        ("m", knob("Mix", None)),
        ("d", knob("Dry Level", None)),
        ("x", knob("Wet Level", Some("Dry Level"))),
    ]);
    let issues = blend_issues(&pairs(&own));
    assert_eq!(issues.len(), 3);
    assert_eq!(
        issues[0],
        "`m` blends with the plugin's own `Mix`: the blend is the engine's `@mix`, drawn in the card's footer — leave it off the panel"
    );
    assert!(
        issues[2].contains("blends with the plugin's own dry and wet"),
        "{}",
        issues[2]
    );
}

#[test]
fn the_engines_levels_are_the_footers_and_never_a_control_on_the_panel() {
    let binds = names(&["Input", "Output", "Drive", "@input-volume", "@output-volume"]);
    let own = named(&[
        ("i", knob("Input", None)),
        ("o", knob("Output", None)),
        ("d", knob("Drive", None)),
    ]);
    assert_eq!(
        level_issues(&pairs(&own), &binds),
        vec![
            "`i` binds the plugin's own `Input`: the level in is the engine's `@input-volume`, drawn in the card's footer — leave it off the panel".to_owned(),
            "`o` binds the plugin's own `Output`: the level out is the engine's `@output-volume`, drawn in the card's footer — leave it off the panel".to_owned(),
        ]
    );
    let alone = named(&[("o", knob("Output", None))]);
    assert!(level_issues(&pairs(&alone), &names(&["Output"])).is_empty());
    let at = named(&[("in", knob("@input-volume", None))]);
    assert_eq!(
        at_control_issues(&pairs(&at)),
        vec![
            "`in` puts a knob on `@input-volume`: k7 draws that in the card's footer — leave `@` params off the panel"
                .to_owned()
        ]
    );
    // A dropdown on `@base` is not a level: it picks the preset the plugin is built from.
    let base = named(&[("b", json!({ "type": "dropdown", "col": 0, "row": 0, "bind": "@base" }))]);
    assert!(at_control_issues(&pairs(&base)).is_empty());
}

#[test]
fn an_eq_band_is_a_fader_whatever_the_plugin_calls_it() {
    for bind in ["Low", "Mid gain 1", "High-shelf gain", "Presnce", "Bass", "Treble"] {
        let w = named(&[("b", knob(bind, None))]);
        assert_eq!(eq_band_issues(&pairs(&w)).len(), 1, "{bind}");
    }
    let fader = named(&[("b", json!({ "type": "fader", "col": 0, "row": 0, "bind": "Low" }))]);
    assert!(eq_band_issues(&pairs(&fader)).is_empty());
    for bind in ["Drive", "Low Cut", "High Freq", "Cutoff"] {
        let w = named(&[("b", knob(bind, None))]);
        assert!(eq_band_issues(&pairs(&w)).is_empty(), "{bind}");
    }
}
