// schema_tests.rs — the schema.* ops through the bytes the ABI carries: the cases of the TypeScript functions they replaced.
// A reader would get wrong that these are the TypeScript tests moved, not new rules: each block names the function it held to.

use crate::Session;
use crate::ops::{ERROR, OK};
use serde_json::{Value, json};

fn ask(op: &str, args: Value) -> Value {
    let mut s = Session::new();
    let mut out = Vec::new();
    let status = s.handle(
        json!({ "op": format!("schema.{op}"), "args": args })
            .to_string()
            .as_bytes(),
        &mut out,
    );
    assert_eq!(status, OK, "{op}: {}", String::from_utf8_lossy(&out));
    serde_json::from_slice(&out).unwrap()
}

fn truth(op: &str, key: &str, value: &str) -> bool {
    ask(op, json!({ key: value })).as_bool().unwrap()
}

#[test]
fn an_engine_ref_is_the_namespace_and_what_is_under_it() {
    for r in ["@engine", "@engine/neural-amp-modeler", "@engine/audio/delay/adt"] {
        assert!(truth("is_engine_ref", "ref", r), "{r}");
    }
    for r in [
        "@engineering/audio/eq",
        "@engines/x",
        "@k7-core/audio/amp/nam",
        "@workspace/io/desk",
        "@project/instruments/bass",
        "instruments/bass",
        "",
    ] {
        assert!(!truth("is_engine_ref", "ref", r), "{r}");
    }
}

#[test]
fn a_status_is_listed_unless_it_says_hidden_or_favorite() {
    let of = |s: Value| ask("module_status_of", json!({ "status": s }));
    assert_eq!(of(Value::Null), "listed");
    assert_eq!(of(json!("listed")), "listed");
    assert_eq!(of(json!("hidden")), "hidden");
    assert_eq!(of(json!("favorite")), "favorite");
    assert_eq!(of(json!("FAVORITE")), "listed");
}

#[test]
fn a_document_extension_is_k7x_in_any_case() {
    for n in ["a.k7x", "dir/b.K7x", "C.K7X"] {
        assert!(truth("is_doc_file", "name", n), "{n}");
    }
    for n in ["a.yaml", "a.yml", "a.json", "a.k7x.bak", "k7x", "a.k7"] {
        assert!(!truth("is_doc_file", "name", n), "{n}");
    }
    let strip = |n: &str| ask("strip_doc_ext", json!({ "name": n }));
    assert_eq!(strip("intro.k7x"), "intro");
    assert_eq!(strip("a/b.K7X"), "a/b");
    assert_eq!(strip("intro.yaml"), "intro.yaml");
    assert_eq!(strip("x.k7x.k7x"), "x.k7x");
}

#[test]
fn audio_is_judged_by_the_extension() {
    for n in ["a.wav", "B.FLAC", "x/y.m4a", "z.webm"] {
        assert!(truth("is_audio", "name", n), "{n}");
    }
    for n in ["a.mp4", ".wav", "wav", "a.txt"] {
        assert!(!truth("is_audio", "name", n), "{n}");
    }
}

#[test]
fn a_placement_id_names_its_material() {
    let of = |id: &str| ask("material_ref_of", json!({ "id": id }));
    assert_eq!(of("riff@16.5"), "riff");
    assert_eq!(of("riff@20#2"), "riff");
    assert_eq!(of("verse@8>kick@0#3"), "kick");
    assert_eq!(of("a#2"), "a#2");
    assert_eq!(of("a@b#2"), "a@b#2");
}

#[test]
fn a_timeline_lane_address_splits_on_the_first_arrow() {
    let of = |id: &str| ask("timeline_lane_of", json!({ "id": id }));
    assert_eq!(of("verse>kick"), json!({ "timeline": "verse", "lane": "kick" }));
    assert_eq!(of("kick"), Value::Null);
    assert_eq!(of(">kick"), Value::Null);
    assert_eq!(of("verse>"), Value::Null);
}

#[test]
fn tasks_read_their_state_and_their_anchor() {
    let file = "# Tasks\n\nNotes for the band.\n\n- [ ] fix the snare `05.paradoxe.k7 @ 49:1`\n- [x] bounce the bass\n* [ ] call Lou\n";
    assert_eq!(
        ask("parse_tasks", json!({ "text": file })),
        json!([
            { "line": 4, "text": "fix the snare", "done": false, "at": { "project": "05.paradoxe.k7", "timecode": "49:1" } },
            { "line": 5, "text": "bounce the bass", "done": true },
            { "line": 6, "text": "call Lou", "done": false },
        ])
    );
}

#[test]
fn an_instance_is_named_after_its_lane_and_always_numbered() {
    let name = |what: &str, owner: Value, taken: Value| {
        ask("instance_name", json!({ "what": what, "owner": owner, "taken": taken }))
    };
    assert_eq!(name("reverb", json!("vo.jo"), json!([])), "vo_jo_reverb_1");
    assert_eq!(name("4osc", Value::Null, json!([])), "4osc_1");
    assert_eq!(name("4osc", json!(""), json!([])), "4osc_1");
    assert_eq!(
        name("reverb", json!("vo"), json!(["vo_reverb_1", "vo_reverb_3"])),
        "vo_reverb_2"
    );
    assert_eq!(name("eq", Value::Null, json!(["eq_1", "eq_2", "eq_3"])), "eq_4");
    assert_eq!(name("tape delay", json!("bs.lead"), json!([])), "bs_lead_tape_delay_1");
    assert_eq!(name("...", json!(""), json!([])), "x_1");
    assert_eq!(name(".lead.", Value::Null, json!([])), "lead_1");
    assert_eq!(name("--x--", Value::Null, json!([])), "--x--_1");
}

#[test]
fn a_key_list_holds_before_and_after_and_eases_by_the_key_it_leaves() {
    let keys = json!([
        { "t": 1, "v": 0, "ease": "linear" },
        { "t": 3, "v": 100, "ease": "hold" },
        { "t": 5, "v": 200, "ease": "linear" },
    ]);
    let at = |t: f64| ask("value_at", json!({ "keys": keys, "t": t })).as_f64().unwrap();
    assert_eq!(at(0.0), 0.0);
    assert_eq!(at(9.0), 200.0);
    assert_eq!(at(2.0), 50.0);
    assert_eq!(at(4.9), 100.0);
    assert_eq!(at(5.0), 200.0);
    assert_eq!(ask("value_at", json!({ "keys": [], "t": 1 })).as_f64(), Some(0.0));
    let eased = |ease: &str, t: f64| {
        let k = json!([{ "t": 0, "v": 0, "ease": ease }, { "t": 1, "v": 1, "ease": "linear" }]);
        ask("value_at", json!({ "keys": k, "t": t })).as_f64().unwrap()
    };
    assert_eq!(eased("in", 0.5), 0.25);
    assert_eq!(eased("out", 0.5), 0.75);
    assert_eq!(eased("inout", 0.25), 0.125);
    assert_eq!(eased("inout", 0.75), 0.875);
    assert_eq!(eased("hold", 0.99), 0.0);
}

#[test]
fn a_playlist_reads_its_title_and_its_lines_in_order() {
    let text = [
        r#"title("Été 2026")"#,
        "items([",
        r#"  "AC-DC/Back in Black/Hells Bells","#,
        r#"  song("Nirvana/Nevermind/Lithium").variant("instrumental"),"#,
        "])",
    ]
    .join("\n");
    assert_eq!(
        ask("parse_playlist", json!({ "text": text })),
        json!({ "title": "Été 2026", "items": [
            { "song": "AC-DC/Back in Black/Hells Bells" },
            { "song": "Nirvana/Nevermind/Lithium", "variant": "instrumental" },
        ]})
    );
    let drops = ask(
        "parse_playlist",
        json!({ "text": "items([[\"x\"], { variant: \"sans-voix\" }, \"Lithium\"])\n" }),
    );
    assert_eq!(drops["items"], json!([{ "song": "Lithium" }]));
    assert_eq!(ask("parse_playlist", json!({ "text": "items([\n" })), Value::Null);
    assert_eq!(
        ask("parse_playlist", json!({ "text": "title(\"Set\")\n" })),
        json!({ "title": "Set", "items": [] })
    );
    assert_eq!(ask("parse_playlist", json!({ "text": "" })), json!({ "items": [] }));
}

#[test]
fn a_playlist_name_is_a_path_under_its_folder_or_nothing() {
    let of = |n: &str| ask("playlist_path_of", json!({ "name": n }));
    assert_eq!(of("ete-2026"), "_playlists/ete-2026.k7x");
    assert_eq!(of("live/repet"), "_playlists/live/repet.k7x");
    assert_eq!(of("set.k7x"), "_playlists/set.k7x");
    assert_eq!(of("  ete-2026  "), "_playlists/ete-2026.k7x");
    assert_eq!(of("/live/repet/"), "_playlists/live/repet.k7x");
    for n in ["../secrets", "live/../../out", "", "   ", "a//b"] {
        assert_eq!(of(n), Value::Null, "{n}");
    }
}

#[test]
fn a_span_laid_over_a_comp_takes_what_it_covers_and_writes_nothing_for_the_base() {
    let comp = json!([{ "take": "t2", "from": 16, "to": 24 }]);
    let laid = |take: &str, from: u32, to: u32| {
        ask(
            "set_comp_span",
            json!({ "comp": comp, "span": { "take": take, "from": from, "to": to }, "baseRef": "gtr" }),
        )
    };
    assert_eq!(
        laid("t3", 18, 20),
        json!([
            { "take": "t2", "from": 16.0, "to": 18.0 },
            { "take": "t3", "from": 18.0, "to": 20.0 },
            { "take": "t2", "from": 20.0, "to": 24.0 },
        ])
    );
    assert_eq!(laid("gtr", 16, 20), json!([{ "take": "t2", "from": 20.0, "to": 24.0 }]));
}

#[test]
fn a_stretch_map_bends_between_markers_and_continues_past_the_ends() {
    let bent = json!([[0, 0], [4, 1], [8, 4]]);
    let beats = |secs: Value| ask("beats_at_sources", json!({ "markers": bent, "secs": secs }));
    let close = |v: &Value, want: &[f64]| {
        let got: Vec<f64> = v.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
        assert!(
            got.iter().zip(want).all(|(a, b)| (a - b).abs() < 1e-9),
            "{got:?} vs {want:?}"
        );
    };
    close(&beats(json!([0, 1, 4])), &[0.0, 4.0, 8.0]);
    close(&beats(json!([0.5, 2.5])), &[2.0, 6.0]);
    close(&beats(json!([5, -1])), &[8.0 + 4.0 / 3.0, -4.0]);
    let secs = [-2.0, 0.0, 0.25, 1.0, 2.5, 3.75, 4.0, 6.0];
    let there = beats(json!(secs));
    let back = ask("sources_at_beats", json!({ "markers": bent, "beats": there }));
    close(&back, &secs);
    let identity = ask("beats_at_sources", json!({ "markers": [[0, 0]], "secs": [3] }));
    assert_eq!(identity, json!([3.0]));
    let flat = ask(
        "beats_at_sources",
        json!({ "markers": [[0, 0], [8, 4]], "secs": [0, 1, 2.5, 4] }),
    );
    close(&flat, &[0.0, 2.0, 5.0, 8.0]);
    let mut last = f64::NEG_INFINITY;
    for i in 0..=32 {
        let s = ask(
            "sources_at_beats",
            json!({ "markers": bent, "beats": [f64::from(i) / 4.0] }),
        )[0]
        .as_f64()
        .unwrap();
        assert!(s >= last);
        last = s;
    }
}

#[test]
fn a_built_in_is_found_by_any_spelling_under_the_engine_namespace_alone() {
    let by = |r: &str| ask("builtin_for_ref", json!({ "ref": r }));
    let four = by("@engine/4osc");
    assert_eq!(four["name"], "4OSC");
    assert_eq!(four["role"], "instrument");
    assert_eq!(by("@engine/ 4OSC "), four);
    assert_eq!(by("@engine/nope"), Value::Null);
    assert_eq!(by("@k7-core/4osc"), Value::Null);
    let param = four["params"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "Voices 1")
        .unwrap();
    assert_eq!(param["automatable"], false);
}

#[test]
fn a_scanned_plugin_becomes_a_module_by_its_slug_and_its_format() {
    let of = |n: &str, f: &str| ask("engine_module_of", json!({ "name": n, "format": f }));
    assert_eq!(
        of("Neural Amp Modeler", "VST3"),
        json!({ "ref": "@engine/neural-amp-modeler", "plugin": { "type": "vst3", "name": "Neural Amp Modeler" } })
    );
    assert_eq!(of("...", "VST3"), Value::Null);
}

#[test]
fn a_module_emits_by_its_folder_its_io_role_or_its_ports() {
    let emits = |c: Value, io: Value, outputs: Value| {
        ask("module_emits", json!({ "category": c, "io": io, "outputs": outputs }))
    };
    assert_eq!(emits(json!("instruments"), Value::Null, json!([])), "audio");
    assert_eq!(emits(json!("transformers"), Value::Null, json!([])), "midi");
    assert_eq!(emits(json!("shaders"), Value::Null, json!([])), "video");
    assert_eq!(emits(json!("io"), json!("camera"), json!([])), "video");
    assert_eq!(emits(json!("io"), json!("midi"), json!([])), "midi");
    assert_eq!(emits(json!("io"), json!("in"), json!([])), "audio");
    assert_eq!(emits(json!("io"), json!("out"), json!([])), Value::Null);
    let lane = |kinds: &[&str]| {
        emits(
            json!("tracks"),
            Value::Null,
            json!(kinds.iter().map(|k| json!({ "type": k })).collect::<Vec<_>>()),
        )
    };
    assert_eq!(lane(&["audio"]), "audio");
    assert_eq!(lane(&["audio", "midi"]), Value::Null);
    assert_eq!(emits(Value::Null, Value::Null, json!([])), Value::Null);
}

#[test]
fn the_rest_of_the_module_rules_answer_as_the_typescript_did() {
    assert!(truth("is_library_name", "name", "k7-core"));
    assert!(!truth("is_library_name", "name", "account"));
    assert!(!truth("is_library_name", "name", "Engine"));
    assert!(
        ask(
            "is_library_file_of",
            json!({ "family": "samples", "path": "A/kick.WAV" })
        )
        .as_bool()
        .unwrap()
    );
    assert!(
        !ask("is_library_file_of", json!({ "family": "models", "path": "a.wav" }))
            .as_bool()
            .unwrap()
    );
    assert!(
        ask("is_picture_pass", json!({ "category": "shaders" }))
            .as_bool()
            .unwrap()
    );
    assert!(
        ask("is_picture_pass", json!({ "category": "video_effects" }))
            .as_bool()
            .unwrap()
    );
    assert!(
        !ask("is_picture_pass", json!({ "category": "audio" }))
            .as_bool()
            .unwrap()
    );
    assert!(!ask("is_picture_pass", json!({})).as_bool().unwrap());
    let level = |n: &str| ask("own_level_side", json!({ "name": n }));
    assert_eq!(level("Output"), "output");
    assert_eq!(level("Input Gain"), "input");
    assert_eq!(level("out level"), "output");
    assert_eq!(level("Output Mix"), Value::Null);
    let mix = |n: &str| ask("own_mix_side", json!({ "name": n }));
    assert_eq!(mix("Dry/Wet"), "wet");
    assert_eq!(mix("Dry"), "dry");
    assert_eq!(mix("@mix"), Value::Null);
}

#[test]
fn an_unknown_schema_op_is_a_bad_request() {
    let mut s = Session::new();
    let mut out = Vec::new();
    let status = s.handle(br#"{"op":"schema.nope","args":{}}"#, &mut out);
    assert_eq!(status, ERROR);
    assert_eq!(serde_json::from_slice::<Value>(&out).unwrap()["code"], "bad_request");
}

// builtinParams.ts' `builtinPluginFor`, placementId.ts' `placementStartOf` and presets.ts' PRESET_NAME.
#[test]
fn a_module_plugin_is_a_builtin_only_when_it_is_internal_and_named_by_any_spelling() {
    let by = |kind: Value, name: &str| ask("builtin_for_plugin", json!({ "type": kind, "name": name }));
    assert!(
        by(json!("internal"), "4OSC")["params"]
            .as_array()
            .is_some_and(|p| !p.is_empty())
    );
    assert_eq!(by(json!("internal"), "4osc"), by(json!("internal"), "4OSC"));
    assert!(by(json!("vst3"), "4OSC").is_null());
    assert!(by(json!(null), "4OSC").is_null());
    assert!(by(json!("internal"), "No Such Plugin").is_null());
}

#[test]
fn a_placement_id_names_its_start_and_a_preset_name_what_every_filesystem_agrees_on() {
    let start = |id: &str| ask("placement_start_of", json!({ "id": id }));
    assert_eq!(start("riff@16.5"), json!(16.5));
    assert_eq!(start("verse@8>kick@0#3"), json!(0.0));
    assert!(start("riff").is_null());
    assert!(truth("is_preset_name", "name", "Warm pad 2"));
    for bad in ["", "-lead", "a/b", &"a".repeat(65)] {
        assert!(!truth("is_preset_name", "name", bad), "{bad:?}");
    }
}
