// doc_compile.rs — the whole compile (decisions 303 to 308, 350 to 354): what stops it, what does not, the jsdsp plugins, the spend.
// A reader would get wrong that a Failure carries the stopping findings only: every finding the compile made is in `Compiled::diagnostics`.

use crate::doc_support;

use doc_support::{AMP, compiled, lane, project, stopped};
use song::files::Severity;

const GRAPH_OK: &str = "{\"params\":[{\"name\":\"gain\"}],\"nodes\":[{\"type\":\"gain\"}]}\n";

fn dsp_project(extra: &[(&str, &str)]) -> song::doc::Compiled {
    let mut files: Vec<(&str, &str)> = vec![
        ("nodes/gtr.k7x", "extends(\"@meta/track\")\n"),
        ("nodes/dsp.k7x", "plugin({ type: \"jsdsp\", name: \"dsp\" })\n"),
        ("graph.k7x", "modules({ gtr: [\"dsp\"], dsp: [\"MIX\"] })\n"),
    ];
    files.extend_from_slice(extra);
    compiled(&files)
}

// 350, jsdspBuildFailure.test.ts: a valid graph beside its source plays as its text, with no word.
#[test]
fn a_valid_graph_plays_as_its_text() {
    let c = dsp_project(&[("nodes/jsdsp.ts", "export {}\n"), ("nodes/jsdsp.json", GRAPH_OK)]);
    let d = c.engine.expect("a document");
    assert_eq!(d.tracks[0].plugins[0].dsp_graph.as_deref(), Some(GRAPH_OK));
    assert!(c.diagnostics.is_empty(), "{:?}", c.diagnostics);
}

// 350: a graph that is missing passes through and says so with an ERROR that does not stop the compile.
#[test]
fn a_missing_graph_passes_through_with_an_error_that_does_not_stop() {
    let c = dsp_project(&[("nodes/jsdsp.ts", "export {}\n")]);
    let d = c.engine.as_ref().expect("a document");
    assert_eq!(
        d.tracks[0].plugins[0].dsp_graph.as_deref(),
        Some("{\"params\":[],\"nodes\":[]}\n")
    );
    assert!(
        c.diagnostics
            .iter()
            .any(|f| f.file == "nodes/dsp.k7x" && f.severity == Severity::Error)
    );
}

// 350, 351: a graph the engine cannot read is an ERROR on its module and plays the pass-through; the compile still answers.
#[test]
fn an_invalid_graph_is_a_finding_and_the_pass_through() {
    let c = dsp_project(&[
        ("nodes/jsdsp.ts", "export {}\n"),
        ("nodes/jsdsp.json", "{\"nodes\":[{\"type\":\"nonsense\"}]}\n"),
    ]);
    let d = c.engine.as_ref().expect("a document");
    assert_eq!(
        d.tracks[0].plugins[0].dsp_graph.as_deref(),
        Some("{\"params\":[],\"nodes\":[]}\n")
    );
    assert!(
        c.diagnostics
            .iter()
            .any(|f| f.file == "nodes/dsp.k7x" && f.severity == Severity::Error)
    );
}

// 350: a json with no source beside it is an orphan and plays with no word.
#[test]
fn an_orphan_graph_plays_without_a_word() {
    let c = dsp_project(&[("nodes/jsdsp.json", GRAPH_OK)]);
    assert_eq!(
        c.engine.expect("a document").tracks[0].plugins[0].dsp_graph.as_deref(),
        Some(GRAPH_OK)
    );
    assert!(c.diagnostics.is_empty());
}

// 304, throw-or-finding: a project that stops for two reasons says both.
#[test]
fn two_reasons_to_fail_are_both_reported() {
    let c = project(
        "gtr: [\"e\"], e: [\"MIX\"]",
        &[
            ("nodes/e.k7x", "extends(\"@k7-core/audio/fx\")\nparams({ Wet: 1 })\n"),
            ("lib/k7-core/modules/audio/fx/module.k7x", AMP),
            ("nodes/bad.k7x", "plugin({ type: \"vst3\" })\n"),
        ],
    );
    assert_eq!(stopped(&c), ["nodes/bad.k7x", "nodes/e.k7x"]);
}

// 304: a format newer than this build refuses the compile and says so.
#[test]
fn a_newer_format_is_a_failure_flagged_too_new() {
    let c = compiled(&[
        ("nodes/gtr.k7x", &lane("")),
        ("graph.k7x", "modules({ gtr: [\"MIX\"] })\n"),
        ("project.k7x", "format(999)\n"),
    ]);
    assert!(c.engine.as_ref().err().is_some_and(|f| f.format_too_new));
}

// 302: a sample rate past what the engine reads as an int is an ERROR on project.k7x.
#[test]
fn a_sample_rate_past_u32_stops_on_the_head() {
    let c = compiled(&[
        ("nodes/gtr.k7x", &lane("")),
        ("graph.k7x", "modules({ gtr: [\"MIX\"] })\n"),
        ("project.k7x", "project({ sampleRate: 5000000000 })\n"),
    ]);
    assert_eq!(stopped(&c), ["project.k7x"]);
}

// 354: a number that is not finite is a compile error, not a null on the wire.
#[test]
fn a_non_finite_param_stops_on_its_node() {
    let c = project(
        "gtr: [\"p\"], p: [\"MIX\"]",
        &[(
            "nodes/p.k7x",
            "plugin({ type: \"internal\", name: \"Chorus\" })\nparams({ rate: 1e999 })\n",
        )],
    );
    assert!(c.engine.is_err(), "{:?}", c.diagnostics);
}

// 323, capacity: every budget a compile spent is printed and stays well under its limit on an ordinary project.
#[test]
fn an_ordinary_project_spends_a_little_of_each_budget() {
    let c = project("gtr: [\"amp\"], amp: [\"MIX\"]", &[]);
    let s = c.spent;
    eprintln!("spent {s:?}");
    assert!(s.doc > 0 && s.doc * 100 < song::doc::BUDGET);
    assert!(s.placer * 100 < song::clips::BUDGET && s.freeze * 100 < song::freeze::BUDGET);
}
