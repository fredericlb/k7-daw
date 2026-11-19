// resolve_findings.rs — the plugin findings over resolved nodes, and what an absent `extends` says: decisions 55, 60, 72.
// A reader would get wrong that the plugin may be INHERITED, and that a finding is placed, never matched by its message.

use crate::support;

use song::files::Severity;
use song::modules::Absent;
use song::resolve::{not_supported_findings, plugin_findings, unknown_plugin_findings};
use support::mem_world;

const LIB: &str = "lib/k7-core/modules";

fn files_of(f: &[song::Finding]) -> Vec<(String, Severity)> {
    f.iter().map(|f| (f.file.clone(), f.severity)).collect()
}

// unknownPluginType.test.ts, "reports a type no engine loads as an error on the node's file" (decision 72), placed at its `plugin` key.
#[test]
fn a_type_no_engine_loads_is_an_error_at_the_nodes_plugin_key() {
    let w = mem_world(&[(
        "ws/proj/nodes/bogus.k7x",
        "\nplugin({ type: \"builtin\", name: \"Sampler\" })\n",
    )]);
    let f = unknown_plugin_findings(&w.nodes.nodes, &w.resolved.modules);
    assert_eq!(files_of(&f), [("nodes/bogus.k7x".to_owned(), Severity::Error)]);
    assert_eq!((f[0].line, f[0].col), (2, 1));
    assert!(f[0].message.contains("\"builtin\""));
}

// unknownPluginType.test.ts, "leaves every known type and the not_supported marker alone", and `offline`, which has no engine plugin on purpose.
#[test]
fn every_known_type_the_marker_and_offline_are_left_alone() {
    let node = |t: &str| format!("plugin({{ type: \"{t}\", name: \"X\" }})\n");
    let files: Vec<(String, String)> = ["internal", "VST3", "clap", "jsdsp", "not_supported", "offline"]
        .iter()
        .map(|t| (format!("ws/proj/nodes/n-{t}.k7x"), node(t)))
        .collect();
    let borrowed: Vec<(&str, &str)> = files.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let w = mem_world(&borrowed);
    assert!(unknown_plugin_findings(&w.nodes.nodes, &w.resolved.modules).is_empty());
}

// notSupported.md: an INHERITED not_supported plugin warns at the node's own file, 1:1 when the node states no plugin.
#[test]
fn an_inherited_not_supported_plugin_warns_at_the_nodes_own_file() {
    let w = mem_world(&[
        (
            &format!("{LIB}/audio/ghost/module.k7x"),
            "plugin({ type: \"not_supported\", name: \"Ghost\" })\n",
        ),
        ("ws/proj/nodes/uses.k7x", "extends(\"@k7-core/audio/ghost\")\n"),
        (
            "ws/proj/nodes/fine.k7x",
            "plugin({ type: \"internal\", name: \"4osc\" })\n",
        ),
    ]);
    let f = not_supported_findings(&w.nodes.nodes, &w.resolved.modules);
    assert_eq!(files_of(&f), [("nodes/uses.k7x".to_owned(), Severity::Warning)]);
    assert_eq!((f[0].line, f[0].col), (1, 1));
    assert!(f[0].message.contains("Ghost"));
    assert_eq!(plugin_findings(&w.nodes.nodes, &w.resolved.modules).len(), 1);
}

// libraryModuleIssues.test.ts, "when a module DOES name it": a ref whose file is there and did not read says WHICH file; else Unknown.
#[test]
fn an_absent_ref_is_unknown_or_broken_with_its_file() {
    let w = mem_world(&[
        (&format!("{LIB}/audio/broken/module.k7x"), "plugin({ type: \"jsdsp\" \n"),
        ("ws/proj/nodes/a.k7x", "extends(\"@k7-core/audio/broken\")\n"),
        ("ws/proj/nodes/b.k7x", "extends(\"@k7-core/audio/gone\")\n"),
    ]);
    assert_eq!(
        w.vocab.absent("@k7-core/audio/broken"),
        Absent::Broken {
            file: "@k7-core/audio/broken/module.k7x".into()
        }
    );
    assert_eq!(w.vocab.absent("@k7-core/audio/gone"), Absent::Unknown);
    assert_eq!(
        w.resolved.modules["b"].authored.extends.as_deref(),
        Some("@k7-core/audio/gone")
    );
}

// nodes.test.ts, "merges a ui written in a node, like any other extends", "still lets a node override everything else".
#[test]
fn a_node_merges_its_ui_per_widget_and_overrides_the_rest() {
    let w = mem_world(&[
        (
            &format!("{LIB}/instruments/synth/module.k7x"),
            "plugin({ type: \"internal\", name: \"S\" })\nui({ cols: 1, rows: 1, widgets: { a: { type: \"knob\", col: 0, row: 0, label: \"SHARED\" } } })\nparams({ \"Filter Freq\": 40 })\n",
        ),
        (
            "ws/proj/nodes/lead.k7x",
            "extends(\"@k7-core/instruments/synth\")\nui({ cols: 2, rows: 2, widgets: { a: { type: \"knob\", col: 1, row: 1, label: \"MINE\" } } })\nparams({ \"Filter Freq\": 90 })\ncontent(\"the lead voice\")\n",
        ),
    ]);
    let lead = &w.resolved.modules["lead"];
    let ui = lead.authored.ui.as_ref().expect("ui");
    assert_eq!((ui.cols.0, ui.widgets["a"].label.as_deref()), (2, Some("MINE")));
    assert_eq!(lead.authored.content.as_deref(), Some("the lead voice"));
    assert!(
        matches!(lead.authored.params.as_ref().and_then(|p| p.get("Filter Freq")), Some(song::modules::ParamValue::Number(n)) if (*n - 90.0).abs() < 1e-9)
    );
}

// coldModules.test.ts, "is INHERITED, so a use of a cold module is cold".
#[test]
fn the_cold_flag_is_inherited() {
    let w = mem_world(&[
        (
            &format!("{LIB}/cold/squash/module.k7x"),
            "cold(true)\nplugin({ type: \"jsdsp\", name: \"Squash\" })\nparams({ Target: -1 })\n",
        ),
        (
            "ws/proj/nodes/master.k7x",
            "extends(\"@k7-core/cold/squash\")\nparams({ Target: -3 })\n",
        ),
    ]);
    assert_eq!(w.resolved.modules["master"].authored.cold, Some(true));
}

// Decision 60: a 2c finding in a library is named by REF: the module file, a node file beside it, the panel and the script.
#[test]
fn findings_outside_the_project_are_named_by_ref() {
    let w = mem_world(&[
        (
            &format!("{LIB}/audio/amp/module.k7x"),
            "plugin({ type: \"internal\", name: \"Amp\" })\n",
        ),
        (&format!("{LIB}/audio/amp/ui.k7x"), "nonsense(1)\n"),
        (&format!("{LIB}/audio/amp/controller.k7x"), "g({ ch: 99, cc: 1 })\n"),
        (&format!("{LIB}/audio/amp/widgets.js"), "x"),
    ]);
    let mut files: Vec<&str> = w.vocab.findings.iter().map(|f| f.file.as_str()).collect();
    files.sort_unstable();
    assert_eq!(
        files,
        ["@k7-core/audio/amp/controller.k7x", "@k7-core/audio/amp/ui.k7x"]
    );
}

// Decision 71: a stripped key waits for a LATER sub-step; the gate refuses one that does not.
#[test]
fn a_stale_strip_entry_is_refused() {
    let ok = support::strip_checked(&serde_json::json!([{ "key": "x", "step": "2m" }]));
    assert_eq!(ok, [("x".to_owned(), "2m".to_owned())]);
    let stale = std::panic::catch_unwind(|| support::strip_checked(&serde_json::json!([{ "key": "x", "step": "2l" }])));
    assert!(stale.is_err());
    assert!(std::panic::catch_unwind(|| support::strip_checked(&serde_json::json!([{ "key": "x" }]))).is_err());
}
