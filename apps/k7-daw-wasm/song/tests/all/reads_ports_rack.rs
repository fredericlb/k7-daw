// reads_ports_rack.rs — the rack-hit cases the page's `rackLit` suite held, as the same inputs and hits.
// A reader would get wrong that the hits are in the ARRANGEMENT's beats: a clip's start is added to each note's time.

use serde_json::{Value, json};
use song::reads::*;

fn tracks(v: Value) -> Vec<RTrack> {
    serde_json::from_value(v).expect("tracks")
}

fn kit_graph() -> RGraph {
    serde_json::from_value(json!({ "chains": { "beat": { "from": "beat", "out": [],
        "use": [{ "module": "kit", "rack": [{ "symbol": "BD", "note": 36 }, { "symbol": "SN", "note": 38 }] }] } } }))
    .expect("graph")
}

fn one_lane(start: f64, notes: Value) -> Vec<RTrack> {
    tracks(json!([{ "id": "beat", "clips": [{ "id": "c", "type": "midi", "start": start, "notes": notes }] }]))
}

fn triples(hits: &[Hit]) -> Vec<(f64, f64, f64)> {
    hits.iter().map(|h| (h.from, h.to, h.key)).collect()
}

#[test]
fn a_rack_gets_the_notes_of_the_lane_feeding_its_chain_in_arrangement_beats() {
    let t = one_lane(4.0, json!([{ "time": 1, "duration": 2, "key": 36 }]));
    assert_eq!(triples(&rack_hits(&t, &kit_graph())["kit"]), vec![(5.0, 7.0, 36.0)]);
}

#[test]
fn a_very_short_note_is_floored_so_a_drum_hit_does_not_flash_between_two_frames() {
    let t = one_lane(0.0, json!([{ "time": 0, "duration": 0.05, "key": 36 }]));
    assert_eq!(rack_hits(&t, &kit_graph())["kit"][0].to, 0.25);
}

#[test]
fn a_looped_events_window_tiles_across_its_placed_length_and_the_last_pass_is_cut() {
    let t = tracks(
        json!([{ "id": "beat", "clips": [{ "id": "c", "type": "midi", "start": 8, "length": 5,
        "loopStart": 4, "loopEnd": 6, "notes": [
            { "time": 0, "duration": 1, "key": 40 },
            { "time": 4, "duration": 1, "key": 36 },
            { "time": 5, "duration": 1, "key": 38 }] }] }]),
    );
    let got: Vec<(f64, f64)> = rack_hits(&t, &kit_graph())["kit"]
        .iter()
        .map(|h| (h.from, h.key))
        .collect();
    assert_eq!(
        got,
        vec![(8.0, 36.0), (9.0, 38.0), (10.0, 36.0), (11.0, 38.0), (12.0, 36.0)]
    );
}

#[test]
fn a_chain_with_no_rack_or_a_lane_with_no_notes_gives_nothing() {
    assert!(rack_hits(&one_lane(0.0, json!([])), &kit_graph()).is_empty());
    let no_rack: RGraph = serde_json::from_value(
        json!({ "chains": { "beat": { "from": "beat", "use": [{ "module": "amp" }], "out": [] } } }),
    )
    .expect("graph");
    let t = one_lane(0.0, json!([{ "time": 0, "duration": 1, "key": 36 }]));
    assert!(rack_hits(&t, &no_rack).is_empty());
}

#[test]
fn a_module_reached_from_two_chains_is_one_list_sorted_by_start() {
    let t = tracks(json!([
        { "id": "late", "clips": [{ "id": "l", "type": "midi", "start": 8, "notes": [{ "time": 0, "duration": 1, "key": 38 }] }] },
        { "id": "early", "clips": [{ "id": "e", "type": "midi", "start": 0, "notes": [{ "time": 0, "duration": 1, "key": 36 }] }] },
    ]));
    let rack = json!([{ "symbol": "BD", "note": 36 }]);
    let g: RGraph = serde_json::from_value(json!({ "chains": {
        "late": { "from": "late", "use": [{ "module": "kit", "rack": rack }], "out": [] },
        "early": { "from": "early", "use": [{ "module": "kit", "rack": rack }], "out": [] },
    } }))
    .expect("graph");
    assert_eq!(
        rack_hits(&t, &g)["kit"].iter().map(|h| h.from).collect::<Vec<_>>(),
        vec![0.0, 8.0]
    );
}
