// A track node lists what its lane holds, on a mini timeline whose scale every lane shares.

use super::*;
use crate::geom::CLIP_LIST_MAX;
use serde_json::json;

/// `tc` is spelled by the file service in its shortest form (`5:1`).
fn clip(name: &str, start: u32, len: Option<u32>) -> Value {
    let mut c =
        json!({"ref": name, "start": start, "tc": format!("{}:{}", start / 4 + 1, start % 4 + 1), "kind": "audio"});
    if let Some(len) = len {
        c["len"] = json!(len);
    }
    c
}

fn graph_with(clips: Option<Vec<Value>>) -> Value {
    let g = doc(
        json!({"gtr": {"from": "gtr", "use": [{"module": "amp"}], "out": [{"to": "OUT", "gain": 1}]}}),
        json!(["gtr"]),
    );
    match clips {
        Some(c) => doc_with(
            g["chains"].clone(),
            g["tracks"].clone(),
            json!({"trackClips": {"gtr": c}}),
        ),
        None => g,
    }
}

fn track_node(g: &Value) -> Value {
    let b = built(g, json!({"drawMainOut": false}));
    b.of_kind("track").first().map(|n| (*n).clone()).expect("a track node")
}

fn takes(n: usize) -> Vec<Value> {
    (0..n)
        .map(|i| clip(&format!("take-{i}"), (i * 4) as u32, None))
        .collect()
}

#[test]
fn carries_the_lanes_clips_each_with_the_position_it_plays_at() {
    let n = track_node(&graph_with(Some(vec![
        clip("verse", 0, None),
        clip("chorus", 16, None),
    ])));
    let refs: Vec<Value> = list(&n["clips"]).iter().map(|c| c["ref"].clone()).collect();
    let tcs: Vec<Value> = list(&n["clips"]).iter().map(|c| c["tc"].clone()).collect();
    assert_eq!(
        (refs, tcs),
        (vec![json!("verse"), json!("chorus")], vec![json!("1:1"), json!("5:1")])
    );
}

#[test]
fn a_lane_with_clips_is_taller_than_the_same_lane_with_nothing_on_it_and_no_wider() {
    let empty = track_node(&graph_with(None));
    let full = track_node(&graph_with(Some(vec![clip("verse", 0, None)])));
    assert!(num(&full["h"]) > num(&empty["h"]));
    assert_eq!(full["w"], empty["w"]);
}

#[test]
fn adds_nothing_to_a_lane_with_no_clips() {
    let n = track_node(&graph_with(None));
    assert!(n["clips"].is_null() && n["clipsMore"].is_null());
}

#[test]
fn counts_the_lists_overflow_while_keeping_every_clip_for_the_timeline() {
    let n = track_node(&graph_with(Some(takes(CLIP_LIST_MAX + 4))));
    assert_eq!(list(&n["clips"]).len(), CLIP_LIST_MAX + 4);
    assert_eq!(n["clipsMore"], json!(4.0));
}

#[test]
fn says_nothing_extra_when_the_list_fits_exactly() {
    let n = track_node(&graph_with(Some(takes(CLIP_LIST_MAX))));
    assert_eq!(list(&n["clips"]).len(), CLIP_LIST_MAX);
    assert!(n["clipsMore"].is_null());
}

fn two_lanes(a: Vec<Value>, b: Vec<Value>) -> Value {
    doc_with(
        json!({
            "gtr": {"from": "gtr", "use": [], "out": [{"to": "OUT", "gain": 1}]},
            "bass": {"from": "bass", "use": [], "out": [{"to": "OUT", "gain": 1}]}
        }),
        json!(["gtr", "bass"]),
        json!({"trackClips": {"gtr": a, "bass": b}}),
    )
}

fn at(g: &Value, track: &str) -> Vec<Value> {
    let b = built(g, json!({"drawMainOut": false}));
    let n = b
        .nodes
        .iter()
        .find(|n| n["kind"] == "track" && n["label"] == track)
        .expect("a track");
    list(&n["clips"]).iter().map(|c| c["at"].clone()).collect()
}

#[test]
fn the_scale_is_shared_by_every_lane_not_each_lanes_own_extent() {
    let g = two_lanes(
        vec![clip("a", 0, Some(4)), clip("b", 96, Some(32))],
        vec![clip("c", 0, Some(4)), clip("d", 32, Some(4))],
    );
    // The span is the end of the last clip, 96 + 32 = 128, so the longest lane fills it.
    assert_eq!(at(&g, "gtr"), [json!(0.0), json!(0.75)]);
    // A quarter of the way along, because 32 is a quarter of 128, not at its own end.
    assert_eq!(at(&g, "bass"), [json!(0.0), json!(0.25)]);
}

#[test]
fn the_scale_runs_to_the_end_of_the_last_clip_not_its_start() {
    let g = two_lanes(vec![clip("a", 0, Some(64))], vec![clip("b", 0, Some(4))]);
    let b = built(&g, json!({"drawMainOut": false}));
    let gtr = b
        .nodes
        .iter()
        .find(|n| n["kind"] == "track" && n["label"] == "gtr")
        .expect("gtr");
    assert_eq!(
        (gtr["clips"][0]["at"].clone(), gtr["clips"][0]["w"].clone()),
        (json!(0.0), json!(1.0))
    );
}

#[test]
fn a_very_short_clip_gets_a_width_you_can_still_hit() {
    let g = two_lanes(
        vec![clip("a", 0, Some(1)), clip("b", 400, Some(4))],
        vec![clip("c", 0, Some(4))],
    );
    let b = built(&g, json!({"drawMainOut": false}));
    let gtr = b
        .nodes
        .iter()
        .find(|n| n["kind"] == "track" && n["label"] == "gtr")
        .expect("gtr");
    assert!(num(&gtr["clips"][0]["w"]) > 0.01);
}

#[test]
fn puts_everything_at_the_start_when_nothing_is_past_bar_1() {
    let g = two_lanes(vec![clip("a", 0, None)], vec![clip("b", 0, None)]);
    assert_eq!(at(&g, "gtr"), [json!(0.0)]);
    assert_eq!(at(&g, "bass"), [json!(0.0)]);
}

#[test]
fn keeps_a_missing_clip_and_says_which_one_it_is() {
    let n = track_node(&graph_with(Some(vec![
        json!({"ref": "gone", "start": 0, "tc": "1:1", "kind": "missing"}),
    ])));
    assert_eq!(
        (n["clips"][0]["ref"].clone(), n["clips"][0]["kind"].clone()),
        (json!("gone"), json!("missing"))
    );
}
