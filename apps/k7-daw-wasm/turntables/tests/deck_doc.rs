// deck_doc.rs — the decks as one document: the stand-in, the compiled project filled in, the click and which deck it follows.
// A reader would get wrong whose cases these are: deck.test.ts and metronome.test.ts, the fixtures spelled as JSON the way the page sends them.

use indexmap::IndexMap;
use serde_json::{Value, json};
use song::doc::{PluginDoc, ProjectDoc, TrackDoc};
use song_schema::{STEMS, stem_lane, stem_platter};
use turntables::deck_doc::{DeckSet, deck_doc, document, with_decks};
use turntables::metronome::{metronome_deck, metronome_track};
use turntables::vocab::{DECKS, DeckLoads, DeckTempo, DeckTempos, PLATTER_TYPE};

fn set(v: Value) -> DeckSet {
    serde_json::from_value(v).unwrap()
}

fn loads(stems: bool) -> Value {
    json!({
        "deckA": { "path": "/m/song/original.mp3", "title": "A side", "artist": "Artist", "id": "songs/a", "stems": stems },
        "deckB": { "path": "/m/b.wav", "title": "B side", "artist": null, "id": "songs/b" },
    })
}

fn tempo(natural: f64, target: f64) -> DeckTempo {
    DeckTempo {
        natural,
        seconds: 60.0,
        target,
        phase: None,
        keylock: None,
    }
}

/// A platter as the PROJECT declares one: what the compiler emits and `with_decks` patches.
fn declared(id: &str, module: &str) -> PluginDoc {
    let mut p = PluginDoc::new(&format!("{id}:{module}"));
    p.plugin_type = PLATTER_TYPE.to_owned();
    p.name = module.to_owned();
    for (k, v) in [("Rate", 0.0), ("Seek", 0.0), ("Gain", 1.0), ("Key", 0.0)] {
        p.params.insert(k.to_owned(), v);
    }
    p
}

fn compiled() -> ProjectDoc {
    let mut doc = ProjectDoc::empty();
    for deck in DECKS {
        let mut t = TrackDoc::new(deck);
        t.plugins.push(declared(deck, &format!("platter{}", &deck[4..])));
        doc.tracks.push(t);
    }
    for deck in DECKS {
        for stem in STEMS {
            let mut t = TrackDoc::new(&stem_lane(deck, stem));
            t.plugins
                .push(declared(&stem_lane(deck, stem), &stem_platter(deck, stem)));
            doc.tracks.push(t);
        }
    }
    doc
}

fn track<'a>(doc: &'a ProjectDoc, id: &str) -> &'a TrackDoc {
    doc.tracks.iter().find(|t| t.id == id).unwrap()
}

#[test]
fn the_compiled_project_keeps_its_tracks_and_gains_the_click() {
    let doc = with_decks(compiled(), &set(json!({ "loads": loads(false) })));
    let ids: Vec<&str> = doc.tracks.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids.len(), 11);
    assert_eq!(ids.last(), Some(&"metronome"));
    assert_eq!(doc.tempo, 120.0);
}

#[test]
fn a_record_goes_on_its_decks_platter_and_the_lane_holds_no_clip() {
    let doc = with_decks(compiled(), &set(json!({ "loads": loads(false) })));
    assert_eq!(
        track(&doc, "deckA").plugins[0].deck_file.as_deref(),
        Some("/m/song/original.mp3")
    );
    assert_eq!(track(&doc, "deckB").plugins[0].deck_file.as_deref(), Some("/m/b.wav"));
    assert!(track(&doc, "deckA").clips.is_empty());
}

#[test]
fn a_separated_record_plays_off_four_platters_and_its_own_lane_stays_silent() {
    let s = set(json!({
        "loads": loads(true),
        "tempos": { "deckA": { "natural": 120.0, "seconds": 60.0, "target": 126.0, "keylock": true } },
        "playing": { "deckA": true },
    }));
    let doc = with_decks(compiled(), &s);
    let bass = &track(&doc, &stem_lane("deckA", song_schema::Stem::Bass)).plugins[0];
    assert_eq!(bass.deck_file.as_deref(), Some("/m/song/stems/bass.wav"));
    let rates: Vec<f64> = STEMS
        .iter()
        .map(|s| track(&doc, &stem_lane("deckA", *s)).plugins[0].params["Rate"])
        .collect();
    assert!(rates.iter().all(|r| *r == rates[0]) && (rates[0] - 1.05).abs() < 1e-9);
    assert_eq!(bass.params["Key"], 1.0);
    // The record's own lane holds no record: the four parts ARE the record.
    let own = &track(&doc, "deckA").plugins[0];
    assert_eq!(own.plugin_type, PLATTER_TYPE);
    assert!(own.deck_file.is_none());
}

#[test]
fn an_unseparated_record_leaves_the_stem_lanes_empty_and_plays_off_the_deck() {
    let doc = with_decks(compiled(), &set(json!({ "loads": loads(false) })));
    for s in STEMS {
        let lane = track(&doc, &stem_lane("deckA", s));
        assert!(lane.clips.is_empty() && lane.plugins[0].deck_file.is_none());
    }
}

#[test]
fn a_document_never_carries_seek_and_a_stopped_deck_does_not_turn() {
    let doc = with_decks(compiled(), &set(json!({ "loads": loads(false) })));
    let p = &track(&doc, "deckA").plugins[0];
    assert!(!p.params.contains_key("Seek"));
    // Rate, Key and Gain stay in the order the file declared them, the first two replaced in place.
    assert_eq!(p.params.keys().collect::<Vec<_>>(), ["Rate", "Gain", "Key"]);
    assert_eq!(p.params["Rate"], 0.0);
}

#[test]
fn a_lane_the_decks_do_not_own_comes_back_untouched() {
    let mut base = compiled();
    base.tracks[0].clips = Vec::new();
    let mut other = TrackDoc::new("guitar");
    other.clips = vec![song::reads::blanks().clip];
    base.tracks.push(other.clone());
    let doc = with_decks(base, &set(json!({ "loads": loads(false) })));
    assert_eq!(track(&doc, "guitar"), &other);
}

#[test]
fn each_deck_turns_on_its_own_not_off_the_transport() {
    let s = |playing: Value| {
        set(
            json!({ "loads": loads(false), "tempos": { "deckA": { "natural": 120.0, "seconds": 60.0, "target": 126.0 } }, "playing": playing }),
        )
    };
    let rate = |doc: &ProjectDoc, d: &str| track(doc, d).plugins[0].params["Rate"];
    let doc = with_decks(compiled(), &s(json!({ "deckA": true })));
    assert!((rate(&doc, "deckA") - 1.05).abs() < 1e-9);
    assert_eq!(rate(&doc, "deckB"), 0.0);
    assert_eq!(rate(&with_decks(compiled(), &s(json!({}))), "deckA"), 0.0);
}

#[test]
fn the_stand_in_carries_both_decks_loaded_or_not_with_the_level_last() {
    let doc = deck_doc(&set(json!({ "loads": { "deckA": null, "deckB": null } })));
    let ids: Vec<&str> = doc.tracks.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, ["deckA", "deckB", "metronome"]);
    assert!(doc.record_dir.is_empty() && doc.groups.is_empty());
    let a = &track(&doc, "deckA").plugins;
    assert_eq!(a.last().map(|p| p.id.as_str()), Some("deckA:level"));
    // The crossfader at its centre: both decks at equal gain, about -6 dB.
    assert!((a.last().unwrap().params["Volume"] + 6.02).abs() < 0.01);
}

#[test]
fn the_stand_in_names_a_loaded_deck_by_its_title_and_puts_the_platter_first() {
    let doc = deck_doc(&set(json!({ "loads": loads(false), "mix": 0.0 })));
    assert_eq!(track(&doc, "deckA").name, "A side");
    let first = &track(&doc, "deckA").plugins[0];
    assert_eq!(
        (first.id.as_str(), first.deck_file.as_deref()),
        ("deckA:platter", Some("/m/song/original.mp3"))
    );
    // At the far end of the fader the far deck is silent.
    assert_eq!(track(&doc, "deckB").plugins.last().unwrap().params["Volume"], -60.0);
}

#[test]
fn a_modules_plugin_is_mapped_onto_the_engines_wire_form_in_order() {
    let s = set(json!({
        "loads": { "deckA": null, "deckB": null },
        "modules": { "deckA": [
            { "name": "eq", "plugin": { "type": "internal", "name": "EQ" }, "enabled": true },
            { "name": "bare", "enabled": false },
        ]},
    }));
    let doc = deck_doc(&s);
    let p = &track(&doc, "deckA").plugins;
    assert_eq!(p[0].id, "deckA:eq");
    assert_eq!((p[0].plugin_type.as_str(), p[0].name.as_str()), ("EQ", "eq"));
    assert!(!p[1].enabled);
    assert_eq!(p.len(), 3);
}

#[test]
fn the_compiled_project_wins_when_there_is_one() {
    let s = set(json!({ "loads": loads(false) }));
    assert_eq!(document(Some(compiled()), &s).tracks.len(), 11);
    assert_eq!(document(None, &s).tracks.len(), 3);
}

fn deck_loads(v: &Value) -> DeckLoads {
    serde_json::from_value(v.clone()).unwrap()
}

fn tempos(phase: Option<f64>) -> DeckTempos {
    IndexMap::from([(
        "deckA".to_owned(),
        DeckTempo {
            phase,
            ..tempo(120.0, 120.0)
        },
    )])
}

#[test]
fn a_click_follows_the_one_loaded_deck_or_the_master_and_needs_a_measured_beat() {
    let both = deck_loads(&loads(false));
    let only_a = deck_loads(&json!({ "deckA": loads(false)["deckA"], "deckB": null }));
    assert_eq!(
        metronome_deck(&only_a, &tempos(Some(0.1)), None).as_deref(),
        Some("deckA")
    );
    // Two free decks have no single beat to be on; synced, the master's is.
    assert_eq!(metronome_deck(&both, &tempos(Some(0.1)), None), None);
    assert_eq!(
        metronome_deck(&both, &tempos(Some(0.1)), Some("deckA")).as_deref(),
        Some("deckA")
    );
    // No phase measured, no tempo, nothing loaded: nothing to click on.
    assert_eq!(metronome_deck(&only_a, &tempos(None), None), None);
    assert_eq!(metronome_deck(&only_a, &IndexMap::new(), None), None);
    assert_eq!(
        metronome_deck(
            &deck_loads(&json!({ "deckA": null, "deckB": null })),
            &tempos(Some(0.1)),
            None
        ),
        None
    );
}

#[test]
fn the_click_track_is_always_there_and_clicks_on_the_decks_own_beat() {
    let off = metronome_track(Some(&tempo(128.0, 128.0)), false);
    assert!(off.clips.is_empty() && off.plugins.iter().any(|p| p.plugin_type == "4osc"));
    let on = metronome_track(Some(&tempo(128.0, 128.0)), true);
    let clip = &on.clips[0];
    // At 128 against a 120 document, a beat of the deck lasts 60/128 seconds, in document beats.
    let gap_seconds = (clip.notes[1].time - clip.notes[0].time) * 60.0 / 120.0;
    assert!((gap_seconds - 60.0 / 128.0).abs() < 1e-9);
    assert_eq!((clip.start, clip.notes.len()), (0.0, 4));
    assert!((clip.loop_end - clip.loop_start - 4.0 * 120.0 / 128.0).abs() < 1e-9);
    assert!(clip.notes[0].key > clip.notes[1].key && clip.notes[1].key == clip.notes[3].key);
}

#[test]
fn the_click_lasts_as_long_as_the_record_plays_stretch_included() {
    let clip = &metronome_track(Some(&tempo(128.0, 256.0)), true).clips[0];
    assert!((clip.length * 60.0 / 120.0 - 30.0).abs() < 1e-6);
    assert!(metronome_track(None, true).clips.is_empty());
}
