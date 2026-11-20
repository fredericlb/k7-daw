//! The render route's doc walk (3f-5): every path field of a posted doc is mapped to a real one or refused as a 403.

use k7_file_service::verb_host::docpaths::{Mapper, map_doc};
use k7_file_service::verb_paths::Virtual;
use serde_json::{Value, json};
use song::doc::{ProjectDoc, TrackDoc};
use song::roots::Roots;

use crate::support::jobs::Tmp;

fn world(t: &Tmp) -> Virtual {
    t.write("ws/songs/a.k7/a.wav", "w");
    t.write("studio/models/m.onnx", "m");
    t.write("outside/secret.wav", "no");
    Virtual::new(
        t.at("ws/songs").trim_end_matches('/'),
        &[],
        &t.at("code"),
        &t.at("studio"),
        &Roots::default(),
    )
}

fn clip(source: &str) -> Value {
    json!({
        "id": "c", "type": "audio", "name": "c", "source": source, "start": 0.0, "length": 1.0,
        "offset": 0.0, "fadeIn": 0.0, "fadeOut": 0.0, "loopStart": 0.0, "loopEnd": 0.0, "notes": []
    })
}

fn plugin(extra: Value) -> Value {
    let mut p = json!({
        "id": "p", "type": "vst3", "name": "p", "path": "", "presetSave": "", "role": "audioFX",
        "enabled": true, "params": {}
    });
    p.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    p
}

fn track(clips: Vec<Value>, plugins: Vec<Value>) -> TrackDoc {
    serde_json::from_value(json!({
        "id": "t", "name": "t", "role": "audio", "contentType": "audio", "group": "", "generated": false,
        "color": "", "mute": false, "solo": false, "disabled": false, "inputChannels": [], "outputChannels": [],
        "monitor": "off", "inputGain": 0.0, "clips": clips, "plugins": plugins, "sends": [], "automation": []
    }))
    .unwrap()
}

fn doc(t: TrackDoc) -> ProjectDoc {
    let mut d = ProjectDoc::empty();
    d.tracks.push(t);
    d
}

fn mapper<'a>(v: &'a Virtual, base: Option<&'a str>) -> Mapper<'a> {
    Mapper { virt: v, base }
}

#[test]
fn a_clip_and_a_plugin_file_under_the_roots_are_made_real() {
    let t = Tmp::new("vh-doc");
    let v = world(&t);
    let mut d = doc(track(
        vec![clip("/w/a.k7/a.wav"), clip("")],
        vec![plugin(
            json!({ "irFile": "/s/models/m.onnx", "deckFile": "/w/a.k7/a.wav" }),
        )],
    ));
    map_doc(&mapper(&v, None), &mut d).unwrap();
    assert_eq!(d.tracks[0].clips[0].source, t.at("ws/songs/a.k7/a.wav"));
    assert_eq!(d.tracks[0].clips[1].source, "");
    let p = &d.tracks[0].plugins[0];
    assert_eq!(
        p.ir_file.as_deref(),
        Some(t.at("studio/models/m.onnx").as_str())
    );
    assert_eq!(
        p.deck_file.as_deref(),
        Some(t.at("ws/songs/a.k7/a.wav").as_str())
    );
}

#[test]
fn every_path_field_outside_the_roots_is_a_403() {
    let t = Tmp::new("vh-doc-bad");
    let v = world(&t);
    let real = t.at("outside/secret.wav");
    let bad = [
        "/etc/passwd",
        real.as_str(),
        "/w/../../outside/secret.wav",
        "a.wav",
        "../a.wav",
    ];
    for b in bad {
        let fields = [
            doc(track(vec![clip(b)], vec![])),
            doc(track(vec![], vec![plugin(json!({ "path": b }))])),
            doc(track(vec![], vec![plugin(json!({ "irFile": b }))])),
            doc(track(vec![], vec![plugin(json!({ "namFile": b }))])),
            doc(track(vec![], vec![plugin(json!({ "surgePatch": b }))])),
            doc(track(vec![], vec![plugin(json!({ "sfzFile": b }))])),
            doc(track(vec![], vec![plugin(json!({ "deckFile": b }))])),
            doc(track(vec![], vec![plugin(json!({ "patternBanks": [b] }))])),
            doc(track(
                vec![],
                vec![plugin(
                    json!({ "rack": [{ "symbol": "s", "note": 1, "clip": b, "start": 0.0, "length": 1.0 }] }),
                )],
            )),
            {
                let mut d = doc(track(vec![], vec![]));
                d.record_dir = b.to_string();
                d
            },
            {
                let mut d = doc(track(vec![], vec![]));
                d.master_plugins
                    .push(serde_json::from_value(plugin(json!({ "path": b }))).unwrap());
                d
            },
            {
                let mut d = doc(track(vec![], vec![]));
                d.tracks[0].frozen_material = Some(vec![serde_json::from_value(clip(b)).unwrap()]);
                d
            },
        ];
        for (i, mut d) in fields.into_iter().enumerate() {
            let e = map_doc(&mapper(&v, None), &mut d).unwrap_err();
            assert_eq!((e.status, i), (403, i), "{b} in field {i}");
        }
    }
}

#[test]
fn a_relative_path_resolves_under_the_base_alone() {
    let t = Tmp::new("vh-doc-base");
    let v = world(&t);
    let base = t.at("ws/songs/a.k7");
    let base = base.trim_end_matches('/');
    let mut d = doc(track(vec![clip("a.wav")], vec![]));
    map_doc(&mapper(&v, Some(base)), &mut d).unwrap();
    assert_eq!(d.tracks[0].clips[0].source, t.at("ws/songs/a.k7/a.wav"));
    for bad in ["../../outside/secret.wav", "a\\b.wav", "C:x.wav"] {
        let mut d = doc(track(vec![clip(bad)], vec![]));
        assert_eq!(
            map_doc(&mapper(&v, Some(base)), &mut d).unwrap_err().status,
            403,
            "{bad}"
        );
    }
}

#[cfg(unix)]
#[test]
fn a_link_out_of_the_base_is_refused() {
    let t = Tmp::new("vh-doc-link");
    let v = world(&t);
    std::os::unix::fs::symlink(t.at("outside"), t.at("ws/songs/a.k7/out")).unwrap();
    let base = t.at("ws/songs/a.k7");
    let mut d = doc(track(vec![clip("out/secret.wav")], vec![]));
    let e = map_doc(&mapper(&v, Some(base.trim_end_matches('/'))), &mut d).unwrap_err();
    assert_eq!(e.status, 403);
}
