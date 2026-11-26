// The TypeScript tests of flow.ts and of cables.ts's `computeFlow`, with their inputs and expected values.

use super::*;

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6, "{a} vs {b}");
}

#[test]
fn an_audio_cable_is_linear_in_decibels_and_dead_at_the_floor() {
    assert_eq!(audio_flow(0.0), 1.0);
    close(audio_flow(FLOW_FLOOR_DB + 1.0), 1.0 / 80.0);
    assert_eq!(audio_flow(FLOW_FLOOR_DB), STILL);
    assert_eq!(audio_flow(-120.0), STILL);
    assert_eq!(audio_flow(f64::NEG_INFINITY), STILL);
    close(audio_flow(-40.0), 0.5);
    close(audio_flow(-20.0), 0.75);
    close(audio_flow(-60.0), 0.25);
    assert_eq!(audio_flow(6.0), 1.0);
}

#[test]
fn a_midi_cable_counts_velocity_against_the_busiest_moment() {
    assert_eq!(midi_flow(2.0, 4.0), 0.5);
    assert_eq!(midi_flow(4.0, 4.0), 1.0);
    assert_eq!(midi_flow(4.0 * 0.25, 4.0), midi_flow(1.0, 4.0));
    assert_eq!(midi_flow(0.0, 4.0), STILL);
    assert_eq!(midi_flow(2.0, 0.0), STILL);
    assert_eq!(midi_flow(f64::NAN, 4.0), STILL);
}

#[test]
fn a_video_cable_is_binary() {
    assert_eq!(video_flow(true), 1.0);
    assert_eq!(video_flow(false), STILL);
}

#[test]
fn a_cable_reads_the_number_matching_what_it_carries() {
    let seed = NodeFlow { audio: 0.2, midi: 0.6, video: 1.0 };
    assert_eq!(flow_of(&seed, Some("audio")), 0.2);
    assert_eq!(flow_of(&seed, Some("midi")), 0.6);
    assert_eq!(flow_of(&seed, Some("video")), 1.0);
    assert_eq!(flow_of(&seed, Some("event")), 0.6);
    assert_eq!(flow_of(&seed, None), 0.2);
}

fn lanes(v: serde_json::Value) -> IndexMap<String, LaneSource> {
    serde_json::from_value(v).unwrap_or_default()
}

#[test]
fn a_lane_gives_all_three_numbers_so_its_two_wires_can_differ() {
    let seeds = seed_flows(&lanes(serde_json::json!({"melody": {"db": -40, "notes": {"load": 4, "peak": 4}}})));
    assert_eq!(seeds.get("melody"), Some(&NodeFlow { audio: 0.5, midi: 1.0, video: 0.0 }));
}

#[test]
fn a_lane_that_puts_out_nothing_is_left_out_and_a_silent_picture_kept() {
    assert!(seed_flows(&lanes(serde_json::json!({"quiet": {"db": -100}}))).is_empty());
    assert!(seed_flows(&lanes(serde_json::json!({"nothing": {}}))).is_empty());
    let cam = seed_flows(&lanes(serde_json::json!({"cam": {"showing": true}})));
    assert_eq!(cam.get("cam"), Some(&NodeFlow { audio: 0.0, midi: 0.0, video: 1.0 }));
}

fn edge(id: &str, source: &str, target: &str, signal: Option<&str>) -> FlowEdge {
    FlowEdge { id: id.into(), source: source.into(), target: target.into(), signal: signal.map(Into::into) }
}

fn live(edges: &[FlowEdge], audio: &[&str], video: &[&str], folded: &[(&str, &str)]) -> Vec<String> {
    let mut seeds: Vec<(String, NodeFlow)> =
        audio.iter().map(|id| ((*id).to_owned(), NodeFlow { audio: 1.0, midi: 1.0, video: 0.0 })).collect();
    seeds.extend(video.iter().map(|id| ((*id).to_owned(), NodeFlow { audio: 0.0, midi: 0.0, video: 1.0 })));
    let folded = folded.iter().map(|(a, b)| ((*a).to_owned(), (*b).to_owned())).collect();
    let mut out: Vec<String> = compute_flow(edges, &seeds, &folded).into_iter().filter(|(_, v)| *v > 0.0).map(|(k, _)| k).collect();
    out.sort();
    out
}

#[test]
fn the_live_set_over_a_folded_assembly_needs_the_map() {
    let edges = [edge("a", "asm:turntableA", "snk:CUE", None), edge("b", "snk:MIX", "snk:OUT", None)];
    assert_eq!(live(&edges, &["deckA"], &[], &[]), vec!["b"]);
    assert_eq!(live(&edges, &["deckA"], &[], &[("deckA", "asm:turntableA")]), vec!["a", "b"]);
    assert_eq!(live(&edges, &["deckA"], &[], &[("deckB", "asm:turntableB")]), vec!["b"]);
}

#[test]
fn the_live_set_flows_forward_from_active_tracks_only_and_ends_on_a_loop() {
    let edges = [edge("a", "trk:melody", "m1", None), edge("b", "m1", "out", None), edge("c", "other", "out", None)];
    assert_eq!(live(&edges, &["melody"], &[], &[]), vec!["a", "b"]);
    assert!(live(&edges, &[], &[], &[]).is_empty());
    let cycle = [edge("in", "trk:vox", "m1", None), edge("fwd", "m1", "m2", None), edge("back", "m2", "m1", None)];
    assert_eq!(live(&cycle, &["vox"], &[], &[]), vec!["back", "fwd", "in"]);
}

#[test]
fn the_master_run_is_seeded_at_the_sum() {
    let edges = [
        edge("lane", "trk:melody", "mod:reverb", None),
        edge("master", MIX_NODE_ID, "mod:comp", None),
        edge("device", "mod:comp", "snk:OUT", None),
    ];
    assert_eq!(live(&edges, &["melody"], &[], &[]), vec!["device", "lane", "master"]);
    assert_eq!(live(&edges, &["pad"], &[], &[]), vec!["device", "master"]);
    assert!(live(&edges, &[], &[], &[]).is_empty());
}

#[test]
fn the_picture_lights_a_video_lane_and_the_run_out_of_render() {
    let edges = [
        edge("lane", "trk:cam", "mod:blur", Some("video")),
        edge("master", RENDER_NODE_ID, "snk:VIDEO_OUT", Some("video")),
        edge("sound", "trk:melody", "mod:reverb", None),
    ];
    assert_eq!(live(&edges, &[], &["cam"], &[]), vec!["lane", "master"]);
    assert_eq!(live(&edges, &["melody"], &[], &[]), vec!["sound"]);
}

#[test]
fn the_walk_goes_up_from_a_meter_and_stops_at_another_lane() {
    let lane = "trk:gtr";
    let edges = [edge("in", "mod:IN", lane, None), edge("out", lane, "mod:amp", None)];
    assert_eq!(live(&edges, &["gtr"], &[], &[]), vec!["in", "out"]);
    let chain = [edge("a", "mod:IN", "mod:amp", None), edge("b", "mod:amp", lane, None)];
    assert_eq!(live(&chain, &["gtr"], &[], &[]), vec!["a", "b"]);
    let other = [edge("pad", "trk:pad", lane, None), edge("in", "mod:IN", lane, None)];
    assert_eq!(live(&other, &["gtr"], &[], &[]), vec!["in"]);
    assert!(live(&[edge("in", "mod:IN", lane, None)], &[], &[], &[]).is_empty());
    let sibling = [edge("in", "mod:IN", lane, None), edge("other", "mod:IN", "trk:quiet", None)];
    assert_eq!(live(&sibling, &["gtr"], &[], &[]), vec!["in"]);
}

#[test]
fn a_materialised_track_seeds_at_the_module_it_was_named_after() {
    let edges = [edge("a", "mod:IN", "mod:fader", None), edge("b", "mod:fader", MIX_NODE_ID, None), edge("c", MIX_NODE_ID, "snk:OUT", None)];
    assert_eq!(live(&edges, &["IN"], &[], &[]), vec!["a", "b", "c"]);
    assert!(live(&edges[..1], &[], &[], &[]).is_empty());
}

#[test]
fn each_wire_ripples_by_what_it_carries_and_a_still_one_is_absent() {
    let edges = [edge("notes", "trk:melody", "mod:piano", Some("midi")), edge("sound", "mod:piano", "snk:MIX", Some("audio"))];
    let seeds = [("melody".to_owned(), NodeFlow { audio: 0.25, midi: 1.0, video: 0.0 })];
    let flow = compute_flow(&edges, &seeds, &HashMap::new());
    assert_eq!(flow.get("notes"), Some(&1.0));
    assert_eq!(flow.get("sound"), Some(&0.25));
    let picture = [("melody".to_owned(), NodeFlow { audio: 0.0, midi: 0.0, video: 1.0 })];
    assert!(compute_flow(&edges, &picture, &HashMap::new()).is_empty());
}

#[test]
fn two_lanes_meeting_on_one_cable_take_the_louder() {
    let merged = [edge("a", "trk:one", "mod:bus", None), edge("b", "trk:two", "mod:bus", None), edge("out", "mod:bus", "snk:MIX", None)];
    let seeds = [
        ("one".to_owned(), NodeFlow { audio: 0.2, midi: 0.0, video: 0.0 }),
        ("two".to_owned(), NodeFlow { audio: 0.9, midi: 0.0, video: 0.0 }),
    ];
    assert_eq!(compute_flow(&merged, &seeds, &HashMap::new()).get("out"), Some(&0.9));
}
