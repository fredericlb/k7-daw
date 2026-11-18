// views_project.rs — a ProjectDoc as the browser's WebProject: the mixer, the plugins, the groups, the timelines, every length by a table.
// A reader would get wrong that a track's `disabled` is never written and a strip is absent (not at unity) when no Mixer ends the chain.

use crate::views_support;

use serde_json::{Value, json};
use song::Marker;
use song::clips::{ClipDoc, TimelineDoc, TimelineLaneDoc};
use song::doc::{GroupDoc, PluginDoc, ProjectDoc, TrackDoc};
use song::views::lengths::{ByTable, Lengths};
use song::views::web::web_project_of;
use views_support::{clip, fast};

fn plugin(id: &str, kind: &str, name: &str) -> PluginDoc {
    let mut p = PluginDoc::new(id);
    p.plugin_type = kind.to_owned();
    p.name = name.to_owned();
    p
}

fn mixer(id: &str, name: &str, volume: Option<f64>, pan: Option<f64>) -> PluginDoc {
    let mut p = plugin(id, "Mixer", name);
    if let Some(v) = volume {
        p.params.insert("Volume".into(), v);
    }
    if let Some(v) = pan {
        p.params.insert("Pan".into(), v);
    }
    p
}

fn web(doc: &ProjectDoc, table: &Lengths) -> Value {
    let w = web_project_of(doc, &mut ByTable(table)).expect("a web project");
    serde_json::to_value(w).expect("json")
}

// 636, webProject.ts: the document's own fields, each under the browser's name.
#[test]
fn a_document_is_a_web_project_field_for_field() {
    let mut doc = ProjectDoc::empty();
    doc.name = "song".into();
    doc.tempo = 96.0;
    doc.time_sig_numerator = 3;
    doc.master_volume = -2.5;
    doc.settings.metronome = true;
    doc.settings.metronome_level = 0.4;
    doc.settings.count_in = 2;
    doc.groups.push(GroupDoc {
        id: "g".into(),
        name: "Drums".into(),
        color: "#fff".into(),
        mute: true,
        solo: false,
    });
    doc.markers.push(Marker {
        tc: "0:01".into(),
        name: "A".into(),
        beat: 4.0,
    });
    let w = web(&doc, &Lengths::new());
    assert_eq!(
        w,
        json!({
            "name": "song", "tempo": 96.0, "timeSigNumerator": 3, "timeSigDenominator": 4, "masterVolume": -2.5,
            "tracks": [],
            "groups": [{"id": "g", "name": "Drums", "color": "#fff", "mute": true, "solo": false}],
            "metronome": true, "metronomeLevel": 0.4, "countIn": 2,
            "markers": [{"beat": 4.0, "name": "A", "tc": "0:01"}]
        })
    );
}

// 636, toWebTracks: the LAST Mixer is the strip, its Volume and Pan read as 0 when the module has none.
#[test]
fn the_strip_is_the_last_mixer_with_missing_params_at_zero() {
    let mut t = TrackDoc::new("a");
    t.plugins = vec![
        mixer("a:1", "first", Some(1.0), Some(1.0)),
        plugin("a:2", "internal", "Chorus"),
        mixer("a:3", "last", Some(-6.0), None),
    ];
    let mut bare = TrackDoc::new("b");
    bare.plugins = vec![plugin("b:1", "internal", "Chorus")];
    let mut doc = ProjectDoc::empty();
    doc.tracks = vec![t, bare];
    let w = web(&doc, &Lengths::new());
    assert_eq!(
        w["tracks"][0]["mixer"],
        json!({"module": "last", "volume": -6.0, "pan": 0.0})
    );
    assert!(w["tracks"][1].get("mixer").is_none());
}

// 636: a plugin carries id, type, name and an sfzFile only when it is not empty; a track's `disabled` is never written.
#[test]
fn a_plugin_names_its_sfz_file_only_when_it_has_one_and_disabled_is_never_written() {
    let mut t = TrackDoc::new("a");
    let mut sfz = plugin("a:1", "internal", "Sampler");
    sfz.sfz_file = Some("@instruments/piano.sfz".into());
    let mut empty = plugin("a:2", "internal", "Sampler");
    empty.sfz_file = Some(String::new());
    t.plugins = vec![sfz, empty];
    t.disabled = true;
    t.highlight = Some(true);
    let mut quiet = TrackDoc::new("b");
    quiet.highlight = Some(false);
    let mut doc = ProjectDoc::empty();
    doc.tracks = vec![t, quiet];
    let w = web(&doc, &Lengths::new());
    assert_eq!(
        w["tracks"][0]["plugins"],
        json!([
            {"id": "a:1", "type": "internal", "name": "Sampler", "sfzFile": "@instruments/piano.sfz"},
            {"id": "a:2", "type": "internal", "name": "Sampler"},
        ])
    );
    assert_eq!(w["tracks"][0]["highlight"], json!(true));
    assert!(w["tracks"][1].get("highlight").is_none());
    assert!(
        w["tracks"]
            .as_array()
            .expect("tracks")
            .iter()
            .all(|t| t.get("disabled").is_none())
    );
}

// 636: frozen material is carried only when it holds a clip, and is not folded.
#[test]
fn frozen_material_is_carried_only_when_it_holds_a_clip() {
    let mut t = TrackDoc::new("a");
    t.frozen_material = Some(Vec::new());
    let mut f = TrackDoc::new("b");
    f.clips = vec![clip(json!({"id": "r__frozen", "length": 4}))];
    f.frozen_material = Some(vec![clip(json!({"id": "c@0", "length": 2}))]);
    let mut doc = ProjectDoc::empty();
    doc.tracks = vec![t, f];
    let w = web(&doc, &Lengths::new());
    assert!(w["tracks"][0].get("frozenMaterial").is_none());
    assert_eq!(w["tracks"][1]["frozenMaterial"][0]["id"], json!("c@0"));
    assert_eq!(w["tracks"][1]["clips"][0]["frozen"], json!(true));
}

fn lane(id: &str, clips: Vec<ClipDoc>) -> TimelineLaneDoc {
    TimelineLaneDoc {
        id: id.into(),
        name: format!("lane {id}"),
        color: "#0af".into(),
        disabled: false,
        clips,
    }
}

// 636, timelineLaneTrack: a lane is drawn as a track of either kind, with no strip, no group, no plugins; `timelines` is absent when empty.
#[test]
fn a_timeline_lane_is_drawn_as_a_track_and_timelines_are_absent_when_empty() {
    let mut doc = ProjectDoc::empty();
    doc.timelines = Some(Vec::new());
    assert!(web(&doc, &Lengths::new()).get("timelines").is_none());
    doc.timelines = Some(vec![TimelineDoc {
        id: "verse".into(),
        length: 16.0,
        lanes: vec![lane("verse>a", vec![clip(json!({"id": "x@0", "length": 2}))])],
    }]);
    let w = web(&doc, &Lengths::new());
    let l = &w["timelines"][0]["lanes"][0];
    assert_eq!(w["timelines"][0]["length"], json!(16.0));
    assert_eq!(l["id"], json!("verse>a"));
    assert_eq!(l["contentType"], json!("audio notes"));
    assert_eq!(l["group"], json!(""));
    assert_eq!(l["plugins"], json!([]));
    assert_eq!(l["color"], json!("#0af"));
    assert!(l.get("mixer").is_none() && l.get("disabled").is_none());
    assert_eq!(l["clips"][0]["id"], json!("x@0"));
}

// 611, 636: a clip with no length draws its file's length from the table, in a track and in a timeline lane; a path not in it draws 0.
#[test]
fn lengths_come_from_the_table_and_a_missing_path_draws_nothing() {
    let table: Lengths = [("/a/one.wav".to_owned(), 30.0)].into_iter().collect();
    let c = |id: &str, src: &str| clip(json!({"id": id, "source": src, "start": 0}));
    let mut t = TrackDoc::new("a");
    t.clips = vec![c("a@0", "/a/one.wav"), c("b@1", "/a/gone.wav")];
    let mut doc = ProjectDoc::empty();
    doc.tempo = 120.0;
    doc.tracks = vec![t];
    doc.timelines = Some(vec![TimelineDoc {
        id: "v".into(),
        length: 8.0,
        lanes: vec![lane("v>a", vec![c("c@0", "/a/one.wav")])],
    }]);
    let w = web(&doc, &table);
    assert_eq!(w["tracks"][0]["clips"][0]["length"], json!(60.0));
    assert_eq!(w["tracks"][0]["clips"][1]["length"], json!(0.0));
    assert_eq!(w["timelines"][0]["lanes"][0]["clips"][0]["length"], json!(60.0));
}

// 618: a full-size project (200 tracks of 2,000 clips) is one clip per clip, in time.
#[test]
fn a_full_size_project_is_drawn_in_time() {
    let table: Lengths = (0..2_000)
        .map(|i| (format!("/a/a{i}.wav"), 1.0 + f64::from(i % 7)))
        .collect();
    let lane: Vec<ClipDoc> = (0..2_000)
        .map(|i| clip(json!({"id": format!("c{i}@{i}"), "source": format!("/a/a{i}.wav"), "start": i})))
        .collect();
    let mut doc = ProjectDoc::empty();
    doc.tracks = (0..200)
        .map(|n| {
            let mut t = TrackDoc::new(&format!("t{n}"));
            t.clips = lane.clone();
            t
        })
        .collect();
    let w = fast("a full-size web project", || {
        web_project_of(&doc, &mut ByTable(&table)).expect("a web project")
    });
    assert_eq!(w.tracks.len(), 200);
    assert_eq!(w.tracks.iter().map(|t| t.clips.len()).sum::<usize>(), 400_000);
    assert!(w.tracks.iter().all(|t| t.clips.iter().all(|c| c.length > 0.0)));
}
