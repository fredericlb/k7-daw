// A lane's card in the built graph: orphans, video lanes, live inputs, sub lines, and a rack's pads on the node.

use super::*;
use crate::geom::node_size;
use crate::panel::Panel;
use serde_json::json;

fn vox_lane() -> Value {
    json!({"lead": {"from": "vox", "use": [{"module": "eq"}], "out": [{"to": "OUT", "gain": 1}]}})
}

#[test]
fn draws_a_track_no_chain_mentions_unwired_and_unlabelled() {
    let b = built(&doc(vox_lane(), json!(["vox", "spare"])), json!({}));
    let spare = b.node("trk:spare");
    assert_eq!(spare["kind"], "track");
    assert!(spare["sub"].is_null());
    assert!(
        b.edges
            .iter()
            .all(|e| e["source"] != "trk:spare" && e["target"] != "trk:spare")
    );
}

#[test]
fn leaves_a_routed_track_alone_no_marker_still_wired() {
    let b = built(&doc(vox_lane(), json!(["vox"])), json!({}));
    assert!(b.node("trk:vox")["sub"].is_null());
    assert!(b.pairs().contains(&"trk:vox->mod:eq".to_owned()));
}

#[test]
fn gives_an_orphan_the_same_mixer_strip_and_colour_as_any_other_track() {
    let g = doc_with(
        json!({}),
        json!(["spare"]),
        json!({"trackColors": {"spare": "#ff5706"}}),
    );
    let spare = built(&g, json!({})).node("trk:spare").clone();
    assert_eq!(spare["color"], "#ff5706");
    assert!(spare["ui"].is_object());
}

#[test]
fn draws_a_whole_songs_worth_of_tracks_before_any_chain_exists() {
    let mut ids = built(&doc(json!({}), json!(["a", "b", "c"])), json!({})).ids();
    ids.sort();
    assert_eq!(ids, ["trk:a", "trk:b", "trk:c"]);
}

#[test]
fn a_modules_sub_line_says_nothing_on_the_first_module_of_a_chain() {
    let g = doc(
        json!({"lead": {"from": "vox", "use": [{"module": "eq"}, {"module": "verb"}], "out": [{"to": "OUT", "gain": 1}]}}),
        json!(["vox"]),
    );
    let b = built(&g, json!({}));
    assert!(b.node("mod:eq")["sub"].is_null());
    assert!(b.node("mod:verb")["sub"].is_null());
}

#[test]
fn a_modules_sub_line_still_warns_when_a_module_is_shared_across_chains() {
    let g = doc(
        json!({
            "lead": {"from": "vox", "use": [{"module": "verb"}], "out": [{"to": "OUT", "gain": 1}]},
            "pad": {"from": "keys", "use": [{"module": "verb"}], "out": [{"to": "OUT", "gain": 1}]}
        }),
        json!(["vox", "keys"]),
    );
    assert_eq!(built(&g, json!({})).node("mod:verb")["sub"], "lead, pad");
}

fn with_video(chains: Value, tracks: Value) -> Value {
    doc_with(chains, tracks, json!({"videoTracks": ["main"]}))
}

#[test]
fn draws_an_unrouted_video_lane_as_an_orphan_exactly_as_an_audio_one() {
    let b = built(&with_video(json!({}), json!(["spare"])), json!({}));
    assert_eq!(b.node("trk:spare")["kind"], "track");
    assert!(b.node("trk:spare")["sub"].is_null());
    let main = b.node("trk:main");
    assert_eq!((&main["kind"], &main["video"]), (&json!("track"), &json!(true)));
    assert!(main["sub"].is_null());
}

#[test]
fn lands_a_cable_on_the_video_lane_not_on_a_sink_invented_for_it() {
    let b = built(
        &with_video(
            json!({"cam": {"from": "cam", "use": [], "out": [{"to": "main", "gain": 1}]}}),
            json!([]),
        ),
        json!({}),
    );
    assert!(b.of_kind("sink").iter().all(|n| n["label"] != "main"));
    assert!(b.edges.iter().any(|e| e["target"] == "trk:main"));
}

#[test]
fn gives_a_video_lane_a_mute_only_strip_not_the_mixers_five_controls() {
    let b = built(&with_video(json!({}), json!(["spare"])), json!({}));
    let keys = |id: &str| -> Vec<String> {
        b.node(id)["ui"]["widgets"]
            .as_object()
            .map(|w| w.keys().cloned().collect())
            .unwrap_or_default()
    };
    assert_eq!(keys("trk:main"), ["mute", "solo"]);
    assert_eq!(keys("trk:spare"), ["mute", "solo", "arm", "vol", "pan"]);
}

#[test]
fn routes_a_video_chain_and_puts_picture_on_the_cable() {
    let g = with_video(
        json!({"main": {"from": "main", "use": [{"module": "blur"}], "out": [{"to": "VIDEO_OUT", "gain": 1}]}}),
        json!([]),
    );
    let b = built(&g, json!({}));
    let main = b.node("trk:main");
    assert_eq!((&main["kind"], &main["video"]), (&json!("track"), &json!(true)));
    assert!(main["sub"].is_null());
    assert_eq!(
        b.edge_from("trk:main").map(|e| e["signal"].clone()),
        Some(json!("video"))
    );
}

#[test]
fn an_input_wired_to_a_track_draws_the_arrow_to_the_track_not_to_its_first_module() {
    let g = doc(
        json!({
            "IN:in-2": {"use": [{"module": "IN:in-2"}], "out": [{"to": "gtr", "gain": 1}]},
            "gtr": {"from": "gtr", "use": [{"module": "wah"}], "out": [{"to": "OUT", "gain": 1}]}
        }),
        json!(["gtr"]),
    );
    let b = built(&g, json!({}));
    assert_eq!(
        b.edge_from("mod:IN:in-2").map(|e| e["target"].clone()),
        Some(json!("trk:gtr"))
    );
}

fn kit() -> Value {
    json!([{"symbol": "BD", "note": 36, "clip": "kick"}, {"symbol": "HH", "note": 42, "clip": "hat"}, {"symbol": "SN", "note": 38, "clip": "snare"}])
}

fn rack_graph(step: Value) -> Value {
    doc(
        json!({"beat": {"from": "beat", "use": [step], "out": [{"to": "OUT", "gain": 1}]}}),
        json!(["beat"]),
    )
}

#[test]
fn a_rack_node_has_its_pads_on_the_node_and_is_sized_to_them() {
    let b = built(&rack_graph(json!({"module": "kit", "rack": kit()})), json!({}));
    let node = module(&b.nodes, "kit");
    assert_eq!(node["rack"].as_array().map(Vec::len), Some(3));
    assert!(
        node["ui"]["tabs"][0]["widgets"]
            .as_object()
            .is_some_and(|w| w.contains_key("rack:BD"))
    );
    let ui: Panel = serde_json::from_value(node["ui"].clone()).expect("a panel");
    assert_eq!(node["w"].as_f64(), Some(node_size(Some(&ui), false).0));
}

#[test]
fn a_rack_node_leaves_an_authored_panel_alone() {
    let own = json!({"cols": 1, "rows": 1, "widgets": {"g": {"type": "knob", "col": 0, "row": 0, "bind": "Gain"}}});
    let b = built(
        &rack_graph(json!({"module": "kit", "rack": kit(), "ui": own})),
        json!({}),
    );
    let keys: Vec<String> = module(&b.nodes, "kit")["ui"]["widgets"]
        .as_object()
        .map(|w| w.keys().cloned().collect())
        .unwrap_or_default();
    assert_eq!(keys, ["g"]);
}
