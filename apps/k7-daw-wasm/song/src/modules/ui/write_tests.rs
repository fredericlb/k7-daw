// write_tests.rs — a panel read from and written to a module's text, case by case; the cases are the old TypeScript tests' inputs (moduleUi.test.ts).

use super::*;
use serde_json::json;

// Two promises: every line outside `ui(…)` comes back byte-identical, and what the grid writes loads.

const MODULE: &str = "// the lead voice
extends(\"instruments/fourosc\")
params({
  \"Filter Freq\": 90,
  Level: -12.0,
})
ui({
  cols: 4,
  rows: 2,
  widgets: {
    // the one knob
    w1: { type: \"knob\", col: 0, row: 0, bind: \"Filter Freq\", min: 0, max: 1 },
  },
})
";

const BARE: &str = "plugin({ type: \"internal\", name: \"4osc\" })\n";

fn ui_of(text: &str) -> Value {
    read_ui(text, false).ui.unwrap()
}

fn write(text: &str, ui: &Value) -> String {
    write_ui(text, ui, false).unwrap()
}

#[test]
fn the_grid_and_its_widgets_are_read() {
    let ui = ui_of(MODULE);
    assert_eq!((&ui["cols"], &ui["rows"]), (&json!(4), &json!(2)));
    let w = &ui["widgets"]["w1"];
    for (key, want) in [
        ("type", json!("knob")),
        ("col", json!(0)),
        ("row", json!(0)),
        ("bind", json!("Filter Freq")),
        ("min", json!(0)),
        ("max", json!(1)),
    ] {
        assert_eq!(w[key], want, "{key}");
    }
}

#[test]
fn a_file_with_no_ui_has_none_and_is_not_malformed() {
    let read = read_ui(BARE, false);
    assert!(read.ui.is_none());
    // The grid may create one.
    assert!(!read.malformed);
}

#[test]
fn a_ui_the_schema_refuses_is_malformed_so_the_pane_stays_read_only() {
    let bad = "ui({ cols: 4, rows: 2, widgets: { w1: { type: \"nope\" } } })\n";
    let read = read_ui(bad, false);
    assert!(read.ui.is_none());
    assert!(read.malformed);
}

#[test]
fn params_read_as_text_as_written_even_one_the_schema_would_refuse() {
    assert_eq!(
        Value::Object(read_params(MODULE)),
        json!({ "Filter Freq": "90", "Level": "-12.0" })
    );
    assert_eq!(
        Value::Object(read_params("params({ Drive: \"loud\" })\n")),
        json!({ "Drive": "loud" })
    );
}

#[test]
fn moving_a_knob_is_a_one_line_diff_and_every_other_line_is_byte_identical() {
    let mut ui = ui_of(MODULE);
    ui["widgets"]["w1"]["col"] = json!(2);
    let after = write(MODULE, &ui);
    let before: Vec<&str> = MODULE.split('\n').collect();
    let after: Vec<&str> = after.split('\n').collect();
    assert_eq!(after.len(), before.len());
    let changed: Vec<&&str> = after
        .iter()
        .enumerate()
        .filter(|(i, l)| **l != before[*i])
        .map(|(_, l)| l)
        .collect();
    assert_eq!(
        changed,
        [&"    w1: { type: \"knob\", col: 2, row: 0, bind: \"Filter Freq\", min: 0, max: 1 },"]
    );
}

#[test]
fn a_comment_inside_the_block_is_kept() {
    let mut ui = ui_of(MODULE);
    ui["widgets"]["w2"] = json!({ "type": "toggle", "col": 2, "row": 0, "bind": "Level" });
    let after = write(MODULE, &ui);
    assert!(after.contains("    // the one knob\n"));
    assert!(after.contains("    w2: { type: \"toggle\", col: 2, row: 0, bind: \"Level\" },\n"));
}

#[test]
fn the_block_is_created_when_the_file_has_none() {
    let ui =
        json!({ "cols": 2, "rows": 1, "widgets": { "a": { "type": "toggle", "col": 0, "row": 0, "bind": "Mute" } } });
    let after = write(BARE, &ui);
    assert!(after.starts_with(BARE));
    assert!(after.contains("a: { type: \"toggle\", col: 0, row: 0, bind: \"Mute\" }"));
    let back = ui_of(&after);
    assert_eq!(back["widgets"]["a"]["type"], "toggle");
    assert_eq!(back["widgets"]["a"]["bind"], "Mute");
}

#[test]
fn the_defaults_are_dropped_so_the_block_stays_as_terse_as_a_hand_written_one() {
    let ui = json!({ "cols": 2, "rows": 1, "widgets": {
        "a": { "type": "knob", "col": 0, "row": 0, "colSpan": 1, "rowSpan": 1, "label": "" } } });
    let after = write("", &ui);
    for dropped in ["colSpan", "rowSpan", "label"] {
        assert!(!after.contains(dropped), "{dropped}");
    }
}

#[test]
fn a_range_does_not_travel_when_the_widget_is_retyped_to_a_kind_that_cannot_carry_one() {
    // Retype a knob to a toggle and the range must not travel, or the file stops loading.
    let ui = json!({ "cols": 2, "rows": 1, "widgets": {
        "a": { "type": "toggle", "col": 0, "row": 0, "bind": "Mute", "min": 0, "max": 1 } } });
    let after = write("", &ui);
    assert!(!after.contains("min") && !after.contains("max"));
    assert!(read_ui(&after, false).ui.is_some());
}

#[test]
fn read_write_read_gives_the_same_model_and_an_unchanged_model_changes_nothing() {
    let once = ui_of(MODULE);
    assert_eq!(ui_of(&write(MODULE, &once)), once);
    assert_eq!(write(MODULE, &once), MODULE);
}

#[test]
fn the_tabs_are_written_in_order_each_widget_still_on_one_line() {
    let ui = json!({ "cols": 4, "rows": 2, "widgets": {}, "tabs": [
        { "name": "Osc", "widgets": { "k": { "type": "knob", "col": 0, "row": 0, "bind": "Mix" } } },
        { "name": "Filter", "widgets": {} },
    ] });
    let after = write("", &ui);
    let (osc, filter) = (after.find("name(\"Osc\")"), after.find("name(\"Filter\")"));
    assert!(osc.is_some() && filter.is_some() && osc < filter);
    assert!(after.contains("k: { type: \"knob\", col: 0, row: 0, bind: \"Mix\" }"));
    let names: Vec<Value> = ui_of(&after)["tabs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].clone())
        .collect();
    assert_eq!(names, [json!("Osc"), json!("Filter")]);
}

#[test]
fn a_panel_without_tabs_writes_none_and_a_buffer_that_does_not_parse_comes_back_unchanged() {
    assert!(!write("", &json!({ "cols": 2, "rows": 1, "widgets": {} })).contains("tabs"));
    assert_eq!(write("ui({", &ui_of(MODULE)), "ui({");
}
