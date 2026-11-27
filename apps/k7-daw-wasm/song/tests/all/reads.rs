// reads.rs — what the page reads off the open project: loops, takes, spans, rows, the graph's inputs, and the small rules, as the old TypeScript answered them.
// A reader would get wrong that the cases are the old suites' inputs: the same numbers went through the TypeScript before the twin was deleted.

use serde_json::{Value, json};
use song::reads::*;

fn tracks(v: Value) -> Vec<RTrack> {
    serde_json::from_value(v).expect("tracks")
}

fn clips(v: Value) -> Vec<RClip> {
    serde_json::from_value(v).expect("clips")
}

fn graph(v: Value) -> RGraph {
    serde_json::from_value(v).expect("graph")
}

#[test]
fn a_midi_loop_is_in_the_notes_beats_and_an_audio_one_in_the_sources() {
    let midi = clip_loop("midi", 16.0, Some(2.0), Some(6.0)).expect("a loop");
    assert_eq!((midi.from, midi.to, midi.win, midi.passes), (2.0, 6.0, 4.0, 4.0));
    assert_eq!(midi.repeats, vec![6.0, 10.0, 14.0]);
    let audio = clip_loop("audio", 16.0, Some(2.0), Some(6.0)).expect("a loop");
    assert_eq!((audio.from, audio.to), (0.0, 4.0));
    assert!(clip_loop("midi", 16.0, None, None).is_none());
    assert!(clip_loop("midi", 16.0, Some(4.0), Some(4.0)).is_none());
}

#[test]
fn a_cut_off_pass_is_a_pass_and_the_marks_are_capped() {
    assert_eq!(
        clip_loop("midi", 5.0, Some(0.0), Some(4.0)).map(|l| l.passes),
        Some(1.0)
    );
    assert_eq!(
        clip_loop("midi", 6.0, Some(0.0), Some(4.0)).map(|l| l.passes),
        Some(2.0)
    );
    let long = clip_loop("midi", 100000.0, Some(0.0), Some(1.0)).expect("a loop");
    assert_eq!(long.repeats.len(), 256);
    assert_eq!(played_zone("midi", 8.0, None, None), Some((0.0, 8.0)));
    assert_eq!(played_zone("midi", 0.0, None, None), None);
}

#[test]
fn the_first_placement_is_the_earliest_and_a_tie_keeps_the_higher_lane() {
    let t = tracks(json!([
        { "id": "a", "clips": [{ "id": "x1", "name": "x", "start": 8 }] },
        { "id": "b", "clips": [{ "id": "x2", "name": "x", "start": 4 }, { "id": "y", "name": "", "start": 0 }] },
        { "id": "c", "generated": true, "clips": [{ "id": "x3", "name": "x", "start": 0 }] },
        { "id": "d", "clips": [{ "id": "x4", "name": "x", "start": 4 }] },
    ]));
    let got = material_instances(&t);
    assert_eq!(got.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(), vec!["x", "y"]);
    let x = &got[0].1;
    assert_eq!((x.n, x.track.as_str(), x.start), (3, "b", 4.0));
}

#[test]
fn a_shared_bus_strip_is_one_card() {
    let t = tracks(json!([
        { "id": "a", "mixer": { "module": "bus" } },
        { "id": "b" },
        { "id": "c", "mixer": { "module": "bus" } },
        { "id": "d", "mixer": { "module": "" } },
        { "id": "e", "mixer": { "module": "own" } },
    ]));
    assert_eq!(mixer_modules(&t), vec!["bus", "own"]);
}

#[test]
fn note_spans_leave_out_muted_lanes_and_give_a_zero_length_note_an_instant() {
    let t = tracks(json!([
        { "id": "on", "clips": [{ "type": "midi", "start": 4, "notes": [{ "time": 0, "duration": 0, "key": 60 }, { "time": 1, "duration": 2, "key": 62, "vel": 0.5 }] }] },
        { "id": "muted", "mute": true, "clips": [{ "type": "midi", "notes": [{ "time": 0, "duration": 1, "key": 60 }] }] },
        { "id": "audio", "clips": [{ "type": "audio", "notes": [] }] },
    ]));
    let got = note_spans(&t, Some(120.0));
    assert_eq!(got.keys().map(String::as_str).collect::<Vec<_>>(), vec!["on"]);
    let lane = &got["on"];
    assert_eq!(
        (lane.spans[0].from, lane.spans[0].to, lane.spans[0].vel),
        (2.0, 2.0625, 1.0)
    );
    assert_eq!(lane.spans[1].vel, 0.5);
    assert_eq!(lane.peak, 1.0);
    assert_eq!(note_spans(&t, Some(0.0))["on"].spans[0].from, 2.0);
}

#[test]
fn a_picture_lane_shows_by_three_rules() {
    let t: Vec<RVideoTrack> = serde_json::from_value(json!([
        { "id": "hidden", "mute": true, "clips": [{ "start": 0, "length": 2 }] },
        { "id": "shape", "clips": [], "shapes": [{}] },
        { "id": "empty", "clips": [] },
        { "id": "shots", "clips": [{ "start": 0, "length": 2 }, { "start": 5, "length": 0 }, { "start": 9, "length": 0 }] },
    ]))
    .expect("lanes");
    let got = video_spans(&t);
    assert_eq!(
        got.keys().map(String::as_str).collect::<Vec<_>>(),
        vec!["shape", "shots"]
    );
    assert_eq!(got["shape"][0].to, None);
    let ends: Vec<Option<f64>> = got["shots"].iter().map(|s| s.to).collect();
    assert_eq!(ends, vec![Some(2.0), Some(9.0), None]);
}

#[test]
fn activity_is_half_at_least_and_one_at_the_loudest() {
    assert_eq!(to_alphas(&[0.0, 0.0]), vec![0.5, 0.5]);
    assert_eq!(to_alphas(&[1.0, 2.0, 0.0]), vec![0.75, 1.0, 0.5]);
    assert_eq!(
        audio_activity(&[0.1, 0.9, 0.2, 0.3], 2),
        vec![1.0, 0.5 + 0.5 * (0.3 / 0.9)]
    );
    assert!(audio_activity(&[], 4).is_empty());
    let notes: Vec<RNote> = serde_json::from_value(
        json!([{ "time": 0, "duration": 1, "key": 60 }, { "time": 1, "duration": 1, "key": 62, "vel": 0.5 }]),
    )
    .expect("notes");
    assert_eq!(midi_activity(&notes, 4.0, 4, 0.0, 0.0), vec![1.0, 0.75, 0.5, 0.5]);
    // A loop of the first beat tiles its window across the span.
    assert_eq!(midi_activity(&notes, 4.0, 4, 0.0, 1.0), vec![1.0, 1.0, 1.0, 1.0]);
    assert_eq!(activity_mask(&[]), "");
    assert_eq!(
        activity_mask(&[0.5]),
        "linear-gradient(rgb(0 0 0 / 0.5), rgb(0 0 0 / 0.5))"
    );
    assert_eq!(
        activity_mask(&[0.5, 1.0]),
        "linear-gradient(to right, rgb(0 0 0 / 0.5) 25%, rgb(0 0 0 / 1) 75%)"
    );
}

fn comped() -> Vec<RClip> {
    clips(json!([
        { "id": "riff@4", "type": "midi", "name": "riff", "start": 4, "length": 2, "comp": { "of": "riff@4", "take": "riff" },
          "event": { "takes": [{ "ref": "t2", "start": "2:3" }], "comp": [{ "take": "t2", "from": "2:3", "to": "3:1" }], "crossfades": [{ "at": "2:3", "length": "0:0:1" }] } },
        { "id": "t2@6", "type": "midi", "name": "t2", "start": 6, "length": 2, "comp": { "of": "riff@4", "take": "t2" } },
        { "id": "other@16", "type": "midi", "name": "other", "start": 16, "length": 4 },
    ]))
}

#[test]
fn an_event_is_found_from_any_of_its_pieces() {
    let ev = takes_event_of("keys", "riff@4", &comped(), 4.0, 4.0)
        .expect("reads")
        .expect("an event");
    assert_eq!((ev.ref_.as_str(), ev.at.as_str(), ev.start), ("riff", "2:1", 4.0));
    assert_eq!(ev.rows.len(), 2);
    assert!(ev.rows[0].own && !ev.rows[1].own);
    assert_eq!((ev.rows[1].ref_.as_str(), ev.rows[1].start), ("t2", 6.0));
    assert_eq!(ev.pieces, vec![0, 1]);
    assert_eq!((ev.from, ev.to), (4.0, 8.0));
    assert_eq!(ev.crossfades.len(), 1);
    assert!(
        takes_event_of("keys", "nothing@9", &comped(), 4.0, 4.0)
            .expect("reads")
            .is_none()
    );
    assert!(
        takes_event_of("keys", "riff@4", &comped(), 4.0, 4.0)
            .expect("reads")
            .is_some()
    );
    assert!(
        takes_event_of(
            "keys",
            "riff@4",
            &clips(json!([{ "id": "riff@4", "event": { "takes": [{ "ref": "t", "start": "x" }] } }])),
            4.0,
            4.0
        )
        .is_err()
    );
}

#[test]
fn the_comp_is_heard_where_a_span_says_and_the_own_row_where_none_does() {
    let comp = vec![("t2".to_owned(), 6.0, 7.0)];
    assert_eq!(heard_spans(&comp, false, "t2", 6.0, 8.0), vec![(6.0, 7.0)]);
    assert_eq!(heard_spans(&comp, true, "riff", 4.0, 8.0), vec![(4.0, 6.0), (7.0, 8.0)]);
    assert_eq!(comp_seams(4.0, 8.0, &[(6.0, 7.0), (4.0, 6.0)]), vec![6.0, 7.0]);
}

#[test]
fn a_pick_and_an_arm_are_one_line_each() {
    assert_eq!(
        comp_line("keys", "2:1", "t2", 6.0, 7.0, 4.0, 4.0).as_deref(),
        Ok("event comp keys 2:1 t2 2:3 2:4")
    );
    assert_eq!(
        arm_line("keys", "2:1", 6.0, None, 4.0, 4.0).as_deref(),
        Ok("event take arm keys 2:1 2:3")
    );
    assert_eq!(
        arm_line("keys", "2:1", 6.0, Some(7.0), 4.0, 4.0).as_deref(),
        Ok("event take arm keys 2:1 2:3 2:4")
    );
    assert_eq!(
        comp_fade_line("keys", "2:1", 6.0, 0.0, 4.0, 4.0).as_deref(),
        Ok("event comp fade keys 2:1 2:3 0")
    );
}

#[test]
fn a_grown_piece_takes_the_span_and_a_shrunk_one_gives_it_back() {
    let pieces = comped();
    let grown = piece_edge_lines("keys", "2:1", "riff", &pieces[..2], "t2@6", 5.0, 8.0, 4.0, 4.0).expect("lines");
    assert_eq!(grown, vec!["event comp keys 2:1 t2 2:2 3:1"]);
    let shrunk = piece_edge_lines("keys", "2:1", "riff", &pieces[..2], "t2@6", 6.0, 7.0, 4.0, 4.0).expect("lines");
    assert_eq!(shrunk, vec!["event comp keys 2:1 riff 2:4 3:1"]);
    assert!(
        piece_edge_lines("keys", "2:1", "riff", &pieces[..2], "gone", 6.0, 7.0, 4.0, 4.0)
            .expect("lines")
            .is_empty()
    );
}

#[test]
fn a_group_is_a_header_over_its_members_at_the_first_and_a_picture_goes_after_its_anchor() {
    let t = tracks(
        json!([{ "id": "a", "group": "" }, { "id": "k", "group": "drums" }, { "id": "b#midi", "group": "" }, { "id": "s", "group": "drums" }]),
    );
    let rows = project_rows(&["drums".to_owned()], &t, &[], &Default::default(), None);
    let shape: Vec<String> = rows
        .iter()
        .map(|r| match r {
            LayoutRow::Group { group } => format!("g{group}"),
            LayoutRow::Track { track, grouped, .. } => format!("t{track}{}", if *grouped { "+" } else { "" }),
            LayoutRow::Video { track } => format!("v{track}"),
            LayoutRow::Automation { track, lane } => format!("a{track}.{lane}"),
        })
        .collect();
    assert_eq!(shape, vec!["t0", "g0", "t1+", "t3+", "t2"]);
    let order: Vec<String> = ["v0", "a", "b", "drums", "v1"].map(String::from).to_vec();
    let with = project_rows(
        &["drums".to_owned()],
        &t,
        &["v0".to_owned(), "v1".to_owned()],
        &Default::default(),
        Some(&order),
    );
    // `v0` leads the order, so it sits on top; `v1` follows the group's last member, and `b` comes after it.
    assert!(matches!(with[0], LayoutRow::Video { track: 0 }));
    let n = with.len();
    assert!(matches!(with[n - 2], LayoutRow::Video { track: 1 }));
    assert!(matches!(with[n - 1], LayoutRow::Track { track: 2, .. }));
}

#[test]
fn a_clip_is_placed_by_the_first_lane_that_holds_it_and_a_frozen_lane_still_holds_its_material() {
    let t = tracks(
        json!([{ "id": "a", "clips": [{ "id": "c1" }] }, { "id": "b", "clips": [{ "id": "c1" }], "frozenMaterial": [{ "id": "m" }] }]),
    );
    assert_eq!(track_placing(&t, "c1"), "a");
    assert_eq!(track_placing(&t, "m"), "b");
    assert_eq!(track_placing(&t, ""), "");
    let lane = clips(json!([{ "id": "x@0", "name": "x" }, { "id": "y@4", "name": "y", "pieces": [{ "name": "z" }] }]));
    assert_eq!(placement_of(&lane, "z"), Some(1));
    assert_eq!(placement_of(&lane, "x"), Some(0));
    assert_eq!(placement_of(&lane, "q"), None);
}

fn rig() -> RGraph {
    graph(json!({
        "tracks": ["gtr", "keys"],
        "chains": {
            "src": { "use": [{ "module": "mic", "monitor": "on", "channel": [1] }], "out": [{ "to": "amp" }] },
            "pre": { "use": [{ "module": "amp" }], "out": [{ "to": "gtr" }] },
            "kb": { "use": [{ "module": "kbd", "device": "" }], "out": [{ "to": "keys" }, { "to": "synth#midi" }] },
            "synth": { "use": [{ "module": "synth" }], "out": [{ "to": "MIX" }] },
            "run": { "from": "keys", "use": [{ "module": "kit", "rack": [{ "symbol": "BD", "note": 36, "sample": "@lib/kits/kick.wav" }, { "symbol": "SN", "note": 36, "clip": "snare" }] }] },
        },
    }))
}

#[test]
fn a_midi_in_node_is_a_device_even_when_its_label_is_empty() {
    let g = rig();
    assert_eq!(midi_in_device_for(&g, "keys").as_deref(), Some(""));
    assert_eq!(midi_in_device_for(&g, "gtr"), None);
    assert_eq!(midi_in_lanes(&g), vec!["keys", "synth#midi"]);
    let played = midi_in_played(&g);
    assert_eq!(played.len(), 1);
    assert_eq!(played[0].track, "synth");
}

#[test]
fn an_input_behind_a_preamp_still_feeds_the_lane() {
    let g = rig();
    let found = lane_input(&g, "gtr").expect("an input");
    assert_eq!(
        (found.module.as_str(), found.kind.as_str(), found.monitor.as_deref()),
        ("mic", "audio", Some("on"))
    );
    assert_eq!(lane_input(&g, "keys").map(|i| i.kind), Some("midi".to_owned()));
    assert!(lane_input(&g, "nobody").is_none());
    assert!(has_audible_input(&g));
    assert!(gives_channel("node param mic @channel 1"));
    assert!(!gives_channel("node param mic @channel "));
}

#[test]
fn the_first_rack_downstream_names_its_keys_by_sample_clip_or_symbol() {
    let g = rig();
    let rack = rack_for(&g, "keys").expect("a rack");
    assert_eq!(rack.len(), 2);
    assert_eq!(rack_key_names(&rack), vec![(36.0, "kick / snare".to_owned())]);
    assert!(rack_for(&g, "").is_none());
}

#[test]
fn notes_leave_the_graph_by_node_and_by_port() {
    let g = json!({ "chains": { "a": { "use": [{ "module": "doc1" }, { "module": "amp" }], "out": [{ "to": "doc1#in" }, { "to": "b" }] } } });
    let notes = std::iter::once("doc1".to_owned()).collect();
    let got = without_notes(g, &notes);
    assert_eq!(got["chains"]["a"]["use"], json!([{ "module": "amp" }]));
    assert_eq!(got["chains"]["a"]["out"], json!([{ "to": "b" }]));
}

#[test]
fn a_looped_clip_tiles_its_window_into_the_racks_hits() {
    let t = tracks(
        json!([{ "id": "keys", "clips": [{ "type": "midi", "start": 8, "length": 4, "loopStart": 0, "loopEnd": 2,
        "notes": [{ "time": 0, "duration": 0.1, "key": 36 }, { "time": 3, "duration": 1, "key": 40 }] }] }]),
    );
    let hits = rack_hits(&t, &rig());
    let kit = &hits["kit"];
    assert_eq!(
        kit.iter().map(|h| (h.from, h.key)).collect::<Vec<_>>(),
        vec![(8.0, 36.0), (10.0, 36.0)]
    );
    assert_eq!(kit[0].to, 8.25);
}

#[test]
fn a_clip_ref_reads_as_a_take_a_sequence_or_a_name() {
    use ClipNameParts::*;
    assert_eq!(
        clip_name_parts("gtr_9.1_261030T172747"),
        Take {
            lane: "gtr".into(),
            bar: 9.0,
            copy: None
        }
    );
    assert_eq!(
        clip_name_parts("gtr_9.1_261030T172747-2@4"),
        Take {
            lane: "gtr".into(),
            bar: 9.0,
            copy: Some(2.0)
        }
    );
    assert_eq!(
        clip_name_parts("keys-seq-3"),
        Sequence {
            lane: "keys".into(),
            n: 3.0
        }
    );
    assert_eq!(
        clip_name_parts("a-seq-1-seq-2"),
        Sequence {
            lane: "a-seq-1".into(),
            n: 2.0
        }
    );
    assert_eq!(clip_name_parts("riff"), Named { name: "riff".into() });
    assert_eq!(clip_name_parts("-seq-3"), Named { name: "-seq-3".into() });
}

#[test]
fn the_fix_trim_is_never_positive_and_the_master_stays_in_its_field() {
    assert_eq!(trim_for(0.0), -6.0);
    assert_eq!(trim_for(3.0), -9.0);
    assert_eq!(trim_for(-20.0), -6.0);
    assert_eq!(trim_for(f64::NEG_INFINITY), -6.0);
    assert_eq!(fixed_master(-2.0, 3.0, -100.0, 12.0), -11.0);
    assert_eq!(fixed_master(-99.0, 6.0, -100.0, 12.0), -100.0);
}

#[test]
fn a_tap_run_restarts_after_a_pause_and_averages_the_last_eight() {
    let first = tap_tempo(&[], 1000.0);
    assert_eq!((first.taps.clone(), first.bpm), (vec![1000.0], None));
    let second = tap_tempo(&first.taps, 1500.0);
    assert_eq!(second.bpm, Some(120.0));
    assert_eq!(tap_tempo(&second.taps, 1500.0 + 2001.0).taps.len(), 1);
    let mut run: Vec<f64> = (0..10).map(|i| i as f64 * 500.0).collect();
    run = tap_tempo(&run, 5000.0).taps;
    assert_eq!(run.len(), 8);
}

#[test]
fn a_folder_name_refuses_separators_dots_and_nothing_and_strips_a_typed_suffix() {
    let sfx = vec![".k7".to_owned()];
    assert_eq!(folder_name("  ", &sfx).error, "required");
    assert_eq!(folder_name("a/b", &sfx).error, "noSlash");
    assert_eq!(folder_name(".hid", &sfx).error, "noDot");
    assert_eq!(folder_name(".k7", &sfx).error, "noDot");
    assert_eq!(folder_name("Song.K7", &sfx).folder, "Song");
    assert_eq!(folder_name("x.k7.k7", &sfx).folder, "x.k7");
}

#[test]
fn a_renamed_timeline_is_followed_only_when_one_vanished_and_one_appeared() {
    let ids = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    assert_eq!(
        follow_rename(Some("a"), &ids(&["a", "b"]), &ids(&["a", "b"])).as_deref(),
        Some("a")
    );
    assert_eq!(
        follow_rename(Some("a"), &ids(&["a", "b"]), &ids(&["c", "b"])).as_deref(),
        Some("c")
    );
    assert_eq!(follow_rename(Some("a"), &ids(&["a", "b"]), &ids(&["b"])), None);
    assert_eq!(follow_rename(None, &ids(&["a"]), &ids(&["b"])), None);
}

#[test]
fn surfaces_read_as_an_archetype() {
    let s = |v: &[&str]| Some(v.iter().map(|x| (*x).to_owned()).collect::<Vec<_>>());
    assert_eq!(
        shape_of_surfaces(s(&["turntables", "timeline"]).as_deref()),
        Some("mix")
    );
    assert_eq!(shape_of_surfaces(s(&["timeline"]).as_deref()), Some("arrangement"));
    assert_eq!(
        shape_of_surfaces(s(&["timeline", "viewport"]).as_deref()),
        Some("video")
    );
    assert_eq!(shape_of_surfaces(s(&["viewport"]).as_deref()), Some("live-video"));
    assert_eq!(shape_of_surfaces(s(&[]).as_deref()), Some("live"));
    assert_eq!(shape_of_surfaces(None), None);
}
