// views_errors_list.rs — the error list of `project_errors`: what a refusal or a clean compile lists, in which order, and how it is cut.
// A reader would get wrong that a refusal lists its own findings alone: the warnings are for a compile that passed.

use crate::views_support;
use crate::views_world;

use indexmap::IndexMap;
use song::Cache;
use song::doc::{AutomationDoc, Compiled, PluginDoc, ProjectDoc, Request, Spent, TrackDoc, Vsts, compile};
use song::files::{Finding, Severity};
use song::lanes::rig::Rig;
use song::views::MAX_DIAGNOSTICS;
use song::views::errors::project_errors;
use views_support::fast;
use views_world::{bare, roots, world, world_bare};

fn request() -> Request {
    Request {
        roots: roots(),
        rig: Rig::default(),
        vsts: Vsts::Scan,
        thawed: Vec::new(),
    }
}

fn automation(plugin: Option<&str>, param: &str) -> AutomationDoc {
    AutomationDoc {
        param: param.into(),
        plugin_id: plugin.map(str::to_owned),
        bipolar: None,
        points: vec![[0.0, 0.0]],
    }
}

fn plugin(id: &str, kind: &str, graph: Option<&str>) -> PluginDoc {
    let mut p = PluginDoc::new(id);
    p.plugin_type = kind.into();
    p.dsp_graph = graph.map(str::to_owned);
    p
}

fn doc_with(plugins: Vec<PluginDoc>, lanes: Vec<AutomationDoc>) -> ProjectDoc {
    let mut t = TrackDoc::new("t");
    t.plugins = plugins;
    t.automation = lanes;
    let mut d = ProjectDoc::empty();
    d.tracks = vec![t];
    d
}

fn finding(file: &str, message: &str, severity: Severity) -> Finding {
    Finding {
        file: file.into(),
        line: 1,
        col: 1,
        message: message.into(),
        severity,
    }
}

fn passed(doc: ProjectDoc, diagnostics: Vec<Finding>) -> Compiled {
    Compiled {
        engine: Ok(doc),
        diagnostics,
        freezes: Vec::new(),
        renders_used: Vec::new(),
        spent: Spent::default(),
    }
}

fn errors_of(files: &[(&str, &str)], c: &Compiled) -> song::views::errors::ProjectErrors {
    let src = world(files);
    project_errors(&src, &mut Cache::new(), &request(), c, None).expect("errors")
}

// 639, compileError.test.ts: a refusal lists `<file>: <message>` of its findings and the diagnostics, nothing else.
#[test]
fn a_refusal_lists_its_own_findings_only() {
    // A root file and no graph: not a project, where a folder with no root file at all is unsupported.
    let src = world(&[("project.k7x", "tempo(120)\n")]);
    let c = compile(&src, &mut Cache::new(), &request()).expect("a compile");
    assert!(c.engine.is_err());
    let e = project_errors(&src, &mut Cache::new(), &request(), &c, None).expect("errors");
    assert_eq!(e.errors.len(), 1);
    assert!(
        e.errors[0].starts_with("project.k7x: not a k7 project"),
        "{}",
        e.errors[0]
    );
    assert_eq!(e.diagnostics.len(), 1);
    assert_eq!(e.diagnostics[0].severity, Severity::Error);
    assert!(e.format_too_new.is_none());
}

// 641, compileError.test.ts: a newer format carries both levels and the diagnostic sits on the format line.
#[test]
fn a_newer_format_carries_its_levels_and_sits_on_the_format_line() {
    let files = [
        ("project.k7x", "project({ tempo: 120 })\nformat(6)\n"),
        ("graph.k7x", "modules({})\n"),
    ];
    let src = world(&files);
    let c = compile(&src, &mut Cache::new(), &request()).expect("a compile");
    let e = project_errors(&src, &mut Cache::new(), &request(), &c, None).expect("errors");
    let levels = e.format_too_new.expect("the levels");
    assert_eq!((levels.file_level, levels.build_level), (6, 5));
    assert_eq!(e.errors.len(), 1);
    assert_eq!(
        (e.diagnostics[0].file.as_str(), e.diagnostics[0].line),
        ("project.k7x", 2)
    );
}

// 641, 668: a folder holding only a newer-format project.k7x is refused as "update k7", where it would otherwise say it is no project.
#[test]
fn a_newer_format_with_no_graph_is_still_update_k7() {
    let src = world(&[("project.k7x", "format(999)\n")]);
    let c = compile(&src, &mut Cache::new(), &request()).expect("a compile");
    assert!(c.engine.as_ref().is_err_and(|f| f.format_too_new));
    let e = project_errors(&src, &mut Cache::new(), &request(), &c, None).expect("errors");
    let levels = e.format_too_new.expect("the levels");
    assert_eq!((levels.file_level, levels.build_level), (999, 5));
}

// 639, jsdspGraph.test.ts: a jsdsp module whose graph does not build is in /project-errors with `errors` empty, and again from the held document.
#[test]
fn a_jsdsp_finding_is_listed_and_errors_stay_empty() {
    let files = [
        ("nodes/gtr.k7x", "extends(\"@meta/track\")\n"),
        ("nodes/dsp.k7x", "plugin({ type: \"jsdsp\", name: \"dsp\" })\n"),
        ("nodes/jsdsp.ts", "export {}\n"),
        ("nodes/jsdsp.json", "{\"nodes\":[{\"type\":\"nonsense\"}]}\n"),
        ("graph.k7x", "modules({ gtr: [\"dsp\"], dsp: [\"MIX\"] })\n"),
    ];
    let src = world(&files);
    let c = compile(&src, &mut Cache::new(), &request()).expect("a compile");
    assert!(c.engine.is_ok());
    for _ in 0..2 {
        let e = project_errors(&src, &mut Cache::new(), &request(), &c, None).expect("errors");
        assert!(e.errors.is_empty());
        let d = e
            .diagnostics
            .iter()
            .find(|d| d.file == "nodes/dsp.k7x")
            .expect("the finding");
        assert_eq!(d.severity, Severity::Error);
    }
}

// 639: a plugin no engine loads is an ERROR and one not supported a WARNING, each in the list of a compile that passed.
#[test]
fn the_plugin_findings_lead_the_list_of_a_clean_compile() {
    let files = [
        ("nodes/odd.k7x", "plugin({ type: \"nonesuch\", name: \"Odd\" })\n"),
        (
            "nodes/gone.k7x",
            "plugin({ type: \"not_supported\", name: \"Gone\" })\n",
        ),
    ];
    let c = passed(
        ProjectDoc::empty(),
        vec![finding("clips/x.k7x", "from the compile", Severity::Warning)],
    );
    let e = errors_of(&files, &c);
    assert!(e.errors.is_empty());
    let files: Vec<&str> = e.diagnostics.iter().map(|d| d.file.as_str()).collect();
    let first_compile = files.iter().position(|f| *f == "clips/x.k7x").expect("the compile's");
    let odd = files
        .iter()
        .position(|f| *f == "nodes/odd.k7x")
        .expect("an unknown plugin");
    let gone = files
        .iter()
        .position(|f| *f == "nodes/gone.k7x")
        .expect("a not supported one");
    assert!(odd < first_compile && gone < first_compile, "{files:?}");
    let sev = |f: &str| e.diagnostics.iter().find(|d| d.file == f).map(|d| d.severity);
    assert_eq!(
        (sev("nodes/odd.k7x"), sev("nodes/gone.k7x")),
        (Some(Severity::Error), Some(Severity::Warning))
    );
}

// 639: the order is the plugin findings, the compile's, the dead automations, the vocabulary's, the dead fields, the surfaces, the assemblies.
#[test]
fn the_sources_come_in_their_order_and_a_finding_read_twice_is_listed_once() {
    let files = [
        ("project.k7x", "project({ modules_from: [\"../x\"] })\n"),
        ("nodes/odd.k7x", "plugin({ type: \"nonesuch\", name: \"Odd\" })\n"),
        ("lib/account/modules/audio/broken/module.k7x", "((\n"),
        ("clips/c.k7x", "source(\"a.wav\")\n"),
    ];
    let doc = doc_with(
        vec![plugin("lead:1", "4osc", None)],
        vec![automation(Some("lead:1"), "Voices 1")],
    );
    let twice = finding("clips/c.k7x", "same", Severity::Warning);
    let c = passed(doc, vec![twice.clone(), twice]);
    let e = errors_of(&files, &c);
    let at = |pred: &dyn Fn(&song::views::Diagnostic) -> bool| e.diagnostics.iter().position(pred).expect("listed");
    let odd = at(&|d| d.file == "nodes/odd.k7x");
    let compile = at(&|d| d.message == "same");
    let dead_auto = at(&|d| d.message.contains("plain plugin state"));
    let library = at(&|d| d.file.ends_with("broken/module.k7x"));
    let field = at(&|d| d.message.contains("modules_from"));
    let surface = at(&|d| d.file == "surfaces/");
    assert!(odd < compile && compile < dead_auto && dead_auto < library && library < field && field < surface);
    assert_eq!(e.diagnostics.iter().filter(|d| d.message == "same").count(), 1);
}

// 639: a warning only the reads hold is the compile's to list; the errors view adds no step of its own for it.
#[test]
fn a_read_warning_the_compile_did_not_report_is_not_added_again() {
    let mut inputs = bare(IndexMap::new());
    inputs
        .warnings
        .push(finding("clips/w.k7x", "a read warning", Severity::Warning));
    let c = passed(ProjectDoc::empty(), Vec::new());
    let src = world(&[]);
    let e = project_errors(&src, &mut Cache::new(), &request(), &c, Some(&inputs)).expect("errors");
    assert!(e.diagnostics.iter().all(|d| d.message != "a read warning"));
}

// 639, diagnostics.ts: a jsdsp module no chain wires is judged too, since `pluginFindings` walks every resolved node.
#[test]
fn a_jsdsp_module_the_document_does_not_build_is_still_judged() {
    let files = [
        ("graph.k7x", "modules({ gtr: [\"MIX\"] })\n"),
        ("nodes/gtr.k7x", "extends(\"@meta/track\")\n"),
        ("nodes/orphan.k7x", "plugin({ type: \"jsdsp\", name: \"dsp\" })\n"),
        ("nodes/jsdsp.ts", "export default (g) => { g.nope(); };\n"),
    ];
    let src = world(&files);
    let c = compile(&src, &mut Cache::new(), &request()).expect("a compile");
    assert!(c.engine.is_ok());
    let e = project_errors(&src, &mut Cache::new(), &request(), &c, None).expect("errors");
    let d = e
        .diagnostics
        .iter()
        .find(|d| d.file == "nodes/orphan.k7x")
        .expect("the finding");
    assert_eq!(d.severity, Severity::Error);
}

// 640: past MAX_DIAGNOSTICS the list holds the first 1,000 and a last WARNING naming how many were left out; a text is cut at 300 bytes.
#[test]
fn a_hundred_thousand_findings_are_a_thousand_and_the_last() {
    let many: Vec<Finding> = (0..100_000)
        .map(|i| {
            finding(
                &format!("clips/{i}.k7x"),
                &format!("{} {i}", "m".repeat(400)),
                Severity::Warning,
            )
        })
        .collect();
    let c = passed(ProjectDoc::empty(), many);
    let e = fast("100,000 findings", || errors_of(&[], &c));
    assert_eq!(e.diagnostics.len(), MAX_DIAGNOSTICS + 1);
    let last = &e.diagnostics[MAX_DIAGNOSTICS];
    assert_eq!(last.severity, Severity::Warning);
    assert!(last.message.starts_with("99000 more"), "{}", last.message);
    assert!(
        e.diagnostics
            .iter()
            .all(|d| d.message.len() <= 300 && d.file.len() <= 300)
    );
    // A refusal is cut the same way, its flat list too.
    let mut refused = passed(ProjectDoc::empty(), Vec::new());
    refused.engine = Err(song::doc::Failure {
        diagnostics: (0..5_000)
            .map(|i| finding(&format!("f{i}.k7x"), "bad", Severity::Error))
            .collect(),
        format_too_new: false,
    });
    let e = errors_of(&[], &refused);
    assert_eq!(
        (e.diagnostics.len(), e.errors.len()),
        (MAX_DIAGNOSTICS + 1, MAX_DIAGNOSTICS + 1)
    );
    assert_eq!(e.errors[0], "f0.k7x: bad");
}

// 606, 639: a project with a clip folder and no timeline surface carries the surface warning in its list.
#[test]
fn a_project_with_clips_and_no_timeline_surface_says_so() {
    let c = passed(ProjectDoc::empty(), Vec::new());
    let e = errors_of(&[("clips/c.k7x", "source(\"a.wav\")\n")], &c);
    assert!(
        e.diagnostics
            .iter()
            .any(|d| d.file == "surfaces/" && d.severity == Severity::Warning)
    );
}

// 811: a folder with no `project.k7x` is unsupported whatever else it holds: the TypeScript's `isFolderProject` needs that file.
#[test]
fn a_folder_with_no_root_file_is_unsupported_even_with_a_graph() {
    for files in [
        &[("graph.k7x", "")][..],
        &[("modules/a.k7x", "extends(\"@workspace/x\")\n")][..],
        &[("clips/a.k7x", "file(\"a.wav\")\n")][..],
    ] {
        let src = world_bare(files);
        let c = compile(&src, &mut Cache::new(), &request()).expect("a compile");
        let e = project_errors(&src, &mut Cache::new(), &request(), &c, None).expect("errors");
        assert_eq!(
            e.errors,
            ["unsupported song 'proj' — use a .k7x file or a folder project"],
            "{files:?}"
        );
    }
}

// 811: a project root that is no folder and no `.k7x` file is the TypeScript's sentence, naming no file.
#[test]
fn a_project_that_is_not_there_is_unsupported_and_names_no_file() {
    let src = world(&[]);
    let c = compile(&src, &mut Cache::new(), &request()).expect("a compile");
    let e = project_errors(&src, &mut Cache::new(), &request(), &c, None).expect("errors");
    assert_eq!(
        e.errors,
        ["unsupported song 'proj' — use a .k7x file or a folder project"]
    );
    assert_eq!(e.diagnostics[0].file, "");
}
