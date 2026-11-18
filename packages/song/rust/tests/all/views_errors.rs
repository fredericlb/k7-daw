// views_errors.rs — dead automations and dead project fields: what each finds, where it says it, and what it costs.
// A reader would get wrong that a chain name is a file name: one holding a separator names no chain file.

use crate::views_support;
use crate::views_world;

use song::Cache;
use song::doc::{AutomationDoc, PluginDoc, ProjectDoc, TrackDoc};
use song::files::{Finding, Severity};
use song::views::NAME_CUT;
use song::views::dead::{dead_automations, dead_project_fields};
use views_support::fast;
use views_world::world;

const DSP: &str = "{\"params\":[{\"name\":\"gain\"},{\"name\":\"mix\"}]}";

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

fn dead(files: &[(&str, &str)], doc: &ProjectDoc) -> Vec<Finding> {
    dead_under(files, doc, song::PathRule::HOST)
}

fn dead_under(files: &[(&str, &str)], doc: &ProjectDoc, rule: song::PathRule) -> Vec<Finding> {
    dead_automations(&world(files), "/p/ws/proj", doc, rule).expect("a check")
}

// 637, deadAutomations.ts: a built-in's plain-state control no curve can drive, said at project.k7x with no chain file.
#[test]
fn a_plain_state_control_is_a_warning_at_the_project() {
    let doc = doc_with(
        vec![plugin("lead:1", "4osc", None)],
        vec![automation(Some("lead:1"), "Voices 1")],
    );
    let f = dead(&[], &doc);
    assert_eq!(f.len(), 1);
    assert_eq!((f[0].file.as_str(), f[0].severity), ("project.k7x", Severity::Warning));
    assert!(
        f[0].message.contains("4osc's 'Voices 1' is plain plugin state"),
        "{}",
        f[0].message
    );
    // A control that automates is not a finding.
    let fine = doc_with(
        vec![plugin("lead:1", "4osc", None)],
        vec![automation(Some("lead:1"), "Tune 1")],
    );
    assert!(dead(&[], &fine).is_empty());
}

// 637: a lane bound to a name the compiled graph does not declare lists what it does declare.
#[test]
fn a_name_the_graph_lacks_lists_the_names_it_has() {
    let doc = doc_with(
        vec![plugin("lead:1", "jsdsp", Some(DSP))],
        vec![automation(Some("lead:1"), "freq"), automation(Some("lead:1"), "gain")],
    );
    let f = dead(&[], &doc);
    assert_eq!(f.len(), 1);
    assert!(
        f[0].message.contains("no 'freq' to automate (it has gain, mix)"),
        "{}",
        f[0].message
    );
    let none = doc_with(
        vec![plugin("a:1", "jsdsp", Some("{\"params\":[]}"))],
        vec![automation(Some("a:1"), "x")],
    );
    assert!(dead(&[], &none)[0].message.ends_with("(it has none)"));
}

// 637: a graph that will not read, or one with no list of names, judges nothing; a plugin or lane unnamed is skipped.
#[test]
fn what_cannot_be_judged_is_not_a_finding() {
    for graph in ["not json", "null", "{\"params\":7}", "{\"params\":[null]}", ""] {
        let doc = doc_with(
            vec![plugin("a:1", "jsdsp", Some(graph))],
            vec![automation(Some("a:1"), "x")],
        );
        assert!(dead(&[], &doc).is_empty(), "graph {graph:?}");
    }
    // Names that are not strings are what `String()` makes of them.
    let odd = doc_with(
        vec![plugin("a:1", "jsdsp", Some("{\"params\":[{\"name\":3},{},[1]]}"))],
        vec![automation(Some("a:1"), "3")],
    );
    assert!(dead(&[], &odd).is_empty());
    let lost = doc_with(
        vec![plugin("a:1", "4osc", None)],
        vec![automation(None, "Voices 1"), automation(Some("b:9"), "Voices 1")],
    );
    assert!(dead(&[], &lost).is_empty());
}

// 637: the first plugin of an id is the one judged, as `find` does.
#[test]
fn the_first_plugin_of_an_id_is_the_one_judged() {
    let doc = doc_with(
        vec![plugin("a:1", "4osc", None), plugin("a:1", "jsdsp", Some(DSP))],
        vec![automation(Some("a:1"), "Tune 1")],
    );
    assert!(dead(&[], &doc).is_empty());
}

// 637, 663: the file is chains/<chain>.k7x when a file is there, project.k7x when none is or the name holds a separator.
#[test]
fn the_chain_file_is_named_when_it_is_there() {
    let lane = |id: &str| doc_with(vec![plugin(id, "4osc", None)], vec![automation(Some(id), "Voices 1")]);
    let files = [
        ("chains/lead.k7x", "modules({})\n"),
        ("chains/a/b.k7x", "modules({})\n"),
        ("chains/..k7x", "modules({})\n"),
        ("chains/...k7x", "modules({})\n"),
        ("chains/.k7x", "modules({})\n"),
    ];
    assert_eq!(dead(&files, &lane("lead:1"))[0].file, "chains/lead.k7x");
    assert_eq!(dead(&files, &lane("other:1"))[0].file, "project.k7x");
    // A separator names no file; a dot name and an empty name are ordinary file names (`chains/..k7x`).
    for id in ["a/b:1", "../../x:1"] {
        assert_eq!(dead(&files, &lane(id))[0].file, "project.k7x", "{id}");
    }
    for (id, file) in [
        (".:1", "chains/..k7x"),
        ("..:1", "chains/...k7x"),
        (":1", "chains/.k7x"),
    ] {
        assert_eq!(dead(&files, &lane(id))[0].file, file, "{id}");
    }
}

// 637, portability: a backslash is a separator under the Windows rule and a letter of a name under the Posix one.
#[test]
fn a_backslash_chain_name_follows_the_path_rule() {
    let id = "a\\b:1";
    let doc = doc_with(vec![plugin(id, "4osc", None)], vec![automation(Some(id), "Voices 1")]);
    for (rule, want) in [
        (song::PathRule::Windows, "project.k7x"),
        (song::PathRule::Posix, "chains/a\\b.k7x"),
    ] {
        let f = dead_under(&[("chains/a\\b.k7x", "modules({})\n")], &doc, rule);
        assert_eq!(f[0].file, want, "{rule:?}");
    }
    let climbing = "..\\..\\x:1";
    let doc = doc_with(
        vec![plugin(climbing, "4osc", None)],
        vec![automation(Some(climbing), "Voices 1")],
    );
    assert_eq!(dead(&[], &doc)[0].file, "project.k7x");
}

// 637, hostile: 100,000 lanes on one plugin whose 1 MiB graph is parsed once, and each message stays under 300 bytes.
#[test]
fn a_big_graph_is_read_once_and_no_message_quotes_it_whole() {
    let names: String = (0..60_000)
        .map(|i| format!("{{\"name\":\"p{i:06}\"}}"))
        .collect::<Vec<_>>()
        .join(",");
    let graph = format!("{{\"params\":[{names}]}}");
    assert!(graph.len() > 1 << 20);
    let lanes = (0..100_000).map(|_| automation(Some("a:1"), "missing")).collect();
    let doc = doc_with(vec![plugin("a:1", "jsdsp", Some(&graph))], lanes);
    let f = fast("100,000 lanes on one big graph", || dead(&[], &doc));
    assert_eq!(f.len(), 100_000);
    assert!(f.iter().all(|x| x.message.len() <= 300), "{}", f[0].message.len());
    assert!(f[0].message.contains("p000000, p000001"));
}

// 637, hostile: a first param name of 1 MiB under 100,000 dead lanes is cut once per message, never copied whole each time.
#[test]
fn a_megabyte_param_name_is_not_copied_for_each_dead_lane() {
    let graph = format!(
        "{{\"params\":[{{\"name\":\"{}\"}},{{\"name\":\"b\"}}]}}",
        "n".repeat(1 << 20)
    );
    let lanes = (0..100_000).map(|_| automation(Some("a:1"), "missing")).collect();
    let doc = doc_with(vec![plugin("a:1", "jsdsp", Some(&graph))], lanes);
    let f = fast("100,000 lanes under a megabyte name", || dead(&[], &doc));
    assert_eq!(f.len(), 100_000);
    assert!(f.iter().all(|x| x.message.len() <= 300));
}

// 637, hostile: a chain file named with 8,000 bytes (a bundle key holds 8 KiB) and 20,000 lanes on it: each finding's file is a copy, so the budget pays for it.
#[test]
fn a_long_chain_file_name_is_charged_with_each_finding() {
    let chain = "c".repeat(8_000);
    let rel = format!("chains/{chain}.k7x");
    let id = format!("{chain}:1");
    let lanes = (0..20_000).map(|_| automation(Some(&id), "Voices 1")).collect();
    let doc = doc_with(vec![plugin(&id, "4osc", None)], lanes);
    let f = fast("20,000 lanes on a long chain name", || {
        dead(&[(rel.as_str(), "modules({})\n")], &doc)
    });
    let bytes: usize = f.iter().map(|x| x.file.len()).sum();
    assert!(bytes < 100_000_000, "{bytes} bytes of file names");
    assert!(
        f.last().is_some_and(|x| x.message.contains("limit")),
        "the check is capped"
    );
}

// 637, hostile: 100,000 plugins with a lane each, a param of 1 MB, and a type of 1 MB, none of them quoted whole.
#[test]
fn a_hundred_thousand_plugins_and_names_of_a_megabyte_are_cut() {
    let plugins: Vec<PluginDoc> = (0..100_000).map(|i| plugin(&format!("c{i}:1"), "4osc", None)).collect();
    let lanes = (0..100_000)
        .map(|i| automation(Some(&format!("c{i}:1")), "Voices 1"))
        .collect();
    let doc = doc_with(plugins, lanes);
    let f = fast("100,000 plugins", || dead(&[], &doc));
    assert_eq!(f.len(), 100_000);
    let big = "x".repeat(1 << 20);
    let doc = doc_with(
        vec![plugin("a:1", &big, Some(DSP))],
        vec![automation(Some("a:1"), &big)],
    );
    let f = dead(&[], &doc);
    assert_eq!(f.len(), 1);
    assert!(f[0].message.len() <= 300, "{}", f[0].message.len());
    assert!(f[0].message.chars().count() < NAME_CUT * 3);
}

// 638, diagnostics.ts: `modules_from` and `libraries_from` are two warnings at project.k7x, in that order; neither set is none.
#[test]
fn the_two_dead_project_fields_are_warnings_at_the_project() {
    let found = |text: &str| {
        let src = world(&[("project.k7x", text)]);
        dead_project_fields(&src, &mut Cache::new(), "/p/ws/proj").expect("a read")
    };
    assert!(found("project({ name: \"a\" })\n").is_empty());
    let both = found("project({ modules_from: [\"../x\"], libraries_from: [] })\n");
    let keys: Vec<bool> = [("modules_from", 0), ("libraries_from", 1)]
        .iter()
        .map(|(k, i)| both[*i].message.contains(k))
        .collect();
    assert_eq!(keys, [true, true]);
    assert!(
        both.iter()
            .all(|f| f.file == "project.k7x" && f.severity == Severity::Warning && (f.line, f.col) == (1, 1))
    );
    assert!(
        found("not a project file ((\n").is_empty(),
        "an unreadable header is the compile's error"
    );
    assert!(
        dead_project_fields(&world(&[]), &mut Cache::new(), "/p/ws/proj")
            .expect("a read")
            .is_empty()
    );
}
