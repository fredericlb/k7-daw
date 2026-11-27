// The two sums a build draws, MIX and RENDER, and the runs and devices around them.

use super::*;
use serde_json::json;

fn song_chains() -> Value {
    json!({
        "melody": {"from": "melody", "use": [{"module": "lead"}], "out": [{"to": "MIX", "gain": 1}]},
        "pad": {"from": "pad", "use": [{"module": "synth-pad"}], "out": [{"to": "MIX", "gain": 1}]},
        "MIX": {"from": "MIX", "use": [], "out": [{"to": "OUT", "gain": 1}]}
    })
}

fn song() -> Value {
    doc(song_chains(), json!(["melody", "pad"]))
}

fn song_with_master(use_: Value) -> Value {
    let mut g = song();
    g["chains"]["MIX"] = json!({"from": "MIX", "use": use_, "out": [{"to": "OUT", "gain": 1}]});
    g
}

fn rank(nodes: &[Value], id: &str) -> f64 {
    card(nodes, id)["rank"].as_f64().unwrap_or(0.0)
}

fn lane_max(nodes: &[Value]) -> f64 {
    nodes
        .iter()
        .filter(|n| n["kind"] == "module" && n["id"] != "mod:comp")
        .map(|n| n["rank"].as_f64().unwrap_or(0.0))
        .fold(f64::MIN, f64::max)
}

#[test]
fn draws_no_cable_from_a_lanes_chain_into_mix() {
    let b = built(&song(), json!({}));
    assert!(b.edges.iter().all(|e| e["target"] != "snk:MIX"));
    assert_eq!(
        b.pairs(),
        ["snk:MIX->snk:OUT", "trk:melody->mod:lead", "trk:pad->mod:synth-pad"]
    );
}

#[test]
fn draws_mix_once_as_a_sink_with_the_master_run_leaving_it() {
    let b = built(&song(), json!({}));
    let mixes: Vec<String> = b
        .nodes
        .iter()
        .filter(|n| n["label"] == "MIX")
        .map(|n| text(&n["id"]))
        .collect();
    assert_eq!(mixes, ["snk:MIX"]);
    assert_eq!(b.node("snk:MIX")["kind"], "sink");
}

#[test]
fn puts_the_master_chains_modules_between_mix_and_out() {
    let b = built(
        &song_with_master(json!([{"module": "comp"}, {"module": "eq"}])),
        json!({}),
    );
    let mut around: Vec<String> = b
        .edges
        .iter()
        .filter(|e| text(&e["source"]).starts_with("snk:") || e["target"] == "snk:OUT")
        .map(pair)
        .collect();
    around.sort();
    assert_eq!(around, ["mod:eq->snk:OUT", "snk:MIX->mod:comp"]);
}

#[test]
fn ranks_the_whole_master_run_after_every_lane_in_its_own_order() {
    let b = built(&song_with_master(json!([{"module": "comp"}])), json!({}));
    assert!(rank(&b.nodes, "snk:MIX") > lane_max(&b.nodes));
    assert!(rank(&b.nodes, "snk:MIX") < rank(&b.nodes, "mod:comp"));
    assert!(rank(&b.nodes, "mod:comp") < rank(&b.nodes, "snk:OUT"));
}

#[test]
fn does_not_rank_a_master_module_by_its_place_inside_the_run() {
    let b = built(&song_with_master(json!([{"module": "comp"}])), json!({}));
    assert!(rank(&b.nodes, "mod:comp") > lane_max(&b.nodes));
}

#[test]
fn does_not_push_mix_when_the_strips_are_aligned() {
    let b = built(&song(), json!({}));
    let was = rank(&b.nodes, "snk:MIX");
    let aligned = b.aligned(&["lead", "synth-pad"]);
    assert_eq!(rank(&aligned, "snk:MIX"), was);
    assert!(rank(&aligned, "snk:MIX") < rank(&aligned, "snk:OUT"));
}

#[test]
fn gives_mix_an_output_socket_and_no_input() {
    let b = built(&song(), json!({}));
    let mix = b.node("snk:MIX");
    assert_eq!(mix["jacks"]["in"], json!([]));
    assert_eq!(mix["jacks"]["out"].as_array().map(Vec::len), Some(1));
    assert_eq!(b.node("snk:OUT")["jacks"]["out"], json!([]));
}

#[test]
fn draws_mix_to_out_even_when_the_file_declares_no_master_run() {
    let g = doc(
        json!({"melody": {"from": "melody", "use": [{"module": "lead"}], "out": [{"to": "MIX", "gain": 1}]}}),
        json!(["melody"]),
    );
    let b = built(&g, json!({}));
    assert_eq!(b.node("snk:MIX")["kind"], "sink");
    assert_eq!(b.pairs(), ["snk:MIX->snk:OUT", "trk:melody->mod:lead"]);
}

#[test]
fn leaves_out_out_of_that_pair_when_draw_main_out_is_off() {
    let g = doc(
        json!({"melody": {"use": [{"module": "lead"}], "out": [{"to": "MIX", "gain": 1}]}}),
        json!([]),
    );
    let b = built(&g, json!({"drawMainOut": false}));
    assert!(b.find("snk:OUT").is_none());
    assert!(b.edges.iter().all(|e| e["source"] != "snk:MIX"));
}

fn film() -> Value {
    let mut chains = song_chains();
    let extra = json!({
        "main": {"from": "main", "use": [{"module": "grade"}], "out": [{"to": "RENDER", "gain": 1}]},
        "pip": {"from": "pip", "use": [{"module": "corner"}], "out": [{"to": "RENDER", "gain": 1}]},
        "RENDER": {"from": "RENDER", "use": [], "out": [{"to": "VIDEO_OUT", "gain": 1}]}
    });
    if let (Some(c), Some(x)) = (chains.as_object_mut(), extra.as_object()) {
        c.extend(x.clone());
    }
    doc_with(
        chains,
        json!(["melody", "pad"]),
        json!({"videoTracks": ["main", "pip"]}),
    )
}

#[test]
fn draws_each_video_chains_cable_into_render_and_still_none_into_mix() {
    let b = built(&film(), json!({}));
    assert!(b.edges.iter().all(|e| e["target"] != "snk:MIX"));
    assert_eq!(
        b.pairs(),
        [
            "mod:corner->snk:RENDER",
            "mod:grade->snk:RENDER",
            "snk:MIX->snk:OUT",
            "snk:RENDER->snk:VIDEO_OUT",
            "trk:main->mod:grade",
            "trk:melody->mod:lead",
            "trk:pad->mod:synth-pad",
            "trk:pip->mod:corner",
        ]
    );
}

#[test]
fn gives_render_a_picture_socket_and_a_sound_socket_the_mixs_cable_landing_on_the_second() {
    let mut g = film();
    g["chains"]["MIX"] =
        json!({"from": "MIX", "use": [], "out": [{"to": "OUT", "gain": 1}, {"to": "RENDER#audio", "gain": 1}]});
    let b = built(&g, json!({}));
    let ins = &b.node("snk:RENDER")["jacks"]["in"];
    let seen: Vec<(Value, Value, Value)> = ins
        .as_array()
        .map(|a| {
            a.iter()
                .map(|j| (j["name"].clone(), j["type"].clone(), j["free"].clone()))
                .collect()
        })
        .unwrap_or_default();
    assert_eq!(
        seen,
        [
            (json!("in"), json!("video"), json!(false)),
            (json!("audio"), json!("audio"), json!(false))
        ]
    );
    let sound = b
        .edges
        .iter()
        .find(|e| e["targetPort"] == "audio")
        .expect("a sound cable");
    assert_eq!(
        (&sound["source"], &sound["target"], &sound["signal"]),
        (&json!("snk:MIX"), &json!("snk:RENDER"), &json!("audio"))
    );
}

#[test]
fn shows_renders_sound_socket_free_when_nothing_feeds_it() {
    let b = built(&film(), json!({}));
    let ins = b.node("snk:RENDER")["jacks"]["in"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let seen: Vec<(Value, Value)> = ins.iter().map(|j| (j["name"].clone(), j["free"].clone())).collect();
    assert_eq!(seen, [(json!("in"), json!(false)), (json!("audio"), json!(true))]);
}

#[test]
fn draws_render_once_as_a_sink_with_the_video_master_run_leaving_it() {
    let b = built(&film(), json!({}));
    let renders: Vec<String> = b
        .nodes
        .iter()
        .filter(|n| n["label"] == "RENDER")
        .map(|n| text(&n["id"]))
        .collect();
    assert_eq!(renders, ["snk:RENDER"]);
    assert_eq!(b.node("snk:RENDER")["kind"], "sink");
}

#[test]
fn carries_picture_on_the_wire_out_of_render() {
    let b = built(&film(), json!({}));
    assert_eq!(
        b.edge_from("snk:RENDER").map(|e| e["signal"].clone()),
        Some(json!("video"))
    );
    assert_eq!(
        b.edge_from("snk:MIX").map(|e| e["signal"].clone()),
        Some(json!("audio"))
    );
}

#[test]
fn puts_the_video_master_chains_modules_between_render_and_video_out() {
    let mut g = film();
    g["chains"]["RENDER"] =
        json!({"from": "RENDER", "use": [{"module": "blur"}], "out": [{"to": "VIDEO_OUT", "gain": 1}]});
    let b = built(&g, json!({}));
    let mut around: Vec<String> = b
        .edges
        .iter()
        .filter(|e| e["source"] == "snk:RENDER" || e["target"] == "snk:VIDEO_OUT")
        .map(pair)
        .collect();
    around.sort();
    assert_eq!(around, ["mod:blur->snk:VIDEO_OUT", "snk:RENDER->mod:blur"]);
}

#[test]
fn gives_video_out_no_fader_a_frame_has_no_level() {
    let b = built(&film(), json!({"mainOutVolume": -6}));
    assert!(b.node("snk:OUT")["ui"].is_object());
    assert!(b.node("snk:VIDEO_OUT")["ui"].is_null());
    assert!(b.node("snk:VIDEO_OUT")["params"].is_null());
}

#[test]
fn does_not_push_render_when_the_strips_are_aligned() {
    let b = built(&film(), json!({}));
    let was = rank(&b.nodes, "snk:RENDER");
    let aligned = b.aligned(&["lead", "synth-pad"]);
    assert_eq!(rank(&aligned, "snk:RENDER"), was);
    assert!(rank(&aligned, "snk:RENDER") < rank(&aligned, "snk:VIDEO_OUT"));
}

#[test]
fn draws_render_to_video_out_even_when_the_file_declares_no_video_master_run() {
    let mut g = film();
    if let Some(c) = g["chains"].as_object_mut() {
        c.shift_remove("RENDER");
    }
    let b = built(&g, json!({}));
    assert_eq!(b.node("snk:RENDER")["kind"], "sink");
    assert!(b.pairs().contains(&"snk:RENDER->snk:VIDEO_OUT".to_owned()));
}

#[test]
fn leaves_video_out_out_of_that_pair_when_draw_main_out_is_off() {
    let b = built(&film(), json!({"drawMainOut": false}));
    assert!(b.find("snk:VIDEO_OUT").is_none());
    assert!(b.find("snk:OUT").is_none());
    assert!(b.edges.iter().all(|e| e["source"] != "snk:RENDER"));
}

#[test]
fn leaves_a_song_with_no_picture_untouched() {
    let g = doc(
        json!({
            "melody": {"from": "melody", "use": [{"module": "lead"}], "out": [{"to": "MIX", "gain": 1}]},
            "MIX": {"from": "MIX", "use": [], "out": [{"to": "OUT", "gain": 1}]}
        }),
        json!(["melody"]),
    );
    let b = built(&g, json!({}));
    assert!(
        b.nodes
            .iter()
            .all(|n| n["label"] != "RENDER" && n["label"] != "VIDEO_OUT")
    );
}
