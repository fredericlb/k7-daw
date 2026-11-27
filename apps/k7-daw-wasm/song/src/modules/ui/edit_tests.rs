// edit_tests.rs — the grid editor's tab, clipboard and retype rules; the cases are the old TypeScript tests' inputs.

use super::*;
use serde_json::json;

fn knob(col: u32, row: u32, bind: &str) -> Value {
    json!({ "type": "knob", "col": col, "row": row, "colSpan": 2, "rowSpan": 2, "bind": bind })
}

fn plain() -> Value {
    json!({ "cols": 4, "rows": 2, "widgets": { "a": knob(0, 0, "Mix") } })
}

fn tabbed() -> Value {
    json!({ "cols": 4, "rows": 2, "widgets": { "s": knob(3, 0, "Mix") }, "tabs": [
        { "name": "Osc", "widgets": { "o": knob(0, 0, "Mix") } },
        { "name": "Filter", "widgets": { "f": knob(1, 0, "Mix") } },
    ] })
}

fn names(ui: &Value) -> Vec<&str> {
    tabs(ui).iter().filter_map(|t| t["name"].as_str()).collect()
}

#[test]
fn the_grid_writes_the_first_tab_of_a_tabbed_panel_and_the_panel_otherwise() {
    assert_eq!(page_in(&tabbed(), None), Some(0));
    assert_eq!(page_in(&plain(), None), None);
    assert_eq!(page_in(&tabbed(), Some(None)), None);
    assert_eq!(page_in(&tabbed(), Some(Some(7))), Some(0));
    assert_eq!(panel_ids(&tabbed()), vec!["s", "o", "f"]);
    let next = write_widgets_at(&tabbed(), Some(1), json!({ "g": knob(2, 0, "X") }));
    assert_eq!(next["tabs"][1]["widgets"].as_object().map(|m| m.len()), Some(1));
    assert_eq!(next["tabs"][0], tabbed()["tabs"][0]);
}

#[test]
fn the_second_tab_makes_the_list_and_removing_back_to_one_folds_it_away() {
    let (ui, at) = add_tab(&plain());
    assert_eq!(names(&ui), ["Main", "Tab 2"]);
    assert_eq!(ui["widgets"], json!({}));
    assert_eq!(ui["tabs"][0]["widgets"]["a"]["bind"], "Mix");
    assert_eq!(at, 1);
    let (ui, at) = add_tab(&tabbed());
    assert_eq!(names(&ui), ["Osc", "Filter", "Tab 3"]);
    assert_eq!(at, 2);
    let folded = remove_tab(&tabbed(), 0);
    assert!(folded.get("tabs").is_none());
    let ids: Vec<&String> = folded["widgets"].as_object().unwrap().keys().collect();
    assert_eq!(ids, ["s", "f"]);
}

#[test]
fn a_name_is_ten_characters_and_never_another_tabs() {
    assert_eq!(
        rename_tab(&tabbed(), 0, "  Oscillators  ")["tabs"][0]["name"],
        "Oscillator"
    );
    assert_eq!(rename_tab(&tabbed(), 0, "   "), tabbed());
    assert_eq!(rename_tab(&tabbed(), 0, "Filter"), tabbed());
    let (ui, at) = move_tab(&tabbed(), 0, 1);
    assert_eq!((names(&ui), at), (vec!["Filter", "Osc"], 1));
    assert_eq!(move_tab(&tabbed(), 0, -1).1, 0);
}

fn panel() -> Obj {
    let group = json!({ "type": "group", "col": 2, "row": 1, "colSpan": 4, "rowSpan": 2, "label": "AMP" });
    json!({ "g": group, "dry": knob(2, 1, "Dry"), "wet": knob(4, 1, "Wet"), "out": knob(8, 1, "Out") })
        .as_object()
        .cloned()
        .unwrap_or_default()
}

#[test]
fn a_group_is_cut_with_what_it_frames_and_pasted_where_it_is_dropped() {
    let widgets = panel();
    assert_eq!(clip_ids(&widgets, "g"), ["g", "dry", "wet"]);
    assert_eq!(clip_ids(&widgets, "out"), ["out"]);
    let clip = copy_widgets(&widgets, "g").unwrap();
    assert_eq!((clip.cols, clip.rows), (4.0, 2.0));
    assert_eq!(clip.widgets["wet"]["col"], 2);
    let (next, first) = paste_widgets(&widgets, &clip, (0.0, 4.0), (14.0, 8.0), &[]);
    assert_eq!(first.as_deref(), Some("g_2"));
    assert_eq!(next["wet_2"]["col"], 2);
    assert_eq!(next["wet_2"]["row"], 4);
    assert_eq!(next["g"], widgets["g"]);
    // Pulled back inside the grid, and clear of an id another page uses.
    let out = copy_widgets(&widgets, "out").unwrap();
    let (next, first) = paste_widgets(&Obj::new(), &out, (13.0, 7.0), (14.0, 8.0), &["out".to_owned()]);
    assert_eq!(first.as_deref(), Some("out_2"));
    assert_eq!(
        (next["out_2"]["col"].clone(), next["out_2"]["row"].clone()),
        (json!(12), json!(6))
    );
}

#[test]
fn a_retype_clears_what_the_new_type_lacks() {
    let w = json!({ "type": "knob", "col": 0, "row": 0, "bind": "Cutoff", "min": 0, "max": 1, "pair": "Dry" });
    let (set, clear) = retype(&w, "toggle");
    assert_eq!(set.get("bind"), Some(&json!("Cutoff")));
    assert_eq!(
        clear,
        [
            "text", "min", "max", "step", "curve", "source", "options", "library", "color", "pair"
        ]
    );
    let (set, _) = retype(&w, "group");
    assert_eq!(set.get("color"), Some(&json!(GROUP_COLOR)));
    assert!(set.get("bind").is_none());
    assert!(bindable("knob") && !bindable("label") && ranged("fader") && !ranged("dropdown"));
}
