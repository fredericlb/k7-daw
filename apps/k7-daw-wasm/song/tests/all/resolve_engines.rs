// resolve_engines.rs — a module run as its engine: engines.test.ts and decision 59.
// A reader would get wrong that the engine is folded with its own `extends` and the module keeps its identity, KEPT.

use crate::support;

use song::modules::ParamValue;
use song::resolve::{current_engine, engine_names, engine_params};
use support::mem_world;

const LIB: &str = "lib/k7-core/modules/instruments/synth";

fn files(extra: &[(&'static str, &'static str)]) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = [
        (
            "/module.k7x",
            "plugin({ type: \"internal\", name: \"Synth\" })\nparams({ base: 1 })\n",
        ),
        (
            "/engines/tracktion/module.k7x",
            "plugin({ type: \"internal\", name: \"TrackSynth\" })\nparams({ e: 1 })\n",
        ),
        (
            "/engines/tracktion/ui.k7x",
            "cols(1)\nrows(1)\nwidgets({ k: { type: \"knob\", col: 0, row: 0 } })\n",
        ),
        (
            "/engines/sampler/module.k7x",
            "plugin({ type: \"internal\", name: \"Sampler\" })\nparams({ s: 2, e: 7 })\n",
        ),
    ]
    .iter()
    .map(|(p, t)| (format!("{LIB}{p}"), (*t).to_owned()))
    .collect();
    out.extend(extra.iter().map(|(p, t)| ((*p).to_owned(), (*t).to_owned())));
    out
}

fn node(text: &str, extra: &[(&'static str, &'static str)]) -> song::Resolved {
    let mut all = files(extra);
    all.push((
        "ws/proj/nodes/lead.k7x".to_owned(),
        format!("extends(\"@k7-core/instruments/synth\")\n{text}"),
    ));
    let borrowed: Vec<(&str, &str)> = all.iter().map(|(p, t)| (p.as_str(), t.as_str())).collect();
    let mut w = mem_world(&borrowed);
    w.resolved.modules.shift_remove("lead").expect("the node resolved")
}

fn plugin_name(m: &song::Resolved) -> Option<&str> {
    m.authored.plugin.as_ref().and_then(|p| p.name.as_deref())
}

fn num(m: &song::Resolved, k: &str) -> Option<f64> {
    match m.authored.params.as_ref()?.get(k)? {
        ParamValue::Number(n) => Some(*n),
        ParamValue::Text(_) => None,
    }
}

// engines.test.ts, "loading": each engine under its module's ref, listed on the module, default first; never a module of its own.
#[test]
fn engines_load_under_their_modules_ref_and_list_default_first() {
    let all = files(&[]);
    let borrowed: Vec<(&str, &str)> = all.iter().map(|(p, t)| (p.as_str(), t.as_str())).collect();
    let w = mem_world(&borrowed);
    let synth = &w.vocab.modules["@k7-core/instruments/synth"];
    assert_eq!(engine_names(synth), ["tracktion", "sampler"]);
    assert_eq!(current_engine(synth).as_deref(), Some("tracktion"));
    assert!(
        w.vocab
            .modules
            .contains_key("@k7-core/instruments/synth/engines/sampler")
    );
    assert!(
        !song::modules::base_module_refs(&w.vocab.modules)
            .contains(&"@k7-core/instruments/synth/engines/sampler".to_owned())
    );
    // engines.test.ts, "finds each engine's panel in its own folder".
    assert!(
        w.vocab
            .panels
            .ui
            .contains_key("@k7-core/instruments/synth/engines/tracktion")
    );
    assert!(
        !w.vocab
            .panels
            .ui
            .contains_key("@k7-core/instruments/synth/engines/sampler")
    );
}

// engines.test.ts, "runs the default engine, with the module's identity and the engine's panel", and decision 59's KEPT.
#[test]
fn a_module_runs_as_its_default_engine_keeping_its_identity() {
    let m = node("params({ ignored: 9 })\n", &[]);
    assert_eq!(plugin_name(&m), Some("TrackSynth"));
    assert!(m.authored.ui.is_some());
    assert_eq!(m.authored.extends.as_deref(), Some("@k7-core/instruments/synth"));
    assert_eq!(m.file, "lead");
    assert_eq!(m.authored.engine.as_ref().map(|e| e.0.as_str()), Some("tracktion"));
    assert_eq!(num(&m, "e"), Some(1.0));
    assert_eq!(num(&m, "ignored"), None, "its top-level params are never the engine's");
}

// engines.test.ts, "runs the engine a node picks, and keeps params PER engine", "falls back to the default", "an ALTERNATIVE switches it".
#[test]
fn a_node_picks_its_engine_and_sets_params_per_engine() {
    let m = node(
        "engine(\"sampler\")\nengines({ sampler: { params: { e: 99 } }, tracktion: { params: { e: 5 } } })\n",
        &[],
    );
    assert_eq!(plugin_name(&m), Some("Sampler"));
    assert_eq!((num(&m, "s"), num(&m, "e")), (Some(2.0), Some(99.0)));
    assert!(
        engine_params(&m, "tracktion").is_some(),
        "the per-engine sets stay on the node"
    );
    let missing = node("engine(\"nope\")\n", &[]);
    assert_eq!(plugin_name(&missing), Some("TrackSynth"));
    let alt = node(
        "alternatives([{ name: \"s\", engine: \"sampler\" }])\nalternative(\"s\")\n",
        &[],
    );
    assert_eq!(plugin_name(&alt), Some("Sampler"));
}

// engines.test.ts, "loads the FILE a node sets for its engine, not the engine's default one" (decision 59).
#[test]
fn a_file_set_for_an_engine_is_the_file_it_loads() {
    let m = node(
        "engine(\"sampler\")\nengines({ sampler: { params: { model: \"mine.nam\" } } })\n",
        &[("ws/proj/nodes/assets/mine.nam", "x")],
    );
    assert_eq!(m.model.as_deref(), Some("/p/ws/proj/nodes/assets/mine.nam"));
    assert_eq!(
        m.assets.as_ref().and_then(|a| a.get("model")).map(String::as_str),
        Some("mine.nam")
    );
}

// engines.test.ts, "a rack whose engines are its kits": the node's taps stay on a rack engine and are set aside on any other.
#[test]
fn a_nodes_taps_stay_on_a_rack_engine_and_are_set_aside_on_a_kit() {
    let rack = "rack({ symbols: { BD: { note: 36, clip: \"kick.wav\" } } })\n";
    let extra = [
        (
            "lib/k7-core/modules/instruments/pads/module.k7x",
            "type(\"rack\")\nplugin({ type: \"internal\", name: \"Pads\" })\n",
        ),
        (
            "lib/k7-core/modules/instruments/pads/engines/sampler/module.k7x",
            "type(\"rack\")\n",
        ),
        (
            "lib/k7-core/modules/instruments/pads/engines/kit/module.k7x",
            "plugin({ type: \"internal\", name: \"Kit\" })\n",
        ),
    ];
    let mut all = files(&[]);
    all.extend(extra.iter().map(|(p, t)| ((*p).to_owned(), (*t).to_owned())));
    all.push((
        "ws/proj/nodes/a.k7x".into(),
        format!("extends(\"@k7-core/instruments/pads\")\nengine(\"sampler\")\n{rack}"),
    ));
    all.push((
        "ws/proj/nodes/b.k7x".into(),
        format!("extends(\"@k7-core/instruments/pads\")\nengine(\"kit\")\n{rack}"),
    ));
    let borrowed: Vec<(&str, &str)> = all.iter().map(|(p, t)| (p.as_str(), t.as_str())).collect();
    let w = mem_world(&borrowed);
    assert_eq!(
        w.resolved.modules["a"].authored.rack.as_ref().map(|r| r.symbols.len()),
        Some(1)
    );
    assert_eq!(
        w.resolved.modules["b"].authored.rack.as_ref().map(|r| r.symbols.len()),
        None
    );
}
