// doc_twin.rs — the Rust ProjectDoc types hold exactly the keys of engine-protocol/project.ts, both ways (decision 301).
// A reader would get wrong that a maximal value is built from the Rust side: it is the TypeScript's keys that are parsed.

use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use song::clips::{ClipDoc, TimelineDoc};
use song::doc::{AutomationDoc, GroupDoc, PluginDoc, ProjectDoc, RackSoundDoc, SendDoc, TrackDoc};
use song::files::{Marker, SettingsDoc};
use std::collections::{BTreeMap, BTreeSet};

/// An interface's top-level keys, and the inline object keys of the fields that carry one.
struct Face {
    keys: BTreeSet<String>,
    inline: BTreeMap<String, BTreeSet<String>>,
}

fn code_of(line: &str) -> &str {
    line.find(" //").map_or(line, |i| &line[..i])
}

fn name_of(field: &str) -> Option<String> {
    let end = field.find(['?', ':'])?;
    let name = field[..end].trim();
    let ok = !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    ok.then(|| name.to_owned())
}

fn inline_keys(ty: &str) -> Option<BTreeSet<String>> {
    let open = ty.find('{')?;
    let close = ty[open..].find('}')? + open;
    Some(
        ty[open + 1..close]
            .split(';')
            .filter_map(|p| name_of(p.trim()))
            .collect(),
    )
}

fn interfaces(text: &str) -> BTreeMap<String, Face> {
    let mut out = BTreeMap::new();
    let mut current: Option<(String, Face)> = None;
    for raw in text.lines() {
        let line = raw.trim();
        if let Some(rest) = line.strip_prefix("export interface ") {
            let name = rest.split([' ', '<']).next().unwrap_or("").to_owned();
            current = Some((
                name,
                Face {
                    keys: BTreeSet::new(),
                    inline: BTreeMap::new(),
                },
            ));
            continue;
        }
        let Some((name, face)) = current.as_mut() else { continue };
        if line == "}" {
            out.insert(name.clone(), current.take().map(|c| c.1).expect("a face"));
            continue;
        }
        if line.starts_with("//") || line.starts_with("/*") || line.starts_with('*') {
            continue;
        }
        let code = code_of(line);
        if let Some(key) = name_of(code) {
            let ty = &code[code.find(':').unwrap_or(0) + 1..];
            if let Some(keys) = inline_keys(ty) {
                face.inline.insert(key.clone(), keys);
            }
            face.keys.insert(key);
        }
    }
    out
}

fn keys_of(v: &Value) -> BTreeSet<String> {
    v.as_object().map(|o| o.keys().cloned().collect()).unwrap_or_default()
}

/// A maximal value goes in strictly and comes out; its key sets are the interface's.
fn holds<T: DeserializeOwned + Serialize>(faces: &BTreeMap<String, Face>, interface: &str, maximal: Value) {
    let face = faces
        .get(interface)
        .unwrap_or_else(|| panic!("{interface} is not in project.ts"));
    let typed: T = serde_json::from_value(maximal.clone())
        .unwrap_or_else(|e| panic!("{interface}: a key of project.ts is unknown to the Rust type: {e}"));
    let back = serde_json::to_value(&typed).expect("a value");
    assert_eq!(
        keys_of(&back),
        face.keys,
        "{interface}: the keys differ from project.ts"
    );
    assert_eq!(
        keys_of(&maximal),
        face.keys,
        "{interface}: the maximal value is not maximal"
    );
    for (field, inline) in &face.inline {
        let at = &back[field];
        let obj = if at.is_array() { &at[0] } else { at };
        assert_eq!(
            &keys_of(obj),
            inline,
            "{interface}.{field}: the inline keys differ from project.ts"
        );
    }
}

fn faces() -> BTreeMap<String, Face> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../engine-protocol/project.ts");
    interfaces(&std::fs::read_to_string(path).expect("project.ts"))
}

fn event() -> Value {
    json!({"from": "1:1", "to": 2.5, "fadeIn": "1", "fadeOut": "1", "loop": 2, "gain": 1.5, "disabled": true, "warp": false,
        "takes": [{"ref": "t", "start": "1:1"}], "comp": [{"take": "t", "from": "1:1", "to": "2:1"}],
        "crossfades": [{"at": "1:1", "length": "1"}]})
}

fn clip() -> Value {
    json!({"id": "c", "type": "audio", "name": "c", "source": "/a.wav", "start": 0, "length": 4, "offset": 0, "sourceFrom": 0.5,
        "sourceTo": 1.5, "fadeIn": 0, "fadeOut": 0, "gainDb": -3, "loopStart": 0, "loopEnd": 0,
        "notes": [{"time": 0, "duration": 1, "key": 60, "vel": 1, "channel": 0, "rel": 1, "repeat": 2, "skip": 1}],
        "warp": {"algorithm": "elastique", "markers": [[0, 0], [4, 2]]}, "warpOff": true, "disabled": true, "event": event(),
        "comp": {"of": "c", "take": "t"}, "crossfade": {"in": true, "out": true},
        "nest": {"of": "tl@0", "ref": "tl", "start": 0, "length": 8, "event": {"gain": 1, "disabled": true}}})
}

fn plugin() -> Value {
    json!({"id": "a:b", "type": "Sampler", "name": "b", "path": "p", "irFile": "i", "namFile": "n", "surgePatch": "s", "sfzFile": "z",
        "deckFile": "d", "dspGraph": "{}", "patternSource": "s", "patternLabel": "l",
        "patternCatchAll": true, "patternClaimed": ["a"], "patternPlays": "both", "patternOnly": true, "patternBanks": ["b"],
        "sidechainFrom": "t", "presetSave": "", "role": "audioFX", "enabled": true, "params": {"x": 1.5},
        "rack": [{"symbol": "BD", "note": 36, "clip": "/k.wav", "start": 0, "length": 0}], "tapOuts": [{"note": 36, "pair": 1}]})
}

fn track() -> Value {
    json!({"id": "t", "name": "t", "role": "regular", "contentType": "audio", "group": "", "generated": false, "color": "#fff",
        "mute": false, "solo": false, "highlight": true, "disabled": false, "inputChannels": [1], "outputChannels": [3],
        "monitor": "auto", "inputGain": 0, "clips": [clip()], "frozenMaterial": [clip()], "plugins": [plugin()],
        "sends": [{"to": "b", "level": 1, "enabled": true}], "inputFrom": {"plugin": "a:b", "pair": 1},
        "automation": [{"param": "x", "pluginId": "a:b", "bipolar": true, "points": [[0, 0.5]]}]})
}

#[test]
fn every_interface_of_project_ts_is_read_and_the_rust_types_keep_its_keys_both_ways() {
    let f = faces();
    holds::<Marker>(&f, "Marker", json!({"tc": "1:1:1", "name": "m", "beat": 0}));
    holds::<SettingsDoc>(
        &f,
        "SettingsDoc",
        json!({"metronome": true, "metronomeLevel": 0.5, "countIn": 1}),
    );
    holds::<ClipDoc>(&f, "ClipDoc", clip());
    holds::<RackSoundDoc>(
        &f,
        "RackSoundDoc",
        json!({"symbol": "BD", "note": 36, "clip": "/k.wav", "start": 0, "length": 0}),
    );
    holds::<PluginDoc>(&f, "PluginDoc", plugin());
    holds::<SendDoc>(&f, "SendDoc", json!({"to": "b", "level": 1, "enabled": true}));
    holds::<AutomationDoc>(
        &f,
        "AutomationDoc",
        json!({"param": "x", "pluginId": "a:b", "bipolar": true, "points": [[0, 1]]}),
    );
    holds::<TrackDoc>(&f, "TrackDoc", track());
    holds::<GroupDoc>(
        &f,
        "GroupDoc",
        json!({"id": "g", "name": "g", "color": "", "mute": false, "solo": false}),
    );
    let lane = json!({"id": "tl>l", "name": "l", "color": "", "disabled": false, "clips": [clip()]});
    holds::<TimelineDoc>(&f, "TimelineDoc", json!({"id": "tl", "length": 8, "lanes": [lane]}));
    let doc = json!({"uuid": "u", "name": "n", "tempo": 120, "sampleRate": 48000, "timeSigNumerator": 4, "timeSigDenominator": 4,
        "masterVolume": 0, "masterPlugins": [plugin()], "recordDir": "/r", "recordMute": true, "tracks": [track()],
        "groups": [{"id": "g", "name": "g", "color": "", "mute": false, "solo": false}], "markers": [], "timelines": [],
        "settings": {"metronome": false, "metronomeLevel": 0.5, "countIn": 0}});
    holds::<ProjectDoc>(&f, "ProjectDoc", doc);
}

#[test]
fn the_parser_sees_every_interface_with_its_fields() {
    let f = faces();
    for (name, n) in [
        ("ProjectDoc", 15),
        ("TrackDoc", 21),
        ("PluginDoc", 24),
        ("ClipDoc", 22),
        ("EventDoc", 11),
    ] {
        assert_eq!(f[name].keys.len(), n, "{name}");
    }
    assert!(f["EventDoc"].inline.contains_key("takes"));
}

#[test]
fn a_key_the_typescript_lacks_is_a_difference_the_test_would_see() {
    let f = faces();
    let mut v = track();
    v["extra"] = json!(1);
    assert!(serde_json::from_value::<TrackDoc>(v).is_err());
    assert!(!f["TrackDoc"].keys.contains("extra"));
}

#[test]
fn constructors_make_documents_that_round_trip() {
    for v in [
        serde_json::to_value(ProjectDoc::empty()).expect("a value"),
        serde_json::to_value(TrackDoc::new("t")).expect("a value"),
        serde_json::to_value(PluginDoc::new("a:b")).expect("a value"),
    ] {
        assert!(!v.to_string().contains("null"));
    }
    assert!(ProjectDoc::empty().finite());
}
