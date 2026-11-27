// tests_text_desk.rs — the cue sheet, playlists, library pins, assembly template, chain file and short forms: text in, the Rust writers' text out.
// What is pinned hardest is what a hand-edited file keeps: comments, order, one line per change.

use crate::tests_support::{call, data, edited, refused};
use serde_json::{Value, json};

const A: &str = "records/Daft Punk/Around the world.flac";
const B: &str = "records/Alain Bashung/La nuit je mens.flac";

fn plain() -> String {
    format!("// where this set drops the needle\ncues([\n  \"{A}\",\n  \"{B}\",\n])\n")
}

/// A song's cue points written over a whole file's text, as the desk does.
fn write(text: &str, reference: &str, markers: Value) -> Result<String, String> {
    edited("text.mix", json!({"text": text, "ops": [{"op": "withMarkers", "ref": reference, "markers": markers}]}))
        .map(|(t, _)| t)
}

fn cues(text: &str) -> Value {
    call("text.mix_cues", json!({"text": text}))
}

fn markers_of(text: &str) -> Value {
    cues(text)["markers"].clone()
}

#[test]
fn a_sheet_written_before_markers_existed_still_reads() {
    assert_eq!(cues(&plain()), json!({"markers": {}, "issues": []}));
}

#[test]
fn a_ref_that_carries_cues_is_read_beside_one_that_does_not() {
    let got = cues(&format!("cues([\n  \"{A}\",\n  ref(\"{B}\").markers([12.5, null, 48]),\n])\n"));
    assert_eq!(got["issues"], json!([]));
    assert_eq!(got["markers"], json!({B: [12.5, null, 48]}));
}

#[test]
fn a_sheet_with_nothing_cued_or_no_file_at_all_is_empty_not_broken() {
    assert_eq!(cues("cues([])\n")["issues"], json!([]));
    assert_eq!(cues("")["issues"], json!([]));
}

#[test]
fn a_sheet_with_too_many_cues_a_cue_before_the_start_an_unnameable_ref_or_an_unknown_key_is_refused() {
    for text in [
        format!("cues([ref(\"{A}\").markers([1, 2, 3, 4, 5, 6, 7, 8, 9])])\n"),
        format!("cues([ref(\"{A}\").markers([-3])])\n"),
        "cues([\n  \"@k7-core/songs/a/b/c\",\n])\n".to_owned(),
        "cues([])\nsongs([])\n".to_owned(),
        "cues([\n".to_owned(),
    ] {
        let got = cues(&text);
        assert!(!got["issues"].as_array().unwrap().is_empty(), "{text}");
        assert_eq!(got["markers"], json!({}), "{text}");
    }
}

#[test]
fn a_cue_is_written_keeping_the_comments_and_the_other_entries() {
    let out = write(&plain(), B, json!([12.5])).unwrap();
    assert!(out.contains("// where this set drops the needle"));
    assert!(out.contains(&format!("  \"{A}\",\n")));
    assert_eq!(markers_of(&out), json!({B: [12.5]}));
}

#[test]
fn a_slot_is_held_open_so_button_3_stays_button_3() {
    assert_eq!(markers_of(&write(&plain(), A, json!([null, null, 48])).unwrap()), json!({A: [null, null, 48]}));
}

#[test]
fn trailing_empties_are_dropped_rather_than_eight_nulls_written() {
    let out = write(&plain(), A, json!([12.5, null, null, null, null, null, null])).unwrap();
    assert_eq!(markers_of(&out), json!({A: [12.5]}));
    assert!(!out.contains("null"));
}

#[test]
fn clearing_the_last_cue_goes_back_to_a_bare_ref() {
    let cued = write(&plain(), A, json!([12.5])).unwrap();
    assert_eq!(write(&cued, A, json!([null])).unwrap(), plain());
}

#[test]
fn a_songs_cues_are_replaced_not_added_as_a_second_entry() {
    let twice = write(&write(&plain(), A, json!([10])).unwrap(), A, json!([10, 20])).unwrap();
    assert_eq!(markers_of(&twice), json!({A: [10, 20]}));
    assert_eq!(twice.matches("Around the world").count(), 1);
}

#[test]
fn a_ref_the_file_does_not_name_is_appended_and_the_key_made_from_nothing() {
    assert_eq!(markers_of(&write("cues([])\n", A, json!([5])).unwrap()), json!({A: [5]}));
    let fresh = write("", A, json!([5])).unwrap();
    assert_eq!(cues(&fresh), json!({"markers": {A: [5]}, "issues": []}));
}

#[test]
fn the_cues_are_written_on_one_line_with_the_ref_they_belong_to() {
    assert!(write(&plain(), A, json!([1, 2, 3])).unwrap().contains(&format!("  ref(\"{A}\").markers([1, 2, 3]),\n")));
}

#[test]
fn a_sheet_that_does_not_read_is_left_as_it_is_and_says_so() {
    assert!(write("cues([", A, json!([1])).is_err());
}

#[test]
fn the_desks_bindings_say_which_control_drives_which_param_null_for_one_not_wired() {
    let got = call("text.mix_view", json!({"text": "fade({ module: \"xf\", param: \"Level\", invert: true })\nfree(null)\n"}));
    assert_eq!(got["issues"], json!([]));
    assert_eq!(got["value"], json!({"fade": {"module": "xf", "param": "Level", "invert": true}, "free": null}));
}

#[test]
fn a_project_that_wired_nothing_has_empty_bindings_and_a_wrong_one_says_why() {
    assert_eq!(call("text.mix_view", json!({"text": ""})), json!({"value": {}, "issues": []}));
    let bad = call("text.mix_view", json!({"text": "fade({ module: \"\", param: \"x\" })\n"}));
    assert_eq!(bad["value"], Value::Null);
}

fn playlist(text: &str) -> Value {
    call("schema.parse_playlist", json!({"text": text}))
}

fn stub(name: &str) -> String {
    call("text.playlist_stub", json!({"name": name})).as_str().unwrap().to_owned()
}

fn append(text: &str, song: &str, variant: Option<&str>) -> Result<(String, Vec<bool>), String> {
    let mut op = json!({"op": "append", "song": song});
    if let Some(v) = variant {
        op["variant"] = json!(v);
    }
    edited("text.playlist", json!({"text": text, "ops": [op]}))
}

fn songs(text: &str) -> Vec<Value> {
    playlist(text)["items"].as_array().unwrap().iter().map(|i| i["song"].clone()).collect()
}

#[test]
fn a_playlist_round_trips_its_own_stub_so_a_file_it_creates_opens_on_a_playlist() {
    let s = stub("répèt du jeudi");
    assert_eq!(s, "// répèt du jeudi\ntitle(\"répèt du jeudi\")\nitems([])\n");
    assert_eq!(playlist(&s), json!({"title": "répèt du jeudi", "items": []}));
}

#[test]
fn a_song_is_added_to_the_end_of_the_list_bare_when_it_names_no_variant() {
    assert_eq!(
        append("title(\"Set\")\nitems([\n  \"Lithium\",\n])\n", "Hells Bells", None).unwrap().0,
        "title(\"Set\")\nitems([\n  \"Lithium\",\n  \"Hells Bells\",\n])\n"
    );
}

#[test]
fn the_stubs_empty_list_is_filled_and_the_key_added_when_the_file_has_none() {
    let out = append(&stub("Set"), "Lithium", None).unwrap().0;
    assert!(out.starts_with("// Set\n"));
    assert_eq!(playlist(&out)["items"], json!([{"song": "Lithium"}]));
    assert_eq!(
        playlist(&append("title(\"Set\")\n", "Lithium", None).unwrap().0),
        json!({"title": "Set", "items": [{"song": "Lithium"}]})
    );
}

#[test]
fn the_variant_is_written_beside_the_song() {
    let out = append("items([\"Lithium\"])\n", "Nevermind/Lithium", Some("inst")).unwrap().0;
    assert_eq!(playlist(&out)["items"][1], json!({"song": "Nevermind/Lithium", "variant": "inst"}));
}

#[test]
fn an_append_keeps_the_comments_and_the_order() {
    let before = "// la répèt du jeudi\ntitle(\"Répèt\")\nitems([\n  // on ouvre là-dessus\n  \"Hells Bells\",\n  \"Lithium\",\n])\n";
    let out = append(before, "Rock 'n' Roll: Part 2", None).unwrap().0;
    assert!(out.contains("// la répèt du jeudi"));
    assert!(out.contains("  // on ouvre là-dessus"));
    assert_eq!(songs(&out), [json!("Hells Bells"), json!("Lithium"), json!("Rock 'n' Roll: Part 2")]);
}

#[test]
fn an_items_that_is_no_list_is_refused_not_guessed_leaving_the_text_alone() {
    let text = "items(\"nonsense\")\n";
    assert_eq!(append(text, "x", None).unwrap(), (text.to_owned(), vec![false]));
}

const SET: &str = "items([\n  \"a\",\n  // the second\n  song(\"b\").variant(\"v\"),\n  \"c\",\n])\n";

fn playlist_op(text: &str, op: Value) -> (String, Vec<bool>) {
    edited("text.playlist", json!({"text": text, "ops": [op]})).unwrap()
}

#[test]
fn one_line_is_dropped_by_its_position_and_one_that_is_not_there_is_refused() {
    assert_eq!(songs(&playlist_op(SET, json!({"op": "remove", "index": 0})).0), [json!("b"), json!("c")]);
    assert_eq!(playlist_op(SET, json!({"op": "remove", "index": 3})).1, [false]);
    assert_eq!(playlist_op("title(\"x\")\n", json!({"op": "remove", "index": 0})).1, [false]);
}

#[test]
fn a_line_moves_with_its_comment() {
    let out = playlist_op(SET, json!({"op": "move", "from": 1, "to": 2})).0;
    assert_eq!(songs(&out), [json!("a"), json!("c"), json!("b")]);
    assert!(out.contains("// the second"));
    assert_eq!(playlist_op(SET, json!({"op": "move", "from": 0, "to": 5})).1, [false]);
}

#[test]
fn a_playlist_text_that_does_not_read_is_refused() {
    assert!(append("items([", "x", None).is_err());
}

#[test]
fn pins_are_read_dropping_what_is_no_version_and_written_in_name_order() {
    let pins = call("text.pins_parse", json!({"text": "\"k7-core\"(7)\ndrums(\"draft\")\nbass(\"soon\")\nkeys(0)\n"}));
    assert_eq!(pins, json!({"k7-core": 7, "drums": "draft"}));
    let text = call("text.pins_write", json!({"pins": pins}));
    assert_eq!(text, json!("drums(\"draft\")\nk7-core(7)\n"));
    assert_eq!(call("text.pins_parse", json!({"text": text})), pins);
    assert_eq!(call("text.pins_parse", json!({"text": "drums("})), json!({}));
}

const TEMPLATE: &str = "// A TURNTABLE — the deck and its strip, as one box.\nlabel(\"Turntable\")\noutputs([\n  // The channel's own fade, which is one half of the crossfader.\n  name(\"mix\").type(\"audio\").node(\"fade\"),\n])\n// The five a hand actually reaches for.\nparams({\n  Trim: { node: \"trim\", param: \"Trim\" },\n})\n";

fn level() -> Value {
    json!({"node": "level", "param": "Gain"})
}

fn promote(text: &str, name: &str) -> Result<String, String> {
    edited("text.assembly", json!({"text": text, "ops": [{"op": "setParam", "name": name, "node": "level", "param": "Gain"}]}))
        .map(|(t, _)| t)
}

fn drop_param(text: &str, name: &str) -> (String, Vec<bool>) {
    edited("text.assembly", json!({"text": text, "ops": [{"op": "dropParam", "name": name}]})).unwrap()
}

fn template_params(text: &str) -> Value {
    call("text.assembly_read", json!({"text": text}))["params"].clone()
}

#[test]
fn an_assembly_template_reads_what_it_promotes_and_places_a_finding_on_its_line() {
    assert_eq!(template_params(TEMPLATE), json!({"Trim": {"node": "trim", "param": "Trim"}}));
    let bad = call("text.assembly_read", json!({"text": "label(\"x\")\nplugs([])\n"}));
    assert_eq!(bad["params"], Value::Null);
    assert_eq!(bad["issues"][0]["line"], json!(2));
}

#[test]
fn a_promoted_param_is_added_and_nothing_else_touched() {
    let text = promote(TEMPLATE, "Level").unwrap();
    assert!(text.contains("// A TURNTABLE — the deck and its strip, as one box."));
    assert!(text.contains("// The channel's own fade, which is one half of the crossfader."));
    assert!(text.contains("// The five a hand actually reaches for."));
    assert_eq!(template_params(&text), json!({"Trim": {"node": "trim", "param": "Trim"}, "Level": level()}));
}

#[test]
fn a_name_already_promoted_is_replaced_and_a_template_with_no_block_gets_one() {
    assert_eq!(template_params(&promote(TEMPLATE, "Trim").unwrap()), json!({"Trim": level()}));
    assert_eq!(template_params(&promote("label(\"Box\")\n", "T").unwrap()), json!({"T": level()}));
}

#[test]
fn taking_a_param_back_off_leaves_the_file_as_it_was() {
    let out = drop_param(&promote(TEMPLATE, "Level").unwrap(), "Level");
    assert_eq!(out, (TEMPLATE.to_owned(), vec![true]));
}

#[test]
fn the_last_param_takes_the_empty_block_and_a_name_nothing_promoted_is_a_no_op() {
    let last = drop_param(TEMPLATE, "Trim").0;
    assert!(!last.contains("params"));
    assert_eq!(template_params(&last), Value::Null);
    assert_eq!(drop_param(TEMPLATE, "Nope"), (TEMPLATE.to_owned(), vec![false]));
}

#[test]
fn a_file_that_does_not_read_as_a_template_is_refused() {
    let e = promote("label(\"x\")\nplugs([])\n", "Level").unwrap_err();
    assert!(e.contains("assembly.k7x"), "{e}");
}

const CHAIN: &str = "// the guitar chain\nstart(true)\nsteps([\n  id(\"in\").extends(\"@k7-core/io/in\"),\n  // the drive, set by ear\n  id(\"drive\").extends(\"@k7-core/audio/overdrive\").params({ Amount: 5 }).bypass(true),\n  [id(\"verb\").extends(\"@k7-core/audio/reverb\"), id(\"echo\").extends(\"@k7-core/audio/delay\")],\n  to([chain(\"OUT\")]),\n])\n";

#[test]
fn a_chains_items_are_listed_a_parallel_steps_included_the_end_step_left_out() {
    assert_eq!(
        call("text.chain_read", json!({"text": CHAIN})),
        json!({"items": [
            {"id": "in", "extends": "@k7-core/io/in", "bypass": false},
            {"id": "drive", "extends": "@k7-core/audio/overdrive", "bypass": true},
            {"id": "verb", "extends": "@k7-core/audio/reverb", "bypass": false},
            {"id": "echo", "extends": "@k7-core/audio/delay", "bypass": false},
        ], "issues": []})
    );
}

#[test]
fn a_node_item_is_checked_as_a_module_file_at_the_items_line() {
    let got = call(
        "text.chain_read",
        json!({"text": "start(true)\nsteps([\n  id(\"in\").extends(\"@k7-core/io/in\"),\n  id(\"x\").paramz({}),\n])\n"}),
    );
    assert_eq!(got["items"], json!([]));
    assert_eq!(got["issues"][0]["line"], json!(4));
}

#[test]
fn a_chain_syntax_error_is_reported_with_its_line() {
    assert!(call("text.chain_read", json!({"text": "steps([\n"}))["issues"][0]["line"].as_u64().unwrap() >= 1);
}

fn expand(path: &str, data: Value) -> Value {
    call("text.sugar_expand", json!({"path": path, "data": data}))
}

fn apply(path: &str, text: &str, after: Value) -> String {
    edited("text.sugar_apply", json!({"path": path, "text": text, "after": after})).unwrap().0
}

#[test]
fn a_sequences_notes_and_a_graphs_runs_are_expanded_and_a_kind_with_none_is_left() {
    assert_eq!(
        expand("song/sequences/m.k7x", json!({"notes": "c4 d4 ~ e4"})),
        json!({"notes": [{"t": 0, "d": 1, "k": 60}, {"t": 1, "d": 1, "k": 62}, {"t": 3, "d": 1, "k": 64}]})
    );
    assert_eq!(
        expand(
            "song/graph.k7x",
            json!({"modules": {"MIX": ["OUT"]}, "runs": [["melody", "lead", "MIX"], ["pad", "MIX"]]})
        ),
        json!({"modules": {"MIX": ["OUT"], "melody": ["lead"], "lead": ["MIX"], "pad": ["MIX"]}})
    );
    let plain = json!({"notes": "c4"});
    assert_eq!(expand("song/project.k7x", plain.clone()), plain);
    assert_eq!(expand("song/chains/chains.k7x", plain.clone()), plain);
}

#[test]
fn a_short_form_that_does_not_read_is_named() {
    let e = refused("text.sugar_expand", json!({"path": "song/sequences/m.k7x", "data": {"notes": "c4 h4"}}));
    assert!(e.contains("notes: \"h4\" is neither a note name"), "{e}");
    let e = refused("text.sugar_expand", json!({"path": "song/sequences/m.k7x", "data": {"notes": "[c4 d4"}}));
    assert!(e.contains("\"]\" missing"), "{e}");
}

const SEQ: &str = "// the riff\nnotes(\"c4 d4 e4 f4\")\nto(\"2:1\")\n";
const SEQ_PATH: &str = "song/sequences/riff.k7x";

fn four() -> Value {
    json!([{"t": 0, "d": 1, "k": 60}, {"t": 1, "d": 1, "k": 62}, {"t": 2, "d": 1, "k": 64}, {"t": 3, "d": 1, "k": 65}])
}

#[test]
fn the_short_form_survives_an_edit_that_leaves_the_notes_alone() {
    assert_eq!(apply(SEQ_PATH, SEQ, json!({"notes": four(), "to": "3:1"})), SEQ.replace("\"2:1\"", "\"3:1\""));
    assert_eq!(apply(SEQ_PATH, SEQ, json!({"notes": four(), "to": "2:1"})), SEQ);
}

#[test]
fn the_full_note_list_is_written_once_a_note_changes() {
    let mut notes = four();
    notes[1]["v"] = json!(0.5);
    let out = apply(SEQ_PATH, SEQ, json!({"notes": notes, "to": "2:1"}));
    assert!(out.contains("// the riff"));
    assert!(!out.contains("notes(\""));
    assert_eq!(expand(SEQ_PATH, data(&out)), expand(SEQ_PATH, json!({"notes": notes, "to": "2:1"})));
}

const GRAPH: &str = "runs([[\"melody\", \"lead\", \"MIX\", \"OUT\"]])\nbypassed([])\n";
const GRAPH_PATH: &str = "song/graph.k7x";

fn modules() -> Value {
    json!({"melody": ["lead"], "lead": ["MIX"], "MIX": ["OUT"]})
}

#[test]
fn the_runs_survive_an_edit_elsewhere_in_the_graph() {
    assert_eq!(
        apply(GRAPH_PATH, GRAPH, json!({"modules": modules(), "bypassed": ["lead"]})),
        GRAPH.replace("bypassed([])", "bypassed([\"lead\"])")
    );
}

#[test]
fn the_segments_are_written_in_full_once_one_changes_and_a_removed_one_never_comes_back() {
    let mut m = modules();
    m["lead"] = json!([]);
    let out = apply(GRAPH_PATH, GRAPH, json!({"modules": m, "bypassed": []}));
    assert!(!out.contains("runs("));
    assert!(out.contains("lead"));
}
