// views_hints.rs — the completion vocabulary, after the `projectHints` cases of nodes, namespacedStep, coldModules, mixerParams.
// A reader would get wrong that a half-written file costs only itself here (decision 625): the TypeScript loses the whole answer.

use crate::support;
use crate::views_world;

use serde_json::json;
use song::lanes::inputs::load_inputs;
use song::lanes::rig::Rig;
use song::views::hints::{Hints, HintsAnswer, hints_of};
use song::{Cache, Roots};
use support::mem_world_roots;
use views_world::world;

fn answer_with(files: &[(&str, &str)], rig: &serde_json::Value, roots: &Roots) -> HintsAnswer {
    let src = world(files);
    let mut cache = Cache::new();
    let inputs = load_inputs(&src, &mut cache, roots, Rig::from_json(rig)).expect("inputs");
    hints_of(&src, &mut cache, roots, &inputs).expect("hints")
}

fn hints(files: &[(&str, &str)]) -> Hints {
    answer_with(files, &json!({}), &mem_world_roots()).hints
}

const SYNTH: &str = "plugin({ type: \"internal\", name: \"4osc\" })\nparams({ \"Filter Freq\": 100 })\n";

fn demo() -> Vec<(&'static str, &'static str)> {
    vec![
        ("project.k7x", "project({ name: \"t\", tempo: 120 })\n"),
        ("ws/_modules/instruments/synth/module.k7x", SYNTH),
        (
            "ws/_modules/instruments/fourosc/module.k7x",
            "plugin({ type: \"internal\", name: \"4osc\" })\n",
        ),
        (
            "nodes/lead.k7x",
            "extends(\"@workspace/instruments/synth\")\nparams({ \"Filter Freq\": 90 })\n",
        ),
    ]
}

#[test]
fn keeps_nodes_out_of_the_extends_vocabulary() {
    // nodes.test.ts: keeps nodes out of the `extends` vocabulary, which names MODULES (627).
    let h = hints(&demo());
    for r in ["@workspace/instruments/fourosc", "@workspace/instruments/synth"] {
        assert!(h.base_modules.iter().any(|b| b == r), "{r}");
    }
    assert!(!h.base_modules.iter().any(|b| b == "lead"));
}

#[test]
fn offers_nodes_each_linked_to_its_own_file() {
    // nodes.test.ts: offers nodes to the editor, each linked to its own file (627, 629).
    let h = hints(&demo());
    assert!(h.modules.iter().any(|m| m == "lead"));
    assert_eq!(h.paths.get("module:lead").map(String::as_str), Some("nodes/lead.k7x"));
}

#[test]
fn a_new_node_is_offered() {
    // nodes.test.ts: scaffolds a new node into nodes/, naming what it uses (627).
    let mut files = demo();
    files.push(("nodes/solo.k7x", "extends(\"@workspace/instruments/synth\")\n"));
    assert!(hints(&files).modules.iter().any(|m| m == "solo"));
}

#[test]
fn an_inherited_module_and_a_shared_sink_are_offered() {
    // namespacedStep.test.ts: is offered as a completion, alongside the project's own modules (627).
    let mut files = demo();
    files.push((
        "ws/_modules/audio/shared-delay/module.k7x",
        "plugin({ type: \"internal\", name: \"delay\" })\n",
    ));
    files.push(("ws/_modules/io/desk/module.k7x", "io(\"out\")\n"));
    let h = hints(&files);
    for m in ["@workspace/audio/shared-delay", "lead", "@workspace/io/desk"] {
        assert!(h.modules.iter().any(|x| x == m), "{m}");
    }
    assert!(h.io_sinks.iter().any(|s| s == "@workspace/io/desk"));
}

#[test]
fn a_cold_module_is_never_suggested() {
    // coldModules.test.ts: never suggests one: not in the picker, not in the chain completions (627).
    let h = hints(&[
        (
            "lib/k7-core/modules/cold/normalize/module.k7x",
            "plugin({ type: \"internal\", name: \"normalize\" })\n",
        ),
        (
            "lib/k7-core/modules/audio/utility/gain/module.k7x",
            "plugin({ type: \"internal\", name: \"gain\" })\n",
        ),
    ]);
    assert!(!h.modules.iter().any(|m| m == "@k7-core/cold/normalize"));
    assert!(!h.base_modules.iter().any(|m| m == "@k7-core/cold/normalize"));
    assert!(h.modules.iter().any(|m| m == "@k7-core/audio/utility/gain"));
}

const MIXER: &str = "@workspace/audio/mixer";

fn mixer() -> Vec<(&'static str, &'static str)> {
    vec![
        ("project.k7x", "project({ name: \"t\", tempo: 120 })\n"),
        (
            "ws/_modules/audio/mixer/module.k7x",
            "plugin({ type: \"internal\", name: \"Mixer\" })\n",
        ),
    ]
}

#[test]
fn a_builtins_params_are_known_before_a_project_authors_one() {
    // mixerParams.test.ts: lists every mixer param on a strip nobody has touched (628).
    let h = hints(&mixer());
    assert_eq!(h.params_by_base[MIXER], ["High", "Low", "Mid", "Pan", "Volume"]);
}

#[test]
fn the_ranges_ride_with_a_builtins_params() {
    // mixerParams.test.ts: carries the RANGES, which no learned name ever has (628).
    let meta = &hints(&mixer()).param_meta_by_base[MIXER];
    let r = |name: &str| (meta[name].min, meta[name].max, meta[name].def);
    assert_eq!(r("Volume"), (-60.0, 6.0, Some(0.0)));
    assert_eq!(r("Pan"), (-1.0, 1.0, Some(0.0)));
    for band in ["Low", "Mid", "High"] {
        assert_eq!(r(band), (-18.0, 18.0, Some(0.0)));
    }
}

#[test]
fn a_node_counts_as_much_as_an_extender() {
    // mixerParams.test.ts: picks up a param authored only on one strip (628).
    let mut files = mixer();
    files.push((
        "nodes/vox.k7x",
        "extends(\"@workspace/audio/mixer\")\nparams({ Drive: 3 })\n",
    ));
    let h = hints(&files);
    assert!(h.params_by_base[MIXER].iter().any(|p| p == "Drive"));
    assert!(!h.param_meta_by_base[MIXER].contains_key("Drive"));
    // A base with no builtin and no param has no entry at all.
    assert!(!h.params_by_base.contains_key("@workspace/instruments/fourosc"));
}

#[test]
fn an_answer_has_every_key_and_the_empty_one_has_the_bare_set() {
    // decision 633: `audioClips`, `videoClips`, `curves` ride with an answer and not with EMPTY_HINTS.
    let a = serde_json::to_value(hints(&[("project.k7x", "")])).expect("json");
    for k in [
        "baseModules",
        "ioSinks",
        "modules",
        "tracks",
        "channels",
        "clips",
        "audioClips",
        "videoClips",
        "videoTracks",
        "sequences",
        "chains",
        "curves",
        "moduleAssets",
        "paramsByBase",
        "paramMetaByBase",
        "racks",
        "keywords",
        "pluginNames",
        "paths",
        "projectRoot",
        "media",
    ] {
        assert!(a.get(k).is_some(), "{k}");
    }
    assert_eq!(a["keywords"], json!(["true", "false", "null"]));
    assert_eq!(a["projectRoot"], "/p/ws/proj");
    let e = serde_json::to_value(Hints::empty()).expect("json");
    for k in ["audioClips", "videoClips", "curves"] {
        assert!(e.get(k).is_none(), "{k}");
    }
    assert_eq!(
        (e["keywords"].clone(), e["projectRoot"].clone()),
        (json!([]), json!(""))
    );
}

#[test]
fn the_sinks_offer_the_pictures_pair_only_to_a_song_with_a_picture_lane() {
    // hints.ts ioSinks: the built-ins first, RENDER and VIDEO_OUT only with a picture lane (627).
    let plain = hints(&[("project.k7x", "")]);
    assert_eq!(plain.io_sinks[..3], ["MIX", "OUT", "CUE"]);
    assert!(!plain.io_sinks.iter().any(|s| s == "RENDER" || s == "VIDEO_OUT"));
    // A project's own io node is no sink: the vocabulary alone decides.
    let own = hints(&[("nodes/desk.k7x", "io(\"out\")\n")]);
    assert!(!own.io_sinks.iter().any(|s| s == "desk"));
    let pic = hints(&[("nodes/main.k7x", "extends(\"@meta/picture-track\")\nevents([])\n")]);
    assert_eq!(pic.io_sinks[..5], ["MIX", "OUT", "CUE", "RENDER", "VIDEO_OUT"]);
    assert_eq!(pic.video_tracks, ["main"]);
    assert!(!pic.tracks.iter().any(|t| t == "main"));
}

#[test]
fn a_favourite_floats_up_and_a_hidden_module_is_dropped() {
    // hints.ts modules: completionOrder off the AUTHORED module's `status` (627).
    let h = hints(&[
        (
            "ws/_modules/audio/a/module.k7x",
            "plugin({ type: \"internal\", name: \"a\" })\n",
        ),
        (
            "ws/_modules/audio/b/module.k7x",
            "plugin({ type: \"internal\", name: \"b\" })\nstatus(\"favorite\")\n",
        ),
        (
            "ws/_modules/audio/c/module.k7x",
            "plugin({ type: \"internal\", name: \"c\" })\nstatus(\"hidden\")\n",
        ),
        ("nodes/z.k7x", "extends(\"@workspace/audio/a\")\nstatus(\"hidden\")\n"),
    ]);
    let shared: Vec<&str> = h
        .modules
        .iter()
        .map(String::as_str)
        .filter(|m| m.starts_with("@workspace/audio"))
        .collect();
    assert_eq!(shared, ["@workspace/audio/b", "@workspace/audio/a"]);
    assert!(!h.modules.iter().any(|m| m == "z"));
}

#[test]
fn the_rigs_labels_the_clips_and_the_lists_are_sorted() {
    // hints.ts channels, clips, audioClips, videoClips, sequences, curves (627, 634).
    let rig =
        json!({ "channels": { "in": [{ "label": "vox", "ch": [2] }, { "label": "guitar", "ch": [1] }], "out": [] } });
    let a = answer_with(
        &[
            ("clips/kick.k7x", "source(\"audio/kick.wav\")\n"),
            ("clips/b-roll.k7x", "source(\"video/b.mp4\")\n"),
            ("clips/cap.k7x", "source(\"subs.srt\")\n"),
            ("sequences/riff.k7x", "notes([])\n"),
            ("curves/fade.k7x", "type(\"lfo\")\n"),
        ],
        &rig,
        &mem_world_roots(),
    );
    let h = a.hints;
    assert_eq!(h.channels, ["guitar", "vox"]);
    assert_eq!(h.clips, ["b-roll", "cap", "kick"]);
    assert_eq!(h.audio_clips.as_deref(), Some(&["kick".to_owned()][..]));
    assert_eq!(
        h.video_clips.as_deref(),
        Some(&["b-roll".to_owned(), "cap".to_owned()][..])
    );
    assert_eq!(h.sequences, ["riff"]);
    assert_eq!(h.curves.as_deref().map(<[String]>::len), Some(1));
    assert!(!a.capped && a.findings.is_empty() && a.spent > 0);
}

#[test]
fn a_broken_file_costs_only_itself() {
    // decision 625 (DEPARTS): the TypeScript loses the whole answer on a syntax error in `clips/`; here the file is absent.
    let h = hints(&[
        ("clips/ok.k7x", "source(\"a.wav\")\n"),
        ("clips/bad.k7x", "source(\n"),
        ("sequences/bad.k7x", "notes(\n"),
        ("curves/bad.k7x", "points(\n"),
        ("nodes/bad.k7x", "extends(\n"),
        ("nodes/fine.k7x", "extends(\"@meta/track\")\n"),
    ]);
    assert_eq!(h.clips, ["ok"]);
    assert!(h.sequences.is_empty() && h.curves.as_deref() == Some(&[][..]));
    assert!(h.modules.iter().any(|m| m == "fine") && !h.modules.iter().any(|m| m == "bad"));
    assert_eq!(h.tracks, ["fine"]);
}

#[test]
fn a_rack_reports_its_effective_symbols_and_inherits() {
    // hints.ts racks: resolved first, so a module inheriting its source or half its symbols reports the whole rack (632).
    let h = hints(&[
        (
            "ws/_modules/instruments/kit/module.k7x",
            "type(\"rack\")\nrack({ rack_type: \"slice\", source: \"break.wav\", symbols: { a: { note: 36, from: 0, to: 1 } } })\n",
        ),
        ("nodes/mine.k7x", "extends(\"@workspace/instruments/kit\")\n"),
        (
            "nodes/more.k7x",
            "extends(\"@workspace/instruments/kit\")\nrack({ rack_type: \"slice\", source: \"break.wav\", symbols: { b: { note: 37, from: 1, to: 2 } } })\n",
        ),
        (
            "nodes/empty.k7x",
            "type(\"rack\")\nrack({ rack_type: \"samples\", symbols: {} })\n",
        ),
    ]);
    let r = &h.racks["mine"];
    assert!(
        r.rack_type == "slice" && r.source.as_deref().is_some_and(|s| s.ends_with("break.wav")),
        "{r:?}"
    );
    let names = |r: &song::views::hints::RackHint| -> Vec<(String, u8)> {
        r.symbols.iter().map(|s| (s.symbol.clone(), s.note)).collect()
    };
    assert_eq!(names(r), [("a".to_owned(), 36)]);
    let more = &h.racks["more"];
    assert!(more.rack_type == "slice" && names(more).len() == 2, "{more:?}");
    assert!(!h.racks.contains_key("empty"));
}

#[test]
fn a_modules_assets_come_with_its_ancestors() {
    // hints.ts moduleAssets: its own `assets/` AND its ancestors', nearest first, without a duplicate (631).
    let h = hints(&[
        (
            "ws/_modules/audio/amp/module.k7x",
            "plugin({ type: \"internal\", name: \"nam\" })\n",
        ),
        ("ws/_modules/audio/amp/assets/clean.nam", "x"),
        ("ws/_modules/audio/amp/assets/deep/crunch.json", "x"),
        ("ws/_modules/audio/amp/assets/readme.txt", "x"),
        ("nodes/thin.k7x", "extends(\"@workspace/audio/amp\")\n"),
    ]);
    let files = h
        .module_assets
        .get("thin")
        .map(|v| v.iter().map(String::as_str).collect::<Vec<_>>());
    assert_eq!(files, Some(vec!["clean.nam", "deep/crunch.json"]));
}

#[test]
fn the_paths_link_each_kind_to_its_file() {
    // hints.ts paths: a stat says a file is there; the chains of a graph project all live in `chains.k7x` (629).
    let h = hints(&[
        ("clips/kick.k7x", "source(\"a.wav\")\n"),
        ("sequences/riff.k7x", "notes([])\n"),
        ("nodes/lead.k7x", "extends(\"@meta/track\")\n"),
        ("graph.k7x", "modules({ main: [\"OUT\"] })\n"),
        ("chains.k7x", "x\n"),
    ]);
    let at = |k: &str| h.paths.get(k).map(String::as_str);
    assert_eq!(at("clip:kick"), Some("clips/kick.k7x"));
    assert_eq!(at("sequence:riff"), Some("sequences/riff.k7x"));
    assert_eq!(at("module:lead"), Some("nodes/lead.k7x"));
    // Every chain is filed in the one `chains.k7x`, whatever the chains are.
    assert_eq!(at("chain:main"), Some("chains.k7x"));
    let bare = hints(&[("graph.k7x", "modules({ main: [\"OUT\"] })\n")]);
    assert!(!bare.paths.contains_key("chain:main") && bare.chains == ["main"]);
}

#[test]
fn the_media_are_grouped_by_folder_and_skip_the_dot_names() {
    // hints.ts media: audio by extension, by project-relative folder, "" the root (630).
    let h = hints(&[
        ("kick.WAV", "x"),
        ("notes.txt", "x"),
        ("audio/a.flac", "x"),
        ("audio/deep/b.m4a", "x"),
        ("audio/.hidden.wav", "x"),
        (".git/c.wav", "x"),
        ("audio/.wav", "x"),
    ]);
    let m: Vec<(&str, Vec<&str>)> = h
        .media
        .iter()
        .map(|(k, v)| (k.as_str(), v.iter().map(String::as_str).collect()))
        .collect();
    let mut sorted = m.clone();
    sorted.sort();
    assert_eq!(m, sorted);
    assert_eq!(
        m,
        vec![
            ("", vec!["kick.WAV"]),
            ("audio", vec!["a.flac"]),
            ("audio/deep", vec!["b.m4a"])
        ]
    );
}

#[test]
fn the_scanned_plugin_names_and_the_project_root_are_filled() {
    // decision 633: the route fills them in the TypeScript; the answer carries them here.
    let roots = Roots {
        scan: Some("/p/home/exts.json".into()),
        ..mem_world_roots()
    };
    let scan = "{\"entries\":[{\"plugins\":[{\"name\":\"Serum\",\"format\":\"VST3\"},{\"name\":\"Pro-Q\",\"format\":\"AU\"}]}]}";
    let h = answer_with(&[("home/exts.json", scan)], &json!({}), &roots).hints;
    assert_eq!(h.plugin_names["vst3"], ["Serum"]);
    assert_eq!(h.plugin_names["au"], ["Pro-Q"]);
    assert_eq!(h.project_root, "/p/ws/proj");
}

#[test]
fn a_chain_project_lists_its_derived_chains() {
    // decision 626: `chains` is derived again with no lane predicate, the keys sorted.
    let h = hints(&[(
        "chains/main.k7x",
        "start(true)\nsteps([id(\"n1\").extends(\"@meta/track\"), to([chain(\"OUT\")])])\n",
    )]);
    assert!(!h.chains.is_empty(), "{:?}", h.chains);
    let mut sorted = h.chains.clone();
    sorted.sort();
    assert_eq!(h.chains, sorted);
}
