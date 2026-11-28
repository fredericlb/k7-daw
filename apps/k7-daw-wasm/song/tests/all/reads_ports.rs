// reads_ports.rs — the cases the page's `clipLoop`, `videoSpans`, `noteSpans`, `clipInstances` and `midiIn` suites held, as the same inputs and answers.
// A reader would get wrong that the picture-lane cases lost their compositor cross-check: an expected span replaces it.

use serde_json::{Value, json};
use song::reads::*;

fn tracks(v: Value) -> Vec<RTrack> {
    serde_json::from_value(v).expect("tracks")
}

fn lanes(v: Value) -> Vec<RVideoTrack> {
    serde_json::from_value(v).expect("lanes")
}

fn graph(v: Value) -> RGraph {
    serde_json::from_value(v).expect("graph")
}

#[test]
fn a_clip_that_states_no_loop_or_a_backwards_window_has_none() {
    assert!(clip_loop("midi", 0.0, Some(0.0), Some(0.0)).is_none());
    assert!(clip_loop("midi", 0.0, Some(8.0), Some(4.0)).is_none());
}

#[test]
fn a_placed_length_is_counted_in_passes_of_the_window() {
    let l = clip_loop("midi", 80.0, Some(0.0), Some(8.0)).expect("a loop");
    assert_eq!((l.from, l.to, l.win, l.passes), (0.0, 8.0, 8.0, 10.0));
    assert_eq!(l.repeats, vec![8.0, 16.0, 24.0, 32.0, 40.0, 48.0, 56.0, 64.0, 72.0]);
    let l = clip_loop("midi", 8.0, Some(8.0), Some(12.0)).expect("a loop");
    assert_eq!((l.from, l.to, l.win, l.passes), (8.0, 12.0, 4.0, 2.0));
    assert_eq!(l.repeats, vec![12.0]);
}

#[test]
fn an_audio_region_opens_at_zero_whatever_beats_into_the_source_the_loop_says() {
    let l = clip_loop("audio", 12.0, Some(40.0), Some(44.0)).expect("a loop");
    assert_eq!((l.from, l.to, l.win, l.passes), (0.0, 4.0, 4.0, 3.0));
    assert_eq!(l.repeats, vec![4.0, 8.0]);
}

#[test]
fn a_window_played_once_has_no_repeats_and_an_unknown_length_is_one_pass() {
    let once = clip_loop("midi", 4.0, Some(0.0), Some(4.0)).expect("a loop");
    assert_eq!(once.passes, 1.0);
    assert!(once.repeats.is_empty());
    assert_eq!(
        clip_loop("midi", 2.0, Some(0.0), Some(8.0)).map(|l| l.passes),
        Some(1.0)
    );
    let open = clip_loop("midi", 0.0, Some(0.0), Some(8.0)).expect("a loop");
    assert_eq!((open.win, open.passes), (8.0, 1.0));
}

#[test]
fn the_played_zone_is_the_window_else_the_placement_else_nothing() {
    assert_eq!(played_zone("midi", 16.0, Some(4.0), Some(8.0)), Some((4.0, 8.0)));
    assert_eq!(played_zone("midi", 4.0, None, None), Some((0.0, 4.0)));
    assert_eq!(played_zone("midi", 0.0, None, None), None);
}

fn spans_of(t: &[RVideoTrack], id: &str) -> Vec<(f64, Option<f64>)> {
    video_spans(t)[id].iter().map(|s| (s.from, s.to)).collect()
}

#[test]
fn a_cut_shot_a_gap_and_a_natural_length_shot_end_where_the_next_begins() {
    let t = lanes(json!([{ "id": "cam", "clips": [
        { "start": 0, "length": 2 }, { "start": 4, "length": 0 }, { "start": 8, "length": 1 }] }]));
    assert_eq!(
        spans_of(&t, "cam"),
        vec![(0.0, Some(2.0)), (4.0, Some(8.0)), (8.0, Some(9.0))]
    );
}

#[test]
fn shots_written_out_of_order_are_read_as_typed() {
    let t = lanes(json!([{ "id": "cam", "clips": [{ "start": 6, "length": 0 }, { "start": 1, "length": 2 }] }]));
    assert_eq!(spans_of(&t, "cam"), vec![(6.0, Some(1.0)), (1.0, Some(3.0))]);
}

#[test]
fn a_lane_of_shapes_follows_its_activation_spans_and_without_any_draws_while_unhidden() {
    let t = lanes(json!([
        { "id": "spans", "shapes": [{}], "clips": [{ "start": 2, "length": 2 }] },
        { "id": "titles", "shapes": [{}] },
    ]));
    assert_eq!(spans_of(&t, "spans"), vec![(2.0, Some(4.0))]);
    assert_eq!(spans_of(&t, "titles"), vec![(0.0, None)]);
}

fn lane_of(notes: Value) -> Vec<RTrack> {
    tracks(json!([{ "id": "m", "clips": [{ "type": "midi", "start": 0, "length": 4, "notes": notes }] }]))
}

#[test]
fn a_note_sounds_in_seconds_at_the_songs_tempo_from_its_clips_start() {
    let t = lane_of(json!([{ "time": 2, "duration": 2, "key": 60 }]));
    let s = &note_spans(&t, Some(120.0))["m"].spans[0];
    assert_eq!((s.from, s.to, s.vel), (1.0, 2.0, 1.0));
    assert_eq!(note_spans(&t, Some(60.0))["m"].spans[0].from, 2.0);
    let moved = tracks(json!([{ "id": "m", "clips": [{ "type": "midi", "start": 4, "length": 4,
        "notes": [{ "time": 0, "duration": 1, "key": 60 }] }] }]));
    assert_eq!(note_spans(&moved, Some(120.0))["m"].spans[0].from, 2.0);
}

#[test]
fn the_lanes_peak_is_the_busiest_instants_summed_velocity() {
    let t = lane_of(json!([
        { "time": 0, "duration": 4, "key": 60, "vel": 0.5 },
        { "time": 0, "duration": 4, "key": 60, "vel": 0.5 },
        { "time": 2, "duration": 1, "key": 60, "vel": 1 },
    ]));
    assert_eq!(note_spans(&t, Some(120.0))["m"].peak, 2.0);
}

#[test]
fn a_lane_that_sounds_nothing_is_left_out() {
    let t = tracks(json!([
        { "id": "muted", "mute": true, "clips": [{ "type": "midi", "notes": [{ "time": 0, "duration": 1, "key": 60 }] }] },
        { "id": "off", "clips": [{ "type": "midi", "disabled": true, "notes": [{ "time": 0, "duration": 1, "key": 60 }] }] },
        { "id": "audio", "clips": [{ "type": "audio", "notes": [] }] },
        { "id": "empty", "clips": [] },
    ]));
    assert!(note_spans(&t, Some(120.0)).is_empty());
}

fn placed(name: &str, start: f64) -> Value {
    json!({ "id": format!("{name}#id"), "type": "midi", "name": name, "start": start, "length": 4 })
}

#[test]
fn a_material_is_counted_across_lanes_and_its_first_is_the_earliest_in_time() {
    let t = tracks(json!([
        { "id": "bs", "clips": [placed("v", 24.0), placed("v", 8.0)] },
        { "id": "gtr", "clips": [placed("v", 24.0)] },
    ]));
    let got = material_instances(&t);
    let v = &got[0].1;
    assert_eq!((v.n, v.track.as_str(), v.start), (3, "bs", 8.0));
}

#[test]
fn a_material_placed_once_counts_one_and_a_generated_lane_counts_nothing() {
    let t = tracks(json!([
        { "id": "bs", "clips": [placed("hit", 0.0), placed("solo", 0.0)] },
        { "id": "algo", "generated": true, "clips": [placed("hit", 4.0), placed("hit", 8.0)] },
    ]));
    let got = material_instances(&t);
    let counts: Vec<(&str, u32)> = got.iter().map(|(k, i)| (k.as_str(), i.n)).collect();
    assert_eq!(counts, vec![("hit", 1), ("solo", 1)]);
}

#[test]
fn unnamed_placements_are_one_material_by_their_id() {
    let t = tracks(json!([{ "id": "bs", "clips": [
        { "id": "#id", "name": "", "start": 0 }, { "id": "#id", "name": "", "start": 8 }] }]));
    let got = material_instances(&t);
    assert_eq!((got[0].0.as_str(), got[0].1.n), ("#id", 2));
}

#[test]
fn a_midi_in_node_names_its_device_and_a_mic_is_no_midi_at_all() {
    let g = graph(json!({ "chains": {
        "keys": { "from": "keys", "use": [{ "module": "keys", "device": "KeyStep 37" }], "out": [{ "to": "lead" }] },
        "mic": { "from": "mic", "use": [{ "module": "mic", "monitor": "on" }], "out": [{ "to": "vox" }] },
    } }));
    assert_eq!(midi_in_device_for(&g, "lead").as_deref(), Some("KeyStep 37"));
    assert_eq!(midi_in_device_for(&g, "vox"), None);
    assert_eq!(midi_in_device_for(&g, "bass"), None);
    assert_eq!(midi_in_lanes(&g), vec!["lead"]);
}

#[test]
fn an_instrument_a_keyboard_feeds_with_no_lane_between_is_played_not_recorded() {
    let g = graph(json!({
        "tracks": ["lead"],
        "chains": {
            "keys": { "from": "keys", "use": [{ "module": "keys", "device": "KeyStep 37" }],
                "out": [{ "to": "piano#midi" }, { "to": "lead" }] },
            "piano": { "use": [{ "module": "piano" }], "out": [{ "to": "MIX" }] },
        },
    }));
    let played = midi_in_played(&g);
    assert_eq!(played.len(), 1);
    assert_eq!(
        (played[0].track.as_str(), played[0].device.as_str()),
        ("piano", "KeyStep 37")
    );
    let mic = graph(json!({ "chains": { "mic": { "from": "mic",
        "use": [{ "module": "mic", "monitor": "on" }], "out": [{ "to": "comp" }] } } }));
    assert!(midi_in_played(&mic).is_empty());
}
