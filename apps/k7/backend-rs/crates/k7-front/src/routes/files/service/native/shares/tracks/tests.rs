//! tests.rs — the tracks export's parts: the strip, the MIDI bytes, the pages and the archive, on one sample document.

use std::collections::HashMap;
use std::io::Read;

use midly::{MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};
use serde_json::{Value, json};

use super::{base_name, entries, midi, notes, pack, strip};

fn plugin(id: &str, kind: &str, params: Value) -> Value {
    json!({ "id": id, "type": kind, "name": kind, "path": "", "presetSave": "", "role": "audioFX",
            "enabled": true, "params": params })
}

fn audio(id: &str, start: f64) -> Value {
    json!({ "id": id, "type": "audio", "name": id, "source": "/a.wav", "start": start, "length": 4,
            "offset": 0, "fadeIn": 0.5, "fadeOut": 0.25, "gainDb": -3, "loopStart": 0, "loopEnd": 0, "notes": [] })
}

fn midi_clip(id: &str, start: f64, length: f64, loop_end: f64, notes: Value) -> Value {
    json!({ "id": id, "type": "midi", "name": id, "source": "", "start": start, "length": length,
            "offset": 0, "fadeIn": 0, "fadeOut": 0, "loopStart": 0, "loopEnd": loop_end, "notes": notes })
}

fn note(time: f64, key: u8, vel: f64, channel: u8) -> Value {
    json!({ "time": time, "duration": 1, "key": key, "vel": vel, "channel": channel, "rel": 0 })
}

fn track(id: &str, name: &str, content: &str) -> Value {
    json!({ "id": id, "name": name, "role": "regular", "contentType": content, "group": "", "generated": false,
            "color": "", "mute": false, "solo": false, "disabled": false, "inputChannels": [], "outputChannels": [],
            "monitor": "auto", "inputGain": 0, "clips": [], "plugins": [], "sends": [], "automation": [] })
}

/// Five tracks: audio with a strip, a notes-only one, a mixed one, a pattern lane and a bus.
pub fn sample() -> Value {
    let mut drums = track("drums", "Drums", "audio");
    drums["clips"] = json!([audio("d1", 3.0)]);
    drums["group"] = json!("g1");
    drums["mute"] = json!(true);
    drums["solo"] = json!(true);
    drums["plugins"] = json!([
        plugin(
            "drums:comp",
            "Compressor",
            json!({ "ratio": 4.0, "threshold": -18.0, "attack": 0.01 })
        ),
        plugin("drums:mix", "Mixer", json!({ "volume": 0.5, "pan": -0.25 })),
    ]);
    drums["sends"] = json!([{ "to": "fx", "level": 0.3, "enabled": true }]);
    drums["automation"] =
        json!([{ "param": "threshold", "pluginId": "drums:comp", "points": [[0, -30], [8, -6]] }]);
    let mut keys = track("keys", "Keys", "notes");
    keys["clips"] = json!([
        midi_clip(
            "k1",
            3.0,
            4.0,
            0.0,
            json!([note(0.0, 60, 1.0, 0), note(1.0, 64, 0.5, 2)])
        ),
        midi_clip("k2", 8.0, 4.0, 2.0, json!([note(0.0, 67, 0.8, 0)])),
        json!({ "id": "k3", "type": "midi", "start": 20.0, "length": 4, "loopStart": 0, "loopEnd": 0,
                "disabled": true, "notes": [note(0.0, 72, 1.0, 0)] }),
    ]);
    keys["plugins"] = json!([plugin("keys:piano", "Piano", json!({ "release": 0.2 }))]);
    let mut bass = track("bass", "Bass/Low", "audio notes");
    bass["clips"] = json!([
        audio("b1", 0.0),
        midi_clip("b2", 0.0, 4.0, 0.0, json!([note(0.0, 36, 1.0, 0)]))
    ]);
    let mut pattern = track("pat", "Pattern", "notes");
    pattern["generated"] = json!(true);
    pattern["clips"] = json!([midi_clip(
        "p1",
        0.0,
        1.0,
        0.0,
        json!([note(0.0, 40, 1.0, 0)])
    )]);
    let mut fx = track("fx", "FX", "audio");
    fx["role"] = json!("effect");
    fx["plugins"] = json!([plugin("fx:verb", "Reverb", json!({ "mix": 1.0 }))]);
    json!({
        "name": "Song", "tempo": 90, "sampleRate": 48000, "timeSigNumerator": 3, "timeSigDenominator": 4,
        "masterVolume": -3, "masterPlugins": [plugin("master:lim", "Limiter", json!({ "ceiling": -1.0 }))],
        "recordDir": "/rec", "tracks": [drums, keys, bass, pattern, fx],
        "groups": [{ "id": "g1", "name": "Rhythm", "color": "", "mute": true, "solo": true }],
        "markers": [], "settings": {}
    })
}

#[test]
fn the_strip_empties_every_effect_and_leaves_the_clips_as_placed() {
    let doc = sample();
    let s = strip::strip(&doc);
    assert_eq!(s.kept, vec![0, 2]);
    let tracks = s.doc["tracks"].as_array().unwrap();
    assert_eq!(tracks.len(), 2);
    for (t, from) in tracks.iter().zip([0usize, 2]) {
        assert_eq!(t["plugins"], json!([]), "plugins");
        assert_eq!(t["sends"], json!([]), "sends");
        assert_eq!(t["automation"], json!([]), "automation");
        assert_eq!(
            (t["mute"].clone(), t["solo"].clone()),
            (json!(false), json!(false))
        );
        assert_eq!(t["name"], json!(strip::stem_name(from)));
    }
    assert_eq!(s.doc["masterPlugins"], json!([]));
    assert_eq!(s.doc["masterVolume"], json!(0));
    assert_eq!(s.doc["recordDir"], json!(""));
    assert_eq!(s.doc["groups"][0]["mute"], json!(false));
    assert_eq!(tracks[0]["clips"], doc["tracks"][0]["clips"]);
    assert_eq!(
        tracks[1]["clips"],
        json!([doc["tracks"][2]["clips"][0].clone()])
    );
}

fn parsed(bytes: &[u8]) -> Vec<(u32, bool, u8, u8, u8)> {
    let smf = Smf::parse(bytes).unwrap();
    assert_eq!(smf.header.timing, Timing::Metrical(midi::PPQ.into()));
    let mut at = 0;
    let mut out = vec![];
    for e in &smf.tracks[0] {
        at += e.delta.as_int();
        if let TrackEventKind::Midi { channel, message } = e.kind {
            match message {
                MidiMessage::NoteOn { key, vel } => {
                    out.push((at, true, key.as_int(), vel.as_int(), channel.as_int()))
                }
                MidiMessage::NoteOff { key, .. } => {
                    out.push((at, false, key.as_int(), 0, channel.as_int()))
                }
                _ => {}
            }
        }
    }
    out
}

#[test]
fn the_midi_file_holds_the_projects_tempo_and_time_signature() {
    let doc = sample();
    let n = notes::notes_of(&doc["tracks"][1]);
    let bytes = midi::midi_bytes("Keys", &doc, &n).unwrap();
    let smf = Smf::parse(&bytes).unwrap();
    let metas: Vec<_> = smf.tracks[0]
        .iter()
        .filter_map(|e| match e.kind {
            TrackEventKind::Meta(m) => Some(m),
            _ => None,
        })
        .collect();
    assert!(
        metas.contains(&MetaMessage::Tempo(666_667.into())),
        "{metas:?}"
    );
    assert!(
        metas.contains(&MetaMessage::TimeSignature(3, 2, 24, 8)),
        "{metas:?}"
    );
}

#[test]
fn the_notes_land_in_ticks_with_the_loop_honoured_and_a_disabled_clip_absent() {
    let doc = sample();
    let bytes = midi::midi_bytes("Keys", &doc, &notes::notes_of(&doc["tracks"][1])).unwrap();
    let ons: Vec<_> = parsed(&bytes).into_iter().filter(|e| e.1).collect();
    // A clip at beat 3 (bar 2 of 3/4): 1440 ticks; its second note a beat later, velocity 0.5, channel 2.
    // The loop of two beats over four: passes at beats 8 and 10; the disabled clip at beat 20 is not there.
    assert_eq!(
        ons,
        vec![
            (1440, true, 60, 127, 0),
            (1920, true, 64, 64, 2),
            (3840, true, 67, 102, 0),
            (4800, true, 67, 102, 0),
        ]
    );
}

#[test]
fn a_page_lists_every_param_of_the_chain_in_order_and_marks_the_automated_one() {
    let doc = sample();
    let page = super::describe::track_page(
        &doc,
        0,
        &super::describe::Files {
            wav: Some("01-Drums.wav".into()),
            mid: None,
        },
        &json!({}),
    );
    let at = |s: &str| {
        page.find(s)
            .unwrap_or_else(|| panic!("{s} missing in\n{page}"))
    };
    assert!(at("### 1. Compressor") < at("### 2. Mixer"));
    // Sorted by name inside a node.
    assert!(at("`attack`: 0.01") < at("`ratio`: 4") && at("`ratio`: 4") < at("`threshold`: -18"));
    assert!(at("`pan`: -0.25") < at("`volume`: 0.5"));
    assert!(
        page.contains("`threshold`: -18 (automated: 2 points, from -30 to -6, over beats 0 to 8)"),
        "{page}"
    );
    assert_eq!(page.matches("automated").count(), 1, "{page}");
    assert!(page.contains("- to FX: level 0.3"), "{page}");
    assert!(
        page.contains("- group: Rhythm") && page.contains("`01-Drums.wav`"),
        "{page}"
    );
}

#[test]
fn a_pattern_lane_says_it_has_no_fixed_audio_or_midi() {
    let doc = sample();
    let page = super::describe::track_page(
        &doc,
        3,
        &super::describe::Files {
            wav: None,
            mid: None,
        },
        &json!({}),
    );
    assert!(
        page.contains("generated by a pattern and has no fixed audio or MIDI"),
        "{page}"
    );
}

#[test]
fn the_archive_lists_exactly_the_expected_entries_with_the_audio_stored() {
    let dir = super::super::fakes::scratch("zip");
    let mut wavs = HashMap::new();
    for i in [0usize, 2] {
        let p = dir.join(format!("w{i}.wav"));
        std::fs::write(&p, vec![7u8; 4096]).unwrap();
        wavs.insert(i, p);
    }
    let out = dir.join("out.zip");
    pack::write_zip(&out, entries(&sample(), &mut wavs, &json!({})).unwrap()).unwrap();
    let mut z = zip::ZipArchive::new(std::fs::File::open(&out).unwrap()).unwrap();
    let names: Vec<String> = z.file_names().map(str::to_owned).collect();
    let expected = [
        "README.md",
        "01-Drums.md",
        "01-Drums.wav",
        "02-Keys.md",
        "02-Keys.mid",
        "03-Bass_Low.md",
        "03-Bass_Low.wav",
        "03-Bass_Low.mid",
        "04-Pattern.md",
        "05-FX.md",
    ];
    assert_eq!(names, expected);
    for n in &names {
        let f = z.by_name(n).unwrap();
        let want = if n.ends_with(".wav") {
            zip::CompressionMethod::Stored
        } else {
            zip::CompressionMethod::Deflated
        };
        assert_eq!(f.compression(), want, "{n}");
    }
    let mut text = String::new();
    z.by_name("README.md")
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();
    assert!(
        text.contains("tempo: 90 bpm") && text.contains("`ceiling`: -1"),
        "{text}"
    );
    assert!(
        !dir.join("w0.wav").exists(),
        "a packed wav is moved, not copied"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_file_name_is_numbered_and_never_holds_a_separator() {
    assert_eq!(base_name(0, 3, "A/B:C"), "01-A_B_C");
    assert_eq!(base_name(99, 120, " .. "), "100-track");
    assert!(base_name(0, 1, &"é".repeat(300)).len() < 200);
}
