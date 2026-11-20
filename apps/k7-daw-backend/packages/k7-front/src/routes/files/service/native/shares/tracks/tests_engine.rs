//! tests_engine.rs — the tracks export where it must agree with the engine: the beat, frozen lanes, loops and disabled clips.

use serde_json::{Value, json};

use super::{entries, midi, notes, strip};

fn midi_clip(start: f64, length: f64, loop_end: f64, notes: Value) -> Value {
    json!({ "id": "m", "type": "midi", "start": start, "length": length, "loopStart": 0,
            "loopEnd": loop_end, "notes": notes })
}

fn note(time: f64) -> Value {
    json!({ "time": time, "duration": 0.25, "key": 60, "vel": 1.0, "channel": 0 })
}

fn starts(track: &Value) -> Vec<f64> {
    notes::notes_of(track).iter().map(|n| n.start).collect()
}

fn on_ticks(doc: &Value, track: &Value) -> Vec<u32> {
    use midly::{MidiMessage, Smf, TrackEventKind};
    let bytes = midi::midi_bytes("t", doc, &notes::notes_of(track)).unwrap();
    let smf = Smf::parse(&bytes).unwrap();
    let mut at = 0;
    let mut out = vec![];
    for e in &smf.tracks[0] {
        at += e.delta.as_int();
        if let TrackEventKind::Midi {
            message: MidiMessage::NoteOn { .. },
            ..
        } = e.kind
        {
            out.push(at);
        }
    }
    out
}

#[test]
fn a_beat_is_the_denominator_note_so_seven_eight_lands_on_its_bar_grid() {
    let doc = json!({ "tempo": 120, "timeSigNumerator": 7, "timeSigDenominator": 8 });
    let t = json!({ "clips": [midi_clip(0.0, 8.0, 0.0, json!([note(7.0)]))] });
    // Beat 7 is the start of bar 2: 7 eighths, 3.5 quarters of 480 ticks, minus none.
    assert_eq!(on_ticks(&doc, &t), vec![1680]);
    let four = json!({ "tempo": 120, "timeSigNumerator": 4, "timeSigDenominator": 4 });
    assert_eq!(on_ticks(&four, &t), vec![3360]);
}

#[test]
fn a_frozen_lane_exports_its_authored_clips_not_the_render() {
    let audio = |id: &str| json!({ "id": id, "type": "audio", "source": "/a.wav", "start": 0, "length": 4 });
    let t = json!({ "generated": false,
        "clips": [audio("render")],
        "frozenMaterial": [audio("authored"), midi_clip(0.0, 4.0, 0.0, json!([note(1.0)]))] });
    let doc = json!({ "tracks": [t] });
    let s = strip::strip(&doc);
    assert_eq!(s.doc["tracks"][0]["clips"], json!([audio("authored")]));
    assert!(s.doc["tracks"][0].get("frozenMaterial").is_none());
    assert_eq!(starts(&t), vec![1.0]);
}

#[test]
fn a_loop_without_skip_fills_the_clip_and_a_clip_without_length_stops_at_four_beats() {
    let looped = json!({ "clips": [midi_clip(0.0, 5.0, 4.0, json!([note(0.5)]))] });
    assert_eq!(starts(&looped), vec![0.5, 4.5]);
    let skipped = {
        let mut n = note(0.5);
        n["skip"] = json!(2);
        json!({ "clips": [midi_clip(0.0, 5.0, 4.0, json!([n]))] })
    };
    assert_eq!(
        starts(&skipped),
        vec![0.5],
        "with a skip the engine rounds the passes"
    );
    let open = json!({ "clips": [midi_clip(0.0, 0.0, 0.0, json!([note(1.0), note(6.0)]))] });
    assert_eq!(starts(&open), vec![1.0]);
}

#[test]
fn a_track_of_only_disabled_audio_is_not_rendered_and_still_has_its_files() {
    let off =
        json!({ "type": "audio", "source": "/a.wav", "start": 0, "length": 4, "disabled": true });
    let t = json!({ "name": "Off", "clips": [off, midi_clip(0.0, 4.0, 0.0, json!([note(0.0)]))] });
    let doc = json!({ "tracks": [t] });
    assert!(strip::strip(&doc).kept.is_empty());
    let all = entries(&doc, &mut Default::default(), &json!({})).unwrap();
    let names: Vec<_> = all.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["README.md", "01-Off.md", "01-Off.mid"]);
}

#[test]
fn a_page_says_which_module_a_node_extends_and_where_each_param_comes_from() {
    use super::describe::{Files, track_page};
    let amp = json!({ "id": "gtr:lead", "name": "lead", "type": "x", "role": "", "enabled": true,
        "params": { "drive": 9.0, "tone": 0.7, "gain": 1.0, "mix": 1.0 } });
    let doc = json!({ "tracks": [{ "name": "Gtr", "plugins": [amp], "frozenMaterial": [{}] }] });
    let origins = json!({ "lead": { "extends": ["@w/amp", "@w/base"], "own": ["drive"],
        "from": { "tone": "@w/amp", "gain": "@w/base" } } });
    let files = Files {
        wav: None,
        mid: None,
    };
    let page = track_page(&doc, 0, &files, &origins);
    for want in [
        "- extends: `@w/amp` > `@w/base`",
        "`drive`: 9 (set by this node)",
        "`tone`: 0.7 (inherited from `@w/amp`)",
        "`gain`: 1 (inherited from `@w/base`)",
        "`mix`: 1 (module default)",
        "- frozen:",
    ] {
        assert!(page.contains(want), "{want} missing in\n{page}");
    }
    let bare = track_page(&doc, 0, &files, &json!({}));
    assert!(
        !bare.contains("extends") && !bare.contains("inherited"),
        "{bare}"
    );
}
