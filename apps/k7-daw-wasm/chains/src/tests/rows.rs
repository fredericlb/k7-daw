// A module row: named modules side by side, with their cables, scaled, with their curves, led by their lane.

use super::*;
use crate::module_row::{RowOptions, module_row};
use crate::node::Laid;
use crate::subgraph::{GraphView, track_inputs};
use serde_json::json;

/// The row as the page receives it, from the named modules and the options in the page's own spelling.
pub fn row(g: &Value, modules: &[&str], o: Value) -> Value {
    serde_json::to_value(row_laid(g, modules, o)).expect("a row")
}

pub fn row_laid(g: &Value, modules: &[&str], o: Value) -> Laid {
    let o: RowOptions = serde_json::from_value(o).expect("row options");
    let modules: Vec<String> = modules.iter().map(|m| (*m).to_owned()).collect();
    module_row(&graph(g), &modules, &o)
}

fn ids(r: &Value) -> Vec<String> {
    list(&r["nodes"]).iter().map(|n| text(&n["id"])).collect()
}

fn modules_of(r: &Value) -> Vec<String> {
    list(&r["nodes"]).iter().map(|n| text(&n["module"])).collect()
}

fn three() -> Value {
    doc(
        json!({
            "a": {"from": "vox", "use": [{"module": "mx-vox"}], "out": [{"to": "OUT", "gain": 1}]},
            "b": {"from": "bass", "use": [{"module": "mx-bass"}], "out": [{"to": "OUT", "gain": 1}]},
            "c": {"from": "gtr", "use": [{"module": "mx-gtr"}], "out": [{"to": "OUT", "gain": 1}]}
        }),
        json!(["vox", "bass", "gtr"]),
    )
}

#[test]
fn places_them_left_to_right_in_the_order_asked_for_not_the_graphs() {
    let r = row(&three(), &["mx-gtr", "mx-vox", "mx-bass"], json!({}));
    assert_eq!(modules_of(&r), ["mx-gtr", "mx-vox", "mx-bass"]);
    let xs: Vec<f64> = list(&r["nodes"]).iter().map(|n| num(&n["x"])).collect();
    assert!(xs.windows(2).all(|w| w[0] < w[1]));
    let ys: Vec<String> = list(&r["nodes"]).iter().map(|n| n["y"].to_string()).collect();
    assert_eq!(ys.iter().collect::<HashSet<_>>().len(), 1);
}

#[test]
fn leaves_a_gap_between_cards_rather_than_butting_them_together() {
    let r = list(&row(&three(), &["mx-vox", "mx-bass"], json!({}))["nodes"]);
    assert!(num(&r[1]["x"]) > num(&r[0]["x"]) + num(&r[0]["w"]));
}

#[test]
fn keeps_the_node_the_full_graph_would_draw_the_card_not_a_copy_of_it() {
    let full = built(&three(), json!({})).laid(json!(null), json!(null));
    let from_full = module(&list(&full["nodes"]), "mx-vox").clone();
    let from_row = list(&row(&three(), &["mx-vox"], json!({}))["nodes"])[0].clone();
    for k in ["id", "w", "h", "chain"] {
        assert_eq!(from_row[k], from_full[k], "{k}");
    }
}

#[test]
fn strips_the_sockets_which_on_an_edgeless_row_would_plug_into_nothing() {
    let b = built(&three(), json!({}));
    let jacks = &module(&b.nodes, "mx-vox")["jacks"];
    assert!(list(&jacks["in"]).len() + list(&jacks["out"]).len() > 0);
    for n in list(&row(&three(), &["mx-vox", "mx-bass"], json!({}))["nodes"]) {
        assert!(n["jacks"].is_null());
    }
}

#[test]
fn skips_a_name_the_graph_does_not_have_rather_than_leaving_a_hole() {
    let r = row(&three(), &["mx-vox", "gone", "mx-bass"], json!({}));
    assert_eq!(modules_of(&r), ["mx-vox", "mx-bass"]);
}

#[test]
fn has_no_extent_when_nothing_is_named() {
    let r = row(&three(), &[], json!({}));
    assert_eq!(
        (list(&r["nodes"]).len(), num(&r["width"]), num(&r["height"])),
        (0, 0.0, 0.0)
    );
    assert_eq!(r["edges"], json!([]));
}

fn lane() -> Value {
    doc(
        json!({"vox": {"from": "vox", "use": [{"module": "comp"}, {"module": "eq"}], "out": [{"to": "OUT", "gain": 1}]}}),
        json!(["vox"]),
    )
}

#[test]
fn with_links_carries_the_graphs_own_cables_between_the_cards_it_kept() {
    let r = row(&lane(), &["comp", "eq"], json!({"links": true}));
    let edges = list(&r["edges"]);
    assert_eq!(edges.len(), 1);
    assert_eq!(
        (&edges[0]["source"], &edges[0]["target"]),
        (&json!("mod:comp"), &json!("mod:eq"))
    );
}

#[test]
fn with_links_off_keeps_none_of_them() {
    assert_eq!(row(&lane(), &["comp", "eq"], json!({}))["edges"], json!([]));
}

#[test]
fn with_links_invents_nothing_between_neighbours_the_graph_does_not_join() {
    let apart = doc(
        json!({
            "a": {"from": "vox", "use": [{"module": "one"}], "out": [{"to": "OUT", "gain": 1}]},
            "b": {"from": "vox", "use": [{"module": "two"}], "out": [{"to": "OUT", "gain": 1}]}
        }),
        json!(["vox"]),
    );
    assert_eq!(row(&apart, &["one", "two"], json!({"links": true}))["edges"], json!([]));
}

#[test]
fn with_links_marks_the_cards_so_their_cables_meet_on_the_sides() {
    let r = row(&lane(), &["comp", "eq"], json!({"links": true}));
    assert!(list(&r["nodes"]).iter().all(|n| n["flow"] == "across"));
    assert!(
        list(&row(&lane(), &["comp", "eq"], json!({}))["nodes"])
            .iter()
            .all(|n| n["flow"].is_null())
    );
}

#[test]
fn with_links_gives_every_card_the_same_height_so_none_of_them_is_short() {
    let tall = doc(
        json!({"vox": {"from": "vox", "use": [{"module": "comp", "ui": {"cols": 2, "rows": 4, "widgets": {}}}, {"module": "eq"}], "out": [{"to": "OUT", "gain": 1}]}}),
        json!(["vox"]),
    );
    let hs: Vec<f64> = list(&row(&tall, &["comp", "eq"], json!({"links": true}))["nodes"])
        .iter()
        .map(|n| num(&n["h"]))
        .collect();
    assert_eq!(hs.iter().map(|h| h.to_string()).collect::<HashSet<_>>().len(), 1);
    assert_eq!(hs[0], hs.iter().cloned().fold(f64::MIN, f64::max));
}

fn two() -> Value {
    doc(
        json!({
            "a": {"from": "vox", "use": [{"module": "mx-vox"}], "out": [{"to": "OUT", "gain": 1}]},
            "b": {"from": "bass", "use": [{"module": "mx-bass"}], "out": [{"to": "OUT", "gain": 1}]}
        }),
        json!(["vox", "bass"]),
    )
}

#[test]
fn width_scale_widens_each_card_without_touching_its_height() {
    let first = |o: Value| list(&row(&two(), &["mx-vox"], o)["nodes"])[0].clone();
    let (plain, wide) = (first(json!({})), first(json!({"widthScale": 1.5})));
    assert_eq!(num(&wide["w"]), (num(&plain["w"]) * 1.5).round());
    assert_eq!(wide["h"], plain["h"]);
}

#[test]
fn width_scale_spaces_the_row_by_the_widened_width_so_cards_still_cannot_overlap() {
    let r = row(&two(), &["mx-vox", "mx-bass"], json!({"widthScale": 1.5}));
    let n = list(&r["nodes"]);
    assert!(num(&n[1]["x"]) >= num(&n[0]["x"]) + num(&n[0]["w"]));
    assert!(num(&r["width"]) > num(&n[1]["x"]) + num(&n[1]["w"]) - 1.0);
}

#[test]
fn width_scale_defaults_to_the_cards_own_width() {
    let w = |o: Value| list(&row(&two(), &["mx-vox"], o)["nodes"])[0]["w"].clone();
    assert_eq!(w(json!({})), w(json!({"widthScale": 1})));
}

fn curved() -> Value {
    doc_with(
        json!({"vox": {"from": "vox", "use": [
            {"module": "comp"}, {"module": "eq", "automations": [{"param": "Gain", "curve": "wob"}]}
        ], "out": [{"to": "OUT", "gain": 1}]}}),
        json!(["vox"]),
        json!({"curves": {"wob": {"type": "lfo", "shape": "sine"}}}),
    )
}

#[test]
fn with_curves_hidden_the_automation_lands_on_a_card_with_no_curve_card_drawn() {
    let r = row(&curved(), &["comp", "eq"], json!({"links": true, "gap": -1}));
    assert!(list(&r["nodes"]).iter().all(|n| n["kind"] != "curve"));
    let auto: Vec<Value> = list(&r["edges"])
        .into_iter()
        .filter(|e| e["automation"].is_object())
        .collect();
    assert_eq!(auto.len(), 1);
    assert_eq!(auto[0]["target"], "mod:eq");
}

#[test]
fn with_curves_hidden_no_event_plug_is_drawn_and_the_nodes_own_when_they_are_shown() {
    let mut g = curved();
    if let Some(steps) = g["chains"]["vox"]["use"].as_array_mut() {
        for s in steps {
            s["events"] = json!(["note-on"]);
        }
    }
    let events = |curves: bool| {
        let r = row(&g, &["comp", "eq"], json!({"links": true, "gap": -1, "curves": curves}));
        module(&list(&r["nodes"]), "eq")["events"].clone()
    };
    assert!(events(false).is_null());
    assert_eq!(events(true), json!(["note-on"]));
}

#[test]
fn with_curves_shown_the_curve_sits_in_a_column_just_before_the_card_it_drives_under_its_top() {
    let r = row(
        &curved(),
        &["comp", "eq"],
        json!({"links": true, "gap": -1, "curves": true}),
    );
    let nodes = list(&r["nodes"]);
    let comp = nodes
        .iter()
        .find(|n| n["kind"] == "module" && n["module"] == "comp")
        .expect("comp");
    let eq = nodes
        .iter()
        .find(|n| n["kind"] == "module" && n["module"] == "eq")
        .expect("eq");
    let crv = nodes.iter().find(|n| n["kind"] == "curve").expect("a curve");
    assert!(num(&crv["x"]) > num(&comp["x"]) + num(&comp["w"]));
    assert!(num(&eq["x"]) > num(&crv["x"]) + num(&crv["w"]));
    assert_eq!(num(&crv["y"]), num(&eq["y"]) + 24.0);
    let auto = list(&r["edges"])
        .into_iter()
        .find(|e| e["automation"].is_object())
        .expect("a cable");
    assert_eq!(auto["source"], crv["id"]);
}

#[test]
fn a_row_led_by_its_lane_puts_the_lane_first() {
    let r = row(&lane(), &["comp", "eq"], json!({"links": true, "track": "vox"}));
    assert_eq!(ids(&r), ["trk:vox", "mod:comp", "mod:eq"]);
}

fn fed() -> Value {
    let mut g = lane();
    g["chains"]["mic"] = json!({"use": [{"module": "in", "monitor": "on"}], "out": [{"to": "pre", "gain": 1}]});
    g["chains"]["pre"] = json!({"use": [{"module": "pre"}], "out": [{"to": "vox", "gain": 1}]});
    g
}

fn inputs_of(g: &Value, track: &str) -> Vec<String> {
    let view: GraphView = serde_json::from_value(g.clone()).expect("a view");
    track_inputs(&view, track)
}

#[test]
fn a_row_led_by_its_lane_puts_what_feeds_the_lane_before_it() {
    let g = fed();
    assert_eq!(inputs_of(&g, "vox"), ["in", "pre"]);
    let r = row(
        &g,
        &["comp", "eq"],
        json!({"links": true, "track": "vox", "inputs": ["in", "pre"]}),
    );
    assert_eq!(ids(&r), ["mod:in", "mod:pre", "trk:vox", "mod:comp", "mod:eq"]);
    let feed = list(&r["edges"])
        .into_iter()
        .any(|e| e["source"] == "mod:pre" && e["target"] == "trk:vox");
    assert!(feed);
    // A lane nothing feeds has nothing before it, and a lane's own `from` is never taken for an input.
    assert!(inputs_of(&lane(), "vox").is_empty());
}

#[test]
fn a_picture_lanes_fade_stretch_is_left_out_of_its_inputs_and_the_shape_that_draws_on_it_kept() {
    let pic = doc_with(
        json!({
            "iris": {"use": [{"module": "iris", "category": "shaders"}], "out": [{"to": "cam", "gain": 1}]},
            "box": {"use": [{"module": "box", "category": "shapes"}], "out": [{"to": "cam", "gain": 1}]}
        }),
        json!([]),
        json!({"videoTracks": ["cam"]}),
    );
    assert_eq!(inputs_of(&pic, "cam"), ["box"]);
}

#[test]
fn a_row_led_by_its_lane_carries_the_cable_out_of_the_lane() {
    let r = row(&lane(), &["comp", "eq"], json!({"links": true, "track": "vox"}));
    assert!(
        list(&r["edges"])
            .iter()
            .any(|e| e["source"] == "trk:vox" && e["target"] == "mod:comp")
    );
}

fn clipped(g: Value) -> Value {
    let mut g = g;
    g["trackClips"] = json!({"vox": [{"ref": "riff", "start": 0, "tc": "1:1:1", "len": 4, "kind": "audio"}]});
    g
}

#[test]
fn a_row_led_by_its_lane_drops_the_lanes_clip_list_which_would_set_every_cards_height() {
    let r = row(
        &clipped(lane()),
        &["comp", "eq"],
        json!({"links": true, "track": "vox"}),
    );
    let nodes = list(&r["nodes"]);
    assert!(nodes[0]["clips"].is_null());
    assert_eq!(
        nodes.iter().map(|n| n["h"].to_string()).collect::<HashSet<_>>().len(),
        1
    );
}

#[test]
fn as_drawn_keeps_every_cards_own_width_and_the_lanes_clips_every_card_the_tallests_height() {
    let tall = doc(
        json!({"vox": {"from": "vox", "use": [{"module": "comp", "ui": {"cols": 2, "rows": 4, "widgets": {}}}, {"module": "eq"}], "out": [{"to": "OUT", "gain": 1}]}}),
        json!(["vox"]),
    );
    let g = clipped(tall);
    let full = built(&g, json!({}));
    let r = row(
        &g,
        &["comp", "eq"],
        json!({"links": true, "track": "vox", "asDrawn": true}),
    );
    let nodes = list(&r["nodes"]);
    for n in &nodes {
        assert_eq!(n["w"], full.node(n["id"].as_str().unwrap_or_default())["w"]);
    }
    assert!(nodes[0]["clips"].is_array());
    let tallest = nodes
        .iter()
        .map(|n| num(&full.node(n["id"].as_str().unwrap_or_default())["h"]))
        .fold(f64::MIN, f64::max);
    assert!(nodes.iter().all(|n| num(&n["h"]) == tallest));
    assert_eq!(
        nodes.iter().map(|n| n["y"].to_string()).collect::<HashSet<_>>().len(),
        1
    );
    // The box is the cards' height and no more, so a height fit makes the tallest card fill the view.
    assert_eq!(num(&r["height"]) - num(&r["top"]), tallest);
}

#[test]
fn builds_its_cards_with_the_machines_lists_so_a_midi_devices_dropdown_offers_them_here_too() {
    let keys = doc(
        json!({"vox": {"from": "vox", "use": [{"module": "keys", "device": ""}, {"module": "comp"}], "out": [{"to": "OUT", "gain": 1}]}}),
        json!(["vox"]),
    );
    let r = row(
        &keys,
        &["keys", "comp"],
        json!({"asDrawn": true, "midiDevices": ["KeyStep 37"]}),
    );
    let card = module(&list(&r["nodes"]), "keys").clone();
    let labels: Vec<String> = list(&card["ui"]["widgets"]["device"]["options"])
        .iter()
        .map(|o| text(&o["label"]))
        .collect();
    assert_eq!(labels, ["any", "KeyStep 37"]);
}

#[test]
fn names_the_lane_by_kind_so_a_module_of_the_same_name_is_not_mistaken_for_it() {
    let clash = doc(
        json!({"vox": {"from": "vox", "use": [{"module": "vox"}], "out": [{"to": "OUT", "gain": 1}]}}),
        json!(["vox"]),
    );
    let r = row(&clash, &["vox"], json!({"links": true, "track": "vox"}));
    assert_eq!(ids(&r), ["trk:vox", "mod:vox"]);
}

#[test]
fn draws_no_lone_lane_a_row_with_no_module_is_not_a_signal_path() {
    assert_eq!(
        row(&lane(), &[], json!({"links": true, "track": "vox"}))["nodes"],
        json!([])
    );
}

#[test]
fn leaves_the_row_alone_when_no_lane_is_named() {
    assert_eq!(
        ids(&row(&lane(), &["comp", "eq"], json!({"links": true}))),
        ["mod:comp", "mod:eq"]
    );
}
