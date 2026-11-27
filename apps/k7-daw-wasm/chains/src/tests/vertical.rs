// The vertical layout and a previewed module: a panel's narrow twin drawn in its place, a bases dropdown, a card sized from its grid.

use super::*;
use crate::geom::{TAB_COL_W, TAB_H, TAB_MENU_H, column_w, menu_h, node_size, rows_height};
use crate::graph_layout::layout;
use crate::module_row::with_open_tabs;
use crate::panel::Panel;
use serde_json::json;

fn knob(col: u32, row: u32, bind: &str) -> Value {
    json!({"type": "knob", "col": col, "row": row, "colSpan": 2, "rowSpan": 2, "bind": bind})
}

fn tabs() -> Value {
    json!([
        {"name": "OSC", "widgets": {"a": knob(0, 0, "A"), "b": knob(2, 0, "B")}},
        {"name": "FLT", "widgets": {"c": knob(0, 0, "C")}}
    ])
}

fn ui() -> Value {
    json!({"cols": 8, "rows": 2, "widgets": {}, "tabs": tabs(), "vertical": {"cols": 4, "rows": 2, "widgets": {}, "tabs": tabs()}})
}

fn graph_of(ui: Value) -> Value {
    doc(
        json!({"c": {"use": [{"module": "synth", "ui": ui}], "out": [{"to": "MIX", "gain": 1}]}}),
        json!([]),
    )
}

fn synth(g: &Value, layout: Option<&str>) -> Value {
    let o = layout.map_or(json!({}), |l| json!({"layout": l}));
    module(&built(g, o).nodes, "synth").clone()
}

fn panel(v: &Value) -> Option<Panel> {
    serde_json::from_value(v.clone()).ok()
}

#[test]
fn draws_the_panels_narrow_twin_its_tabs_a_menu_no_column_one_line_on_top() {
    let g = graph_of(ui());
    let n = synth(&g, Some("vertical"));
    assert_eq!(
        (n["ui"]["cols"].clone(), n["ui"]["tabsAs"].clone()),
        (json!(4.0), json!("dropdown"))
    );
    assert_eq!(column_w(panel(&n["ui"]).as_ref()), 0.0);
    assert_eq!(menu_h(panel(&n["ui"]).as_ref()), TAB_MENU_H);
    assert!(num(&n["w"]) < num(&synth(&g, None)["w"]));
}

#[test]
fn leaves_the_horizontal_panel_as_it_is_its_own_grid_its_tabs_a_column() {
    let n = synth(&graph_of(ui()), None);
    assert_eq!(n["ui"]["cols"], json!(8.0));
    assert_eq!(column_w(panel(&n["ui"]).as_ref()), TAB_COL_W);
    assert_eq!(menu_h(panel(&n["ui"]).as_ref()), 0.0);
}

#[test]
fn draws_a_panel_with_no_twin_as_itself_tabs_as_a_menu_for_the_view_to_fold() {
    let mut bare = ui();
    if let Some(o) = bare.as_object_mut() {
        o.remove("vertical");
    }
    let n = synth(&graph_of(bare.clone()), Some("vertical"));
    assert_eq!(
        (n["ui"]["cols"].clone(), n["ui"]["tabsAs"].clone()),
        (json!(8.0), json!("dropdown"))
    );
    let (shown, plain) = (panel(&n["ui"]), panel(&bare));
    assert!(node_size(shown.as_ref(), false).1 > node_size(plain.as_ref(), false).1 - TAB_MENU_H);
}

#[test]
fn keeps_a_horizontal_cards_tabs_a_column_however_many_and_the_card_no_taller_than_4_rows() {
    let many_tabs: Vec<Value> = (0..25)
        .map(|i| json!({"name": format!("T{i}"), "widgets": {}}))
        .collect();
    let many = json!({"cols": 8, "rows": 4, "widgets": {}, "tabs": many_tabs});
    let p = panel(&many);
    assert_eq!((column_w(p.as_ref()), menu_h(p.as_ref())), (TAB_COL_W, 0.0));
    let flat = |v: &Value| {
        let mut v = v.clone();
        v["tabs"] = Value::Null;
        panel(&v)
    };
    assert_eq!(node_size(p.as_ref(), false).1, node_size(flat(&many).as_ref(), false).1);
    // A short panel still grows to list a few tabs, up to 4 rows' height.
    let mut short = many.clone();
    short["rows"] = json!(1);
    short["tabs"] = json!(many["tabs"].as_array().map(|t| t[..5].to_vec()));
    let grown = node_size(panel(&short).as_ref(), false).1 - node_size(flat(&short).as_ref(), false).1;
    assert_eq!(grown, 5.0 * TAB_H - rows_height(1.0));
}

#[test]
fn draws_a_narrow_card_as_tall_as_its_open_page_its_top_edge_where_it_was() {
    let mut tall = ui();
    tall["vertical"] = json!({"cols": 4, "rows": 6, "widgets": {}, "tabs": [
        {"name": "OSC", "widgets": {"a": knob(0, 0, "A"), "b": knob(0, 2, "B"), "z": knob(0, 4, "Z")}},
        {"name": "FLT", "widgets": {"c": knob(0, 0, "C")}}
    ]});
    let b = built(&graph_of(tall), json!({"layout": "vertical"}));
    let (nodes, edges) = b.parts();
    let mut laid = layout(nodes, edges, None, None).nodes;
    let n = laid
        .iter()
        .find(|n| n.module.as_deref() == Some("synth"))
        .expect("synth")
        .clone();
    let tab_h = n.tab_h.clone().unwrap_or_default();
    assert_eq!(tab_h[0], n.h);
    assert_eq!(tab_h[1], n.h - rows_height(6.0) + rows_height(2.0));
    with_open_tabs(&mut laid, &std::collections::HashMap::from([(n.id.clone(), 1.0)]));
    let shown = laid.iter().find(|x| x.id == n.id).expect("synth");
    assert_eq!((shown.h, shown.y), (tab_h[1], n.y));
}

fn preview(bases: Option<Vec<&str>>, ui: Value) -> Value {
    let mut step = json!({"module": "fuzz", "category": "audio", "emits": "audio", "enabled": true, "pluginId": "preview:fuzz", "ui": ui, "params": {}});
    if let Some(b) = bases.filter(|b| !b.is_empty()) {
        step["bases"] = json!(b);
    }
    doc_with(
        json!({"preview": {"from": "in", "use": [step], "out": [{"to": "OUT", "gain": 1}]}}),
        json!(["in"]),
        json!({"trackSignals": {"in": "audio"}}),
    )
}

fn base_widget() -> Value {
    json!({"cols": 2, "rows": 1, "widgets": {"b": {"type": "dropdown", "col": 0, "row": 0, "bind": "@base"}}})
}

fn rows_of(bases: Option<Vec<&str>>) -> Vec<(String, Value)> {
    let b = built(&preview(bases, base_widget()), json!({}));
    let card = b
        .of_kind("module")
        .first()
        .map(|n| (*n).clone())
        .expect("a module card");
    list(&card["ui"]["widgets"]["b"]["options"])
        .iter()
        .map(|o| (text(&o["label"]), o["value"].clone()))
        .collect()
}

#[test]
fn the_base_dropdown_lists_the_modules_bases_with_none_first_so_a_choice_can_be_undone() {
    let rows = rows_of(Some(vec!["warm", "bright"]));
    assert_eq!(
        rows.iter().map(|r| r.0.as_str()).collect::<Vec<_>>(),
        ["none", "warm", "bright"]
    );
}

#[test]
fn the_base_dropdown_keeps_each_row_its_place_in_the_folder_as_its_value() {
    let rows = rows_of(Some(vec!["FM Bass", "WT Bass", "fingered"]));
    let seen: Vec<(&str, f64)> = rows.iter().map(|(l, v)| (l.as_str(), num(v))).collect();
    assert_eq!(
        seen,
        [("none", 0.0), ("FM Bass", 1.0), ("WT Bass", 2.0), ("fingered", 3.0)]
    );
}

#[test]
fn the_base_dropdown_still_draws_none_alone_when_nothing_was_passed() {
    assert_eq!(rows_of(None).iter().map(|r| r.0.as_str()).collect::<Vec<_>>(), ["none"]);
    assert_eq!(rows_of(Some(vec![])).len(), 1);
}

fn drive_ui(cols: u32, rows: u32) -> Value {
    json!({"cols": cols, "rows": rows, "widgets": {"drive": {"type": "knob", "col": 0, "row": 0, "bind": "drive"}}})
}

#[test]
fn a_previewed_module_lays_out_with_real_geometry_and_draws_the_source_the_module_and_the_sink() {
    let b = built(&preview(None, drive_ui(2, 1)), json!({}));
    let kinds: HashSet<String> = b.nodes.iter().map(|n| text(&n["kind"])).collect();
    assert!(["track", "module", "sink"].iter().all(|k| kinds.contains(*k)));
    let laid = b.laid(Value::Null, Value::Null);
    assert!(!list(&laid["nodes"]).is_empty());
    assert!(
        list(&laid["nodes"])
            .iter()
            .all(|n| num(&n["x"]).is_finite() && num(&n["y"]).is_finite())
    );
}

#[test]
fn a_previewed_card_is_sized_from_the_grid_rather_than_the_default() {
    let w = |cols, rows| num(&built(&preview(None, drive_ui(cols, rows)), json!({})).of_kind("module")[0]["w"]);
    assert!(w(8, 4) > w(1, 1));
}

#[test]
fn a_previewed_midi_module_is_fed_by_a_midi_cable() {
    let mut g = preview(None, drive_ui(2, 1));
    g["chains"]["preview"]["use"][0]["module"] = json!("jun");
    g["chains"]["preview"]["from"] = json!("midi-in");
    g["tracks"] = json!(["midi-in"]);
    g["trackSignals"] = json!({"midi-in": "midi"});
    let b = built(&g, json!({}));
    let signals: Vec<(Value, Value)> = b
        .edges
        .iter()
        .map(|e| (e["source"].clone(), e["signal"].clone()))
        .collect();
    assert_eq!(
        signals,
        [
            (json!("trk:midi-in"), json!("midi")),
            (json!("mod:jun"), json!("audio"))
        ]
    );
}
