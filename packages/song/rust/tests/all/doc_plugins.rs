// doc_plugins.rs — a step's PluginDoc (decisions 312 to 322): the kinds, the files, the stage params, the racks, the sidechain.
// Ported from nodes.test.ts, bare-module.test.ts, missingModule.test.ts, unknownPluginType.test.ts, surgeModule.test.ts, mixCompile.test.ts.

use crate::doc_support;

use doc_support::{AMP, compiled, lane, project, stopped};
use song::doc::{PluginDoc, ProjectDoc};

const EQ: &str = "plugin({ type: \"vst3\", name: \"Pro-Q 3\" })\nmix(0.5)\nparams({ gain: 3, name: \"hello\", sfz: \"a.sfz\", model: \"@k7-core/m.nam\", ir: \"@k7-core/i.wav\" })\n";
const SYN: &str = "plugin({ type: \"vst3\", name: \"Surge\" })\n";

/// A library module of `ty` named `name`.
fn lib(ty: &str, name: &str, body: &str) -> (String, String) {
    (format!("lib/k7-core/modules/{ty}/{name}/module.k7x"), body.to_owned())
}

fn doc_with(modules: &str, nodes: &[(&str, &str)], libs: &[(&str, &str, &str)], more: &[(&str, &str)]) -> ProjectDoc {
    let made: Vec<(String, String)> = libs.iter().map(|(t, n, b)| lib(t, n, b)).collect();
    let mut extra: Vec<(&str, &str)> = nodes.to_vec();
    extra.extend(made.iter().map(|(a, b)| (a.as_str(), b.as_str())));
    extra.extend_from_slice(more);
    let c = project(modules, &extra);
    match c.engine {
        Ok(d) => d,
        Err(f) => panic!("{:?}", f.diagnostics),
    }
}

fn plugin<'a>(d: &'a ProjectDoc, track: &str, at: usize) -> &'a PluginDoc {
    &d.tracks.iter().find(|t| t.id == track).expect("a track").plugins[at]
}

// 315: the plugin's fields, its files as resolved, and the stage params after the numeric ones.
#[test]
fn a_vst_carries_its_files_its_text_params_stay_out_and_the_stage_follows() {
    let d = doc_with(
        "gtr: [\"v\"], v: [\"MIX\"]",
        &[
            ("nodes/v.k7x", "extends(\"@k7-core/audio/eq\")\nbase(\"x\")\n"),
            ("lib/k7-core/models/m.nam", "n"),
            ("lib/k7-core/irs/i.wav", "w"),
        ],
        &[("audio", "eq", EQ)],
        &[("lib/k7-core/modules/audio/eq/base/x.vstpreset", "b")],
    );
    let p = plugin(&d, "gtr", 0);
    assert_eq!(
        (p.id.as_str(), p.plugin_type.as_str(), p.name.as_str()),
        ("gtr:v", "vst3", "Pro-Q 3")
    );
    assert!(p.path.ends_with("/eq/base/x.vstpreset"));
    assert_eq!(p.sfz_file.as_deref(), Some("a.sfz"));
    assert!(
        p.nam_file.as_deref().is_some_and(|f| f.ends_with("m.nam"))
            && p.ir_file.as_deref().is_some_and(|f| f.ends_with("i.wav"))
    );
    assert_eq!(p.role, "");
    assert!(p.enabled && p.preset_save.ends_with("/nodes/v.vstpreset"));
    let keys: Vec<&str> = p.params.keys().map(String::as_str).collect();
    assert_eq!(keys, ["gain", "@mix", "@input-volume", "@output-volume"]);
    assert_eq!(p.params["@mix"], 0.5);
}

// 315, surgeModule.test.ts: a `.fxp` leaves as `surgePatch` and its `path` is empty.
#[test]
fn a_fxp_base_is_a_surge_patch_not_a_path() {
    let d = doc_with(
        "gtr: [\"s\"], s: [\"MIX\"]",
        &[("nodes/s.k7x", "extends(\"@k7-core/instruments/syn\")\nbase(\"s\")\n")],
        &[("instruments", "syn", SYN)],
        &[("lib/k7-core/modules/instruments/syn/base/s.fxp", "b")],
    );
    let p = plugin(&d, "gtr", 0);
    assert_eq!(p.path, "");
    assert!(p.surge_patch.as_deref().is_some_and(|f| f.ends_with("s.fxp")));
    // An instrument takes an output stage and no blend.
    assert_eq!(
        p.params.keys().map(String::as_str).collect::<Vec<_>>(),
        ["@output-volume"]
    );
}

// 315, mixCompile.test.ts: an effect that owns its blend always sends `@mix` (1 when unstated); an instrument never does.
#[test]
fn an_effect_sends_its_blend_and_both_levels_defaulted() {
    let d = doc_with(
        "gtr: [\"e\"], e: [\"MIX\"]",
        &[("nodes/e.k7x", "extends(\"@k7-core/audio/fx\")\ninput_volume(-3)\n")],
        &[("audio", "fx", AMP)],
        &[],
    );
    let p = plugin(&d, "gtr", 0);
    assert_eq!(
        (p.params["@mix"], p.params["@input-volume"], p.params["@output-volume"]),
        (1.0, -3.0, 0.0)
    );
}

// 314, mixCompile.test.ts "refuses a module that states the engine's own blend": a Failure on the node's file, all of them.
#[test]
fn a_param_the_engines_stage_holds_is_refused_naming_its_node() {
    let c = project(
        "gtr: [\"e\"], e: [\"MIX\"], drums: [\"f\"], f: [\"MIX\"]",
        &[
            ("nodes/e.k7x", "extends(\"@k7-core/audio/fx\")\nparams({ Wet: 1 })\n"),
            (
                "nodes/f.k7x",
                "extends(\"@k7-core/audio/fx\")\nparams({ \"Output Gain\": 1 })\n",
            ),
            ("lib/k7-core/modules/audio/fx/module.k7x", AMP),
        ],
    );
    assert_eq!(stopped(&c), ["nodes/e.k7x", "nodes/f.k7x"]);
}

// 312, bare-module.test.ts, unknownPluginType.test.ts: what is not a plugin of the engine is skipped, not refused.
#[test]
fn offline_unknown_and_io_steps_are_skipped() {
    let d = doc_with(
        "gtr: [\"a\"], a: [\"b\"], b: [\"c\"], c: [\"d\"], d: [\"MIX\"]",
        &[
            ("nodes/a.k7x", "plugin({ type: \"offline\", name: \"Render\" })\n"),
            ("nodes/b.k7x", "plugin({ type: \"mystery\", name: \"X\" })\n"),
            ("nodes/c.k7x", "io(\"out\")\n"),
            ("nodes/d.k7x", AMP),
        ],
        &[],
        &[],
    );
    let t = d.tracks.iter().find(|t| t.id == "gtr").expect("gtr");
    assert_eq!(t.plugins.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), ["gtr:d"]);
}

// 312, missingModule.test.ts: a step naming no module is skipped; the reference check reports it elsewhere.
#[test]
fn a_bypassed_step_is_disabled_and_keeps_its_place() {
    let c = project(
        "gtr: [\"amp\"], amp: [\"fx\"], fx: [\"MIX\"]",
        &[(
            "graph.k7x",
            "modules({ gtr: [\"amp\"], amp: [\"fx\"], fx: [\"MIX\"] })\nbypassed([\"amp\"])\n",
        )],
    );
    let d = c.engine.expect("a document");
    let t = d.tracks.iter().find(|t| t.id == "gtr").expect("gtr");
    assert_eq!(t.plugins.iter().map(|p| p.enabled).collect::<Vec<_>>(), [false, true]);
}

// 316, nodes.test.ts: a rack's samples and its slice window; a bad window is length 0; a muted tap is not given to the sampler.
#[test]
fn a_rack_becomes_a_sampler_with_its_sounds() {
    let samples = "type(\"rack\")\nrack({ rack_type: \"samples\", symbols: { k: { note: 36, clip: \"/p/ws/proj/k.wav\" }, m: { note: 37, clip: \"/p/ws/proj/k.wav\", mute: true } } })\n";
    let slice = "type(\"rack\")\nrack({ rack_type: \"slice\", source: \"/p/ws/proj/k.wav\", symbols: { a: { note: 36, from: 1, to: 3 }, b: { note: 37, from: 3, to: 1 } } })\n";
    let d = doc_with(
        "gtr: [\"r\"], r: [\"MIX\"], drums: [\"s\"], s: [\"MIX\"]",
        &[("nodes/r.k7x", samples), ("nodes/s.k7x", slice), ("k.wav", "x")],
        &[],
        &[],
    );
    let p = plugin(&d, "gtr", 0);
    assert_eq!(
        (p.plugin_type.as_str(), p.role.as_str(), p.name.as_str()),
        ("sampler", "instrument", "r")
    );
    let rack = p.rack.as_ref().expect("a rack");
    assert_eq!(rack.len(), 1);
    assert_eq!(
        (rack[0].symbol.as_str(), rack[0].note, rack[0].start, rack[0].length),
        ("k", 36, 0.0, 0.0)
    );
    let s = plugin(&d, "drums", 0).rack.as_ref().expect("a rack");
    assert_eq!((s[0].start, s[0].length, s[1].start, s[1].length), (1.0, 2.0, 3.0, 0.0));
}

// 320, tapOuts.test.ts: a kit's routed taps leave by pairs of their own, and the run each feeds is a track fed from the kit's plugin.
#[test]
fn a_routed_tap_leaves_by_a_pair_and_feeds_a_track() {
    let kit = "plugin({ type: \"internal\", name: \"Sampler\" })\nkit({ Snare: 38, Clap: 39 })\ntap_outs({ Snare: \"snare-fx\", Clap: \"clap-fx\" })\n";
    let c = project(
        "gtr: [\"kit\"], kit: [\"MIX\"], \"snare-fx\": [\"OUT\"], \"clap-fx\": [\"OUT\"]",
        &[
            ("nodes/kit.k7x", kit),
            ("nodes/snare-fx.k7x", AMP),
            ("nodes/clap-fx.k7x", AMP),
            (
                "graph.k7x",
                "modules({ gtr: [\"kit\"], kit: [\"MIX\"], \"snare-fx\": [\"OUT\"], \"clap-fx\": [\"OUT\"] })\ntap_feeds([{ node: \"kit\", tap: \"Snare\", to: \"snare-fx\" }, { node: \"kit\", tap: \"Clap\", to: \"clap-fx\" }])\n",
            ),
        ],
    );
    let d = c.engine.expect("a document");
    let outs = plugin(&d, "gtr", 0).tap_outs.as_ref().expect("tap outs");
    assert_eq!(
        outs.iter().map(|o| (o.note, o.pair)).collect::<Vec<_>>(),
        [(39, 1), (38, 2)]
    );
    let snare = d.tracks.iter().find(|t| t.id == "snare-fx").expect("the tap's run");
    assert_eq!(snare.role, "effect");
    let from = snare.input_from.as_ref().expect("fed from the kit");
    assert_eq!((from.plugin.as_str(), from.pair), ("gtr:kit", 2));
}

// 320: a tap the kit does not have is a Failure on the node, one error per tap.
#[test]
fn a_tap_with_no_note_is_a_failure_on_its_node() {
    let kit = "plugin({ type: \"internal\", name: \"Sampler\" })\nkit({ Snare: 38 })\ntap_outs({ Snare: \"snare-fx\", Clap: \"clap-fx\" })\n";
    let c = project(
        "gtr: [\"kit\"], kit: [\"MIX\"], \"snare-fx\": [\"OUT\"], \"clap-fx\": [\"OUT\"]",
        &[
            ("nodes/kit.k7x", kit),
            ("nodes/snare-fx.k7x", AMP),
            ("nodes/clap-fx.k7x", AMP),
            (
                "graph.k7x",
                "modules({ gtr: [\"kit\"], kit: [\"MIX\"], \"snare-fx\": [\"OUT\"], \"clap-fx\": [\"OUT\"] })\ntap_feeds([{ node: \"kit\", tap: \"Snare\", to: \"snare-fx\" }, { node: \"kit\", tap: \"Clap\", to: \"clap-fx\" }])\n",
            ),
        ],
    );
    assert_eq!(stopped(&c), ["nodes/kit.k7x"]);
}

// 319, ports.test.ts "carries the side input": the first side input naming a track is `sidechainFrom`.
#[test]
fn a_side_input_names_the_track_it_comes_from() {
    let comp = "plugin({ type: \"internal\", name: \"Compressor\" })\ninputs([{ name: \"in\", type: \"audio\" }, { name: \"side\", type: \"audio\" }])\n";
    let c = project(
        "gtr: [\"comp\"], comp: [\"MIX\"], drums: [\"MIX\"], \"drums\": [\"MIX\"]",
        &[
            ("nodes/comp.k7x", comp),
            (
                "graph.k7x",
                "modules({ gtr: [\"comp\"], comp: [\"MIX\"], drums: [\"comp#side\"] })\n",
            ),
        ],
    );
    let d = c.engine.expect("a document");
    assert_eq!(plugin(&d, "gtr", 0).sidechain_from.as_deref(), Some("drums"));
}

// 321: a run's sends are its `out` into a bus, level its gain.
#[test]
fn a_run_into_a_bus_sends_at_its_level() {
    let c = project(
        "gtr: [\"amp\"], amp: [\"fx\"], fx: [\"MIX\"], drums: [\"verb\"], verb: [\"fx\"]",
        &[],
    );
    let d = c.engine.expect("a document");
    for lane in ["gtr", "drums"] {
        let t = d.tracks.iter().find(|t| t.id == lane).expect("a lane");
        assert_eq!(
            t.sends
                .iter()
                .map(|s| (s.to.as_str(), s.level, s.enabled))
                .collect::<Vec<_>>(),
            [("fx", 1.0, true)]
        );
    }
    assert!(d.tracks.iter().any(|t| t.id == "fx" && t.role == "effect"));
}

#[test]
fn a_project_that_compiles_has_no_error_diagnostics() {
    let c = compiled(&[
        ("nodes/gtr.k7x", &lane("")),
        ("graph.k7x", "modules({ gtr: [\"MIX\"] })\n"),
    ]);
    assert!(c.engine.is_ok());
    assert!(
        c.diagnostics.iter().all(|f| f.severity != song::files::Severity::Error),
        "{:?}",
        c.diagnostics
    );
}
