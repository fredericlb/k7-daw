// auto_cases_tests.rs — what Auto organize proposes, case by case; the cases are the old TypeScript tests' inputs (autoLayout.test.ts).

use super::*;
use crate::files::validate_file;
use crate::modules::ui::geometry::twin_issues;
use crate::modules::ui::write::to_k7;
use k7x::edit::{StringifyOptions, stringify};

fn p(name: &str) -> AutoParam {
    AutoParam {
        name: name.to_owned(),
        meta: None,
    }
}

fn ranged(name: &str, min: f64, max: f64) -> AutoParam {
    AutoParam {
        name: name.to_owned(),
        meta: Some(AutoMeta {
            min: Some(min),
            max: Some(max),
            ..AutoMeta::default()
        }),
    }
}

fn states(name: &str, labels: &[(&str, i64)]) -> AutoParam {
    let all = labels.iter().map(|(l, v)| json!({ "label": l, "value": v })).collect();
    AutoParam {
        name: name.to_owned(),
        meta: Some(AutoMeta {
            min: Some(0.0),
            max: Some(1.0),
            states: Some(all),
            ..AutoMeta::default()
        }),
    }
}

fn knobs(n: usize, family: &str) -> Vec<AutoParam> {
    (0..n).map(|i| ranged(&format!("{family} {i}"), 0.0, 1.0)).collect()
}

fn plain(names: &[&str]) -> Vec<AutoParam> {
    names.iter().map(|n| p(n)).collect()
}

fn page_names(params: &[AutoParam]) -> Vec<String> {
    group_params(params).into_iter().map(|g| g.page).collect()
}

/// Does the Rust panel reader accept this panel?
fn accepted(ui: &Value) -> bool {
    let text = stringify(&to_k7(ui), &StringifyOptions::default()).unwrap();
    validate_file("modules/audio/x/ui.k7x", &text).ok
}

/// Every page as name → widgets: the tabs, or the panel itself when one family made no tabs.
fn pages_of(ui: &Value) -> Vec<(String, Vec<(String, Value)>)> {
    let list = |w: &Value| -> Vec<(String, Value)> {
        w.as_object()
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default()
    };
    match ui.get("tabs").and_then(Value::as_array) {
        Some(tabs) => tabs
            .iter()
            .map(|t| (t["name"].as_str().unwrap_or("").to_owned(), list(&t["widgets"])))
            .collect(),
        None => vec![(String::new(), list(&ui["widgets"]))],
    }
}

fn page(ui: &Value, name: &str) -> Vec<Value> {
    pages_of(ui)
        .into_iter()
        .filter(|(n, _)| ui.get("tabs").is_none() || n == name)
        .flat_map(|(_, w)| w.into_iter().map(|(_, v)| v))
        .collect()
}

fn binds(ui: &Value) -> Vec<String> {
    pages_of(ui)
        .into_iter()
        .flat_map(|(_, w)| w.into_iter().filter_map(|(_, v)| v["bind"].as_str().map(str::to_owned)))
        .collect()
}

fn bound<'a>(widgets: &'a [Value], bind: &str) -> &'a Value {
    widgets.iter().find(|w| w["bind"] == bind).unwrap()
}

#[test]
fn a_family_is_read_off_the_obvious_word_and_an_underscore_is_a_space() {
    for (name, family) in [
        ("Reverb Room Size", "Reverb"),
        ("Filter Cutoff", "Filter"),
        ("Osc 1 Wave", "Oscillator"),
        ("Master Volume", "Mix"),
        ("String Tension", "Other"),
        ("Hammer Hardness", "Other"),
        ("MPE_PitchBend_3", "MIDI"),
        ("Pitch Bend", "MIDI"),
    ] {
        assert_eq!(family_of(name), family, "{name}");
    }
}

#[test]
fn the_plugins_plumbing_is_named_a_family_and_kept_off_the_pages() {
    for n in ["VST_ProgramChange_7", "VST3_PitchBend", "MPE_Aftertouch_2"] {
        assert_eq!(family_of(n), "MIDI", "{n}");
    }
    for n in ["Select Preset", "Next Preset", "HardwareDisplayControl3"] {
        assert_eq!(family_of(n), "Preset", "{n}");
    }
}

#[test]
fn the_pages_keep_the_tables_order_with_the_leftovers_last() {
    let groups = plain(&["String Tension", "Master Volume", "Reverb Room Size"]);
    assert_eq!(page_names(&groups), ["Reverb", "Mix", "Other"]);
    assert_eq!(page_names(&plain(&["Filter Cutoff"])), ["Filter"]);
}

#[test]
fn the_plumbing_leaves_no_page_and_the_params_list_keeps_it() {
    assert!(is_plumbing("Select Preset") && is_plumbing("MPE_PitchBend_3"));
    assert!(!is_plumbing("Filter Cutoff"));
    let groups = group_params(&plain(&["Select Preset", "VST_ProgramChange_7", "Filter Cutoff"]));
    assert_eq!(groups.iter().map(|g| g.page.as_str()).collect::<Vec<_>>(), ["Filter"]);
    assert_eq!(groups[0].params.len(), 1);
}

#[test]
fn a_plugin_that_publishes_nothing_else_gets_an_empty_panel_the_schema_accepts() {
    let ui = auto_organize(&plain(&["Select Preset", "MPE_Aftertouch_2"]));
    assert_eq!(ui["widgets"], json!({}));
    assert!(accepted(&ui));
}

#[test]
fn the_lanes_own_names_are_dropped_except_base_and_anchored_at_the_start() {
    for n in ["@pan", "@monitor", "@mute"] {
        assert!(is_plumbing(n), "{n}");
    }
    assert_eq!(family_of("@base"), GENERAL_PAGE);
    assert!(!is_plumbing("@base"));
    assert_ne!(family_of("@base_mix"), GENERAL_PAGE);
    let groups = plain(&["Master Volume", "@pan", "Hammer Hardness"]);
    assert_eq!(page_names(&groups), ["Mix", "Other"]);
}

#[test]
fn the_modules_own_blend_is_told_from_an_effects_and_decides_a_position_not_a_page() {
    for n in ["Mix", "Dry/Wet"] {
        assert!(is_blend(n), "{n}");
    }
    for n in ["Reverb Dry/Wet", "Delay Mix", "Master Volume"] {
        assert!(!is_blend(n), "{n}");
    }
    let groups = group_params(&plain(&["Mix", "Reverb Dry/Wet", "@pan"]));
    assert_eq!(
        groups.iter().map(|g| g.page.as_str()).collect::<Vec<_>>(),
        ["Reverb", "Mix"]
    );
    assert_eq!(
        groups[1].params.iter().map(|x| x.name.as_str()).collect::<Vec<_>>(),
        ["Mix"]
    );
}

fn proposed() -> (Vec<AutoParam>, Value) {
    let params = vec![
        ranged("Filter Cutoff", 20.0, 20000.0),
        ranged("Filter Resonance", 0.0, 1.0),
        ranged("Master Volume", -60.0, 6.0),
        states("Lid Position", &[("Closed", 0), ("Open", 1)]),
    ];
    let ui = auto_organize(&params);
    (params, ui)
}

#[test]
fn every_parameter_is_on_one_tab_of_the_panel_and_every_widget_has_its_own_id() {
    let (params, ui) = proposed();
    assert_eq!(ui["widgets"], json!({}));
    assert!(ui["tabs"].as_array().unwrap().len() > 1);
    let mut got = binds(&ui);
    got.sort();
    let mut want: Vec<String> = params.iter().map(|x| x.name.clone()).collect();
    want.sort();
    assert_eq!(got, want);
    let ids: Vec<String> = pages_of(&ui)
        .into_iter()
        .flat_map(|(_, w)| w.into_iter().map(|(k, _)| k))
        .collect();
    let mut unique = ids.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), ids.len());
    assert!(accepted(&ui));
}

#[test]
fn the_tabs_are_a_column_beside_the_grid_so_one_starts_on_row_zero() {
    let (_, ui) = proposed();
    let rows = pages_of(&ui)
        .into_iter()
        .flat_map(|(_, w)| w.into_iter().map(|(_, v)| v["row"].as_u64().unwrap()));
    assert_eq!(rows.min(), Some(0));
}

#[test]
fn a_knob_carries_its_own_range_and_a_named_setting_is_a_dropdown() {
    let (_, ui) = proposed();
    let cutoff = bound(&page(&ui, "Filter"), "Filter Cutoff").clone();
    assert_eq!(
        (&cutoff["type"], &cutoff["min"], &cutoff["max"]),
        (&json!("knob"), &json!(20.0), &json!(20000.0))
    );
    let other = page(&ui, "Other");
    let lid = bound(&other, "Lid Position");
    assert_eq!(lid["type"], "dropdown");
    assert_eq!(lid["options"].as_array().map(Vec::len), Some(2));
}

#[test]
fn the_panel_opens_on_general_whatever_order_the_parameters_arrived_in() {
    let ui = auto_organize(&[p("Reverb Room Size"), p("Filter Cutoff"), p("@base")]);
    assert_eq!(pages_of(&ui)[0].0, GENERAL_PAGE);
}

#[test]
fn a_module_with_no_base_has_no_general_page_and_one_family_is_the_panel_itself() {
    let ui = auto_organize(&plain(&["Reverb Room Size", "@solo", "@pan"]));
    assert!(ui.get("tabs").is_none());
    assert_eq!(binds(&ui), ["Reverb Room Size"]);
}

#[test]
fn base_has_the_front_page_to_itself_and_the_other_intrinsics_are_dropped() {
    let ui = auto_organize(&[
        states("@base", &[("none", 0), ("warm", 1)]),
        p("@mute"),
        ranged("@pan", 0.0, 1.0),
    ]);
    let general = page(&ui, GENERAL_PAGE);
    assert_eq!(general.len(), 1);
    let w = &general[0];
    assert_eq!(
        (&w["type"], &w["bind"], &w["col"], &w["row"]),
        (&json!("dropdown"), &json!("@base"), &json!(0), &json!(0))
    );
}

#[test]
fn an_empty_list_still_proposes_a_grid_the_schema_accepts() {
    let none = auto_organize(&[]);
    assert_eq!(none["rows"], 1);
    assert_eq!(none["widgets"], json!({}));
    assert!(accepted(&none));
}

#[test]
fn the_engines_blend_replaces_the_plugins_and_stays_out_of_the_panel() {
    let ui = auto_organize(&plain(&["Room Size", "Wet Level", "Dry Level", "@mix"])).to_string();
    for gone in ["\"bind\":\"@mix\"", "Wet Level", "Dry Level"] {
        assert!(!ui.contains(gone), "{gone}");
    }
    let kept = auto_organize(&plain(&["Room Size", "Mix"])).to_string();
    assert!(kept.contains("\"bind\":\"Mix\""));
}

#[test]
fn the_engines_levels_are_the_footers_and_replace_the_plugins_input_and_output() {
    let ui = auto_organize(&plain(&["Drive", "Input", "Output", "@input-volume", "@output-volume"])).to_string();
    for gone in [
        "@input-volume",
        "@output-volume",
        "\"bind\":\"Input\"",
        "\"bind\":\"Output\"",
    ] {
        assert!(!ui.contains(gone), "{gone}");
    }
}

#[test]
fn knobs_lay_out_two_by_two_and_a_dropdown_is_the_slots_width_and_one_row() {
    let mut params = plain(&["Filter Cutoff", "Filter Resonance"]);
    params.push(states("Lid Position", &[("Open", 1)]));
    let ui = auto_organize(&params);
    let res = bound(&page(&ui, "Filter"), "Filter Resonance").clone();
    assert_eq!((&res["colSpan"], &res["rowSpan"]), (&json!(2), &json!(2)));
    let lid = bound(&page(&ui, "Other"), "Lid Position").clone();
    assert_eq!(lid["colSpan"], 2);
    assert!(lid.get("rowSpan").is_none());
}

#[test]
fn a_page_is_never_taller_than_four_rows_and_a_long_one_wraps_onto_a_second() {
    let ui = auto_organize(&knobs(30, "Filter"));
    assert!(ui["rows"].as_u64().unwrap() <= LAYOUT_MAX as u64);
    let tabs: Vec<&str> = ui["tabs"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert_eq!(tabs, ["Filter", "Filter 2", "Filter 3"]);
}

#[test]
fn a_vertical_twin_four_columns_wide_comes_only_when_the_panel_is_wider() {
    let wide = auto_organize(&knobs(9, "Filter"));
    assert!(wide["vertical"]["cols"].as_u64().unwrap() <= LAYOUT_MAX as u64);
    assert!(twin_issues(&wide).is_empty());
    let narrow = auto_organize(&knobs(2, "Filter"));
    assert!(narrow.get("vertical").is_none());
    assert!(twin_issues(&narrow).is_empty());
}

#[test]
fn base_comes_first_and_the_blend_last_in_both_shapes() {
    let mut params = plain(&["Dry", "Filter Cutoff", "Wet"]);
    params.push(states("@base", &[("none", 0)]));
    params.extend(knobs(6, "Filter"));
    let ui = auto_organize(&params);
    let mix: Vec<String> = page(&ui, "Mix")
        .iter()
        .filter_map(|w| w["bind"].as_str().map(str::to_owned))
        .collect();
    assert_eq!(mix, ["Dry", "Wet"]);
    assert!(twin_issues(&ui).is_empty());
}
