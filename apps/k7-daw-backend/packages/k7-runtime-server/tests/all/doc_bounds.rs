//! doc_bounds.rs — the document read off the wire: the C++ `DocJson` tests' behaviours, the bounds table, nlohmann's
//! refusals and the golden file `DocReader_test.cpp` reads on the other side of the ABI.

use k7_runtime_server::doc::{Checks, parse_project, parse_tracks};
use k7_runtime_server::wire::json::dump;
use serde_json::{Value, json};
use std::cell::RefCell;

type Warned = RefCell<Vec<String>>;

/// Every path but a UNC one passes: the engine with no roots set, as the C++ tests ran it.
fn open(warned: &Warned) -> Checks<'_> {
    let ok = |p: &str| !p.starts_with("\\\\");
    Checks::new(ok, ok, |m, s| warned.borrow_mut().push(format!("{s}|{m}")))
}

/// Reads under `/music/`, writes under `/music/rec/`: serve()'s roots.
fn rooted(warned: &Warned) -> Checks<'_> {
    Checks::new(
        |p| p.starts_with("/music/"),
        |p| p.starts_with("/music/rec/"),
        |m, s| warned.borrow_mut().push(format!("{s}|{m}")),
    )
}

fn doc_of(doc: Value) -> (Value, Vec<String>) {
    let warned = Warned::default();
    let parsed =
        parse_project(&json!({"doc": doc}), "k7", &open(&warned)).expect("a readable document");
    (parsed.doc, warned.into_inner())
}

fn refusal(doc: Value) -> String {
    let warned = Warned::default();
    match parse_project(&json!({"doc": doc}), "k7", &open(&warned)) {
        Ok(_) => panic!("the document was read"),
        Err(e) => e.0,
    }
}

#[test]
fn a_time_signature_tracktion_cannot_divide_by_is_read_as_four_four_and_said() {
    let (doc, warned) = doc_of(json!({"timeSigNumerator": 0, "timeSigDenominator": 0}));
    assert_eq!(
        (
            doc["timeSigNumerator"].clone(),
            doc["timeSigDenominator"].clone()
        ),
        (json!(4), json!(4))
    );
    assert_eq!(warned, ["|time signature 0/0 cannot be played — using 4/4"]);
    let (doc, warned) = doc_of(json!({"timeSigNumerator": 7, "timeSigDenominator": 8}));
    assert_eq!(
        (
            doc["timeSigNumerator"].clone(),
            doc["timeSigDenominator"].clone()
        ),
        (json!(7), json!(8))
    );
    assert!(warned.is_empty());
}

#[test]
fn the_bounds_table_holds_at_each_edge() {
    let rate = |r: i64| {
        doc_of(json!({"sampleRate": r})).0["sampleRate"]
            .as_i64()
            .unwrap()
    };
    assert_eq!(
        [
            rate(7999),
            rate(8000),
            rate(768_000),
            rate(768_001),
            rate(0)
        ],
        [48000, 8000, 768_000, 48000, 48000]
    );
    let tempo = |t: f64| doc_of(json!({"tempo": t})).0["tempo"].as_f64().unwrap();
    assert_eq!(
        [tempo(0.0), tempo(-3.0), tempo(0.5), tempo(300.0)],
        [120.0, 120.0, 0.5, 300.0]
    );
    let sig = |n: i64, d: i64| {
        let doc = doc_of(json!({"timeSigNumerator": n, "timeSigDenominator": d})).0;
        (
            doc["timeSigNumerator"].as_i64().unwrap(),
            doc["timeSigDenominator"].as_i64().unwrap(),
        )
    };
    assert_eq!(
        [
            sig(1, 1),
            sig(64, 32),
            sig(65, 4),
            sig(4, 3),
            sig(4, 64),
            sig(0, 4),
            sig(4, -2)
        ],
        [(1, 1), (64, 32), (4, 4), (4, 4), (4, 4), (4, 4), (4, 4)]
    );
    // A count-in is not bounded here: Tracktion clamps it.
    assert_eq!(
        doc_of(json!({"settings": {"countIn": 9}})).0["settings"]["countIn"],
        9
    );
}

#[test]
fn loop_counts_and_midi_numbers_come_off_the_wire_bounded() {
    let note = json!({"key": 900, "repeat": 1_000_000_000, "skip": -5});
    let clip = json!({"type": "midi", "notes": [note, {"key": -4, "repeat": 64, "skip": 65}]});
    let (doc, _) = doc_of(json!({"tracks": [{"id": "t1", "clips": [clip]}]}));
    let notes = &doc["tracks"][0]["clips"][0]["notes"];
    assert_eq!(
        (
            notes[0]["key"].clone(),
            notes[0]["repeat"].clone(),
            notes[0]["skip"].clone()
        ),
        (json!(127), json!(64), json!(0))
    );
    assert_eq!(
        (
            notes[1]["key"].clone(),
            notes[1]["repeat"].clone(),
            notes[1]["skip"].clone()
        ),
        (json!(0), json!(64), json!(64))
    );
}

#[test]
fn an_output_pair_is_a_rack_grown_to_that_many_outputs_so_it_is_held_to_0_through_31() {
    let plugin =
        json!({"id": "kit", "tapOuts": [{"note": 400, "pair": 1_000_000}], "rack": [{"note": -1}]});
    let track = json!({"id": "t", "plugins": [plugin], "inputFrom": {"plugin": "kit", "pair": -7}});
    let (doc, _) = doc_of(json!({"tracks": [track]}));
    let t = &doc["tracks"][0];
    assert_eq!(
        t["plugins"][0]["tapOuts"][0],
        json!({"note": 127, "pair": 31})
    );
    assert_eq!(t["plugins"][0]["rack"][0]["note"], 0);
    assert_eq!(t["inputFrom"], json!({"plugin": "kit", "pair": 0}));
}

#[test]
fn a_repeated_id_is_renamed_in_document_order_and_said() {
    let plug = json!({"id": "fx", "type": "4osc"});
    let clip = json!({"id": "c", "type": "midi"});
    let t1 = json!({"id": "t", "plugins": [plug], "clips": [clip, clip]});
    let t2 = json!({"id": "t", "plugins": [plug]});
    let (doc, warned) = doc_of(json!({"tracks": [t1, t2]}));
    let ids = |t: usize, key: &str| -> Vec<String> {
        doc["tracks"][t][key]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x["id"].as_str().unwrap().to_owned())
            .collect()
    };
    assert_eq!(
        (
            doc["tracks"][0]["id"].clone(),
            doc["tracks"][1]["id"].clone()
        ),
        (json!("t"), json!("t#2"))
    );
    assert_eq!(
        (ids(0, "plugins"), ids(1, "plugins")),
        (vec!["fx".to_owned()], vec!["fx#2".to_owned()])
    );
    assert_eq!(ids(0, "clips"), ["c", "c#2"]);
    assert_eq!(warned.len(), 3);
    assert_eq!(
        warned[0],
        "|two tracks share the id 't' — the second is 't#2'"
    );
}

#[test]
fn the_master_chain_shares_the_plugin_ids_of_the_tracks() {
    let (doc, warned) = doc_of(
        json!({"masterPlugins": [{"id": "fx"}], "tracks": [{"id": "t", "plugins": [{"id": "fx"}]}]}),
    );
    assert_eq!(doc["masterPlugins"][0]["id"], "fx");
    assert_eq!(doc["tracks"][0]["plugins"][0]["id"], "fx#2");
    assert_eq!(
        warned,
        ["|two modules share the id 'fx' — the second is 'fx#2'"]
    );
}

#[test]
fn a_preset_save_path_is_a_vstpreset_or_nothing() {
    let save = |p: &str| doc_of(json!({"masterPlugins": [{"id": "a", "presetSave": p}]}));
    assert_eq!(
        save("/x/Amp.VSTPRESET").0["masterPlugins"][0]["presetSave"],
        "/x/Amp.VSTPRESET"
    );
    let (doc, warned) = save("/x/.bashrc");
    assert_eq!(doc["masterPlugins"][0]["presetSave"], "");
    assert_eq!(warned.len(), 1);
    assert_eq!(
        save("\\\\host\\share\\a.vstpreset").0["masterPlugins"][0]["presetSave"],
        ""
    );
}

#[test]
fn a_path_outside_the_roots_is_dropped_where_it_arrives_and_remembered() {
    let warned = Warned::default();
    let checks = rooted(&warned);
    let clip = json!({"id": "c", "source": "/music/a.wav"});
    let stray = json!({"id": "d", "source": "/etc/b.wav"});
    let plugin =
        json!({"id": "p", "namFile": "/etc/passwd", "presetSave": "/music/rec/p.vstpreset"});
    let track = json!({"id": "t", "clips": [clip, stray], "plugins": [plugin]});
    let msg = json!({"doc": {"recordDir": "/", "tracks": [track]}});
    let parsed = parse_project(&msg, "k7", &checks).unwrap();
    let t = &parsed.doc["tracks"][0];
    assert_eq!(t["clips"][0]["source"], "/music/a.wav");
    assert_eq!(t["clips"][1]["source"], "");
    assert_eq!(t["plugins"][0]["namFile"], "");
    assert_eq!(t["plugins"][0]["presetSave"], "/music/rec/p.vstpreset");
    assert_eq!(parsed.doc["recordDir"], "");
    assert_eq!(*checks.refused.borrow(), ["/", "/etc/b.wav", "/etc/passwd"]);
    assert_eq!(warned.borrow().len(), 3);
}

#[test]
fn an_instrument_reference_may_not_climb_out_of_the_instruments_folder() {
    let sfz = |p: &str| {
        doc_of(json!({"masterPlugins": [{"sfzFile": p}]})).0["masterPlugins"][0]["sfzFile"].clone()
    };
    assert_eq!(sfz("@instruments/a/b.sfz"), "@instruments/a/b.sfz");
    assert_eq!(sfz("@instruments/a/../b.sfz"), "");
    assert_eq!(
        sfz("@instruments\\a\\..\\b.sfz"),
        "@instruments\\a\\..\\b.sfz"
    );
    assert_eq!(sfz("@instruments/..\\b.sfz"), "");
    assert_eq!(sfz("/home/me/x.sfz"), "/home/me/x.sfz");
}

#[test]
fn a_field_of_the_wrong_type_is_refused_with_nlohmanns_words() {
    assert_eq!(
        refusal(json!({"tempo": "fast"})),
        "[json.exception.type_error.302] type must be number, but is string"
    );
    assert_eq!(
        refusal(json!({"name": null})),
        "[json.exception.type_error.302] type must be string, but is null"
    );
    assert_eq!(
        refusal(json!({"recordMute": 1})),
        "[json.exception.type_error.302] type must be boolean, but is number"
    );
    assert_eq!(
        refusal(json!([1])),
        "[json.exception.type_error.306] cannot use value() with array"
    );
    assert_eq!(
        refusal(json!({"tracks": [3]})),
        "[json.exception.type_error.306] cannot use value() with number"
    );
    assert_eq!(
        refusal(json!({"tracks": [{"plugins": [{"params": {"a": "x"}}]}]})),
        "[json.exception.type_error.302] type must be number, but is string"
    );
    let warned = Warned::default();
    let missing = parse_project(&json!({"type": "project"}), "k7", &open(&warned));
    assert_eq!(
        missing.err().unwrap().0,
        "[json.exception.out_of_range.403] key 'doc' not found"
    );
    let marker = json!({"tracks": [{"clips": [{"warp": {"markers": [[1]]}}]}]});
    assert_eq!(
        refusal(marker),
        "[json.exception.out_of_range.401] array index 1 is out of range"
    );
}

#[test]
fn a_footswitch_is_carried_for_the_slot_only_and_with_its_plugin_ids_alone() {
    let msg = json!({"doc": {"footswitch": {"cc": 64, "plugins": ["fx", 3]}}});
    let warned = Warned::default();
    let slot = parse_project(&msg, "slot", &open(&warned)).unwrap();
    assert_eq!(slot.doc["footswitch"], json!({"cc": 64, "plugins": ["fx"]}));
    assert!(
        parse_project(&msg, "k7", &open(&warned))
            .unwrap()
            .doc
            .get("footswitch")
            .is_none()
    );
    let bad = json!({"doc": {"footswitch": {"cc": 1.5}}});
    assert!(
        parse_project(&bad, "slot", &open(&warned))
            .unwrap()
            .doc
            .get("footswitch")
            .is_none()
    );
}

#[test]
fn a_tracks_delta_reads_each_track_as_a_document_does_without_making_ids_unique() {
    let warned = Warned::default();
    let msg = json!({"tracks": [{"id": "a", "clips": [{"id": "c", "notes": [{"key": 500}]}, {"id": "c"}]}, {"id": "a"}]});
    let tracks = parse_tracks(&msg, &open(&warned)).unwrap();
    assert_eq!(tracks[0]["clips"][0]["notes"][0]["key"], 127);
    assert_eq!(
        (tracks[0]["clips"][1]["id"].clone(), tracks[1]["id"].clone()),
        (json!("c"), json!("a"))
    );
    assert!(warned.borrow().is_empty());
    assert_eq!(
        parse_tracks(&json!({}), &open(&warned)).err().unwrap().0,
        "[json.exception.out_of_range.403] key 'tracks' not found"
    );
}

fn fixture(name: &str) -> String {
    crate::repo_file(&format!(
        "apps/k7-daw-backend/packages/k7-runtime-server/fixtures/{name}"
    ))
}

/// The golden file: the raw document read as the C++ side will read it, held to `DocReader_test.cpp` as well.
#[test]
fn the_golden_document_is_what_the_cpp_reader_is_held_to() {
    let raw: Value = serde_json::from_str(&fixture("project.raw.json")).unwrap();
    let warned = Warned::default();
    let parsed = parse_project(&raw, "slot", &rooted(&warned)).unwrap();
    let got = format!("{}\n", dump(&parsed.doc));
    if std::env::var_os("K7_UPDATE_GOLDEN").is_some() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/fixtures/project.validated.json"
        );
        std::fs::write(path, &got).unwrap();
    }
    assert_eq!(got, fixture("project.validated.json"));
    assert_eq!((parsed.tracks, parsed.clips, parsed.plugins), (2, 3, 2));
}
