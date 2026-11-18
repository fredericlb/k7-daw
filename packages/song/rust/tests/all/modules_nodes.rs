// modules_nodes.rs — a project's `nodes/`: keyed by file name, stamped as the loader stamps, and the plugin rule.
// A reader would get wrong that a chain project's nodes are not `nodes/` files: they are chain items (chain_nodes.rs).

use crate::support;

use song::modules::types::split_module_key;
use song::modules::{ModuleKind, NodesRead, build_vocabulary_with, load_nodes};
use song::{Cache, Severity};
use support::{Tmp, disk, plugin_text, roots};

fn nodes(t: &Tmp) -> NodesRead {
    let r = roots(t);
    let source = disk(&r);
    let mut cache = Cache::new();
    load_nodes(&source, &mut cache, &r).expect("nodes")
}

// nodes.test.ts and nodeFolder.test.ts: the loading cases.
#[test]
fn a_node_is_keyed_by_its_file_name_and_stamped_as_the_loader_does() {
    let t = Tmp::new("nodes-keys");
    t.write(
        "ws/proj/nodes/lead.k7x",
        "extends(\"@k7-core/audio/amp\")\nparams({ Mix: 1 })\n",
    );
    t.write("ws/proj/nodes/mic.k7x", "io(\"in\")\nchannel([1])\n");
    t.write("ws/proj/nodes/synth.k7x", &plugin_text("4OSC"));
    t.write("ws/proj/nodes/with space.k7x", "extends(\"@k7-core/audio/amp\")\n");
    let n = nodes(&t);
    assert!(n.findings.is_empty(), "{:?}", n.findings);
    assert!(!n.chain_project);
    let lead = &n.nodes["lead"];
    assert_eq!(
        (
            lead.name.as_deref(),
            lead.node.as_deref(),
            lead.file.as_str(),
            lead.home.as_deref()
        ),
        (Some("lead"), Some("lead"), "lead", Some("nodes"))
    );
    // A node that extends takes its kind from what it extends; the others state theirs.
    assert_eq!((lead.kind, lead.category.as_deref()), (None, None));
    assert_eq!(
        (n.nodes["mic"].kind, n.nodes["mic"].category.as_deref()),
        (Some(ModuleKind::Io), Some("io"))
    );
    assert_eq!(
        (n.nodes["synth"].kind, n.nodes["synth"].category.as_deref()),
        (Some(ModuleKind::Plugin), None)
    );
    assert!(n.nodes.contains_key("with space"));
    // Authored as it is: `params` is not defaulted, the ports not resolved.
    assert!(lead.authored.inputs.is_none() && n.nodes["synth"].authored.params.is_none());
    assert!(lead.folder_abs.as_deref().is_some_and(|p| p.ends_with("proj/nodes")));
    // nodeFolder.test.ts: splitModuleKey splits on the first separator.
    assert_eq!(split_module_key("synth:lead"), ("synth", Some("lead")));
    assert_eq!(split_module_key("a:b:c"), ("a", Some("b:c")));
    assert_eq!(split_module_key("plain"), ("plain", None));
}

// nodes.test.ts: nodes are out of the `extends` vocabulary, and the project's own `modules/` is not read.
#[test]
fn nodes_and_the_projects_own_modules_are_outside_the_vocabulary() {
    let t = Tmp::new("nodes-vocab");
    t.write("ws/proj/nodes/lead.k7x", "extends(\"@k7-core/audio/amp\")\n");
    t.write("ws/proj/modules/audio/mine/module.k7x", &plugin_text("Mine"));
    t.write("lib/k7-core/modules/audio/amp/module.k7x", &plugin_text("Amp"));
    let r = roots(&t);
    let source = disk(&r);
    let mut cache = Cache::new();
    let v = build_vocabulary_with(&source, &mut cache, &r, 100_000).expect("a vocabulary");
    assert!(v.modules.contains_key("@k7-core/audio/amp"));
    assert!(!v.modules.keys().any(|k| k.contains("mine") || k == "lead"));
    assert!(nodes(&t).nodes.contains_key("lead"));
}

// Decision 23: a node that does not decode is an ERROR finding for its file, located; the node is absent.
#[test]
fn a_node_that_does_not_decode_is_an_error_for_its_file() {
    let t = Tmp::new("nodes-bad");
    t.write("ws/proj/nodes/typo.k7x", "extends(\"@a/b\")\nparamz({})\n");
    t.write("ws/proj/nodes/ok.k7x", "extends(\"@a/b\")\n");
    let n = nodes(&t);
    assert!(!n.nodes.contains_key("typo") && n.nodes.contains_key("ok"));
    assert_eq!(n.findings.len(), 1);
    let f = &n.findings[0];
    assert_eq!(
        (f.file.as_str(), f.severity, f.line, f.col),
        ("nodes/typo.k7x", Severity::Error, 2, 1)
    );
}

// Decision 23: a known plugin format with no `plugin.name` is an error at the `plugin` key, beside decode findings.
#[test]
fn a_known_plugin_format_must_name_its_plugin() {
    let t = Tmp::new("nodes-plugin");
    t.write(
        "ws/proj/nodes/nameless.k7x",
        "content(\"x\")\nplugin({ type: \"vst3\" })\n",
    );
    t.write(
        "ws/proj/nodes/empty.k7x",
        "plugin({ type: \"Internal\", name: \"\" })\n",
    );
    t.write("ws/proj/nodes/named.k7x", &plugin_text("4OSC"));
    t.write("ws/proj/nodes/offline.k7x", "plugin({ type: \"offline\" })\n");
    t.write("ws/proj/nodes/unsupported.k7x", "plugin({ type: \"not_supported\" })\n");
    t.write("ws/proj/nodes/untyped.k7x", "plugin({ name: \"X\" })\n");
    t.write("ws/proj/nodes/broken.k7x", "nonsense(1)\n");
    let n = nodes(&t);
    let mut files: Vec<&str> = n.findings.iter().map(|f| f.file.as_str()).collect();
    files.sort_unstable();
    assert_eq!(files, ["nodes/broken.k7x", "nodes/empty.k7x", "nodes/nameless.k7x"]);
    let nameless = n
        .findings
        .iter()
        .find(|f| f.file == "nodes/nameless.k7x")
        .expect("a finding");
    assert_eq!(
        (nameless.severity, nameless.line, nameless.col),
        (Severity::Error, 2, 1)
    );
    // The views ignore the findings, so the node stays in the map.
    assert!(n.nodes.contains_key("nameless") && n.nodes.contains_key("empty"));
}

// Decision 18: a project with `chains/` has its nodes from its chain items, none here.
#[test]
fn a_chain_project_has_no_nodes_read_here() {
    let t = Tmp::new("nodes-chains");
    t.write("ws/proj/chains/main.k7x", "steps([])\n");
    t.write("ws/proj/nodes/a.k7x", "extends(\"@a/b\")\n");
    let n = nodes(&t);
    assert!(n.chain_project && n.nodes.is_empty() && n.findings.is_empty());
}

// Decision 4 of 2a, kept: a repeated base keeps the first position and the last file (`a.k7x` sorts after `a.K7X`).
// Over MemSource, where two names equal under case folding can be held on every platform.
#[test]
fn a_repeated_node_name_reads_one_file() {
    let source = support::mem(&[
        ("proj/nodes/a.k7x", "content(\"first\")\n"),
        ("proj/nodes/a.K7X", "content(\"second\")\n"),
    ]);
    let n = load_nodes(
        &source,
        &mut Cache::new(),
        &song::Roots {
            project: "/p/proj".into(),
            ..song::Roots::default()
        },
    )
    .expect("nodes");
    assert_eq!(n.nodes.len(), 1);
    assert_eq!(
        n.nodes["a"].authored.content.as_deref(),
        Some("first"),
        "the last file in bytewise order is a.k7x"
    );
}
