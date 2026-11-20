// chain_nodes.rs — a chain project's nodes: items and lanes stamped as `loadChainNodes` stamps them (decisions 98 to 102).
// A reader would get wrong that a preset is only stat'ed, and that a name which escapes `vstpresets/` gives no preset and no `presetRel`.

use crate::support;

use song::modules::{ModuleKind, NodesRead, load_nodes};
use song::{Cache, Severity};
use std::sync::Mutex;
use support::{Spy, mem, mem_world, mem_world_roots};

fn nodes(files: &[(&str, &str)]) -> NodesRead {
    nodes_under(files, song::PathRule::HOST)
}

fn nodes_under(files: &[(&str, &str)], rule: song::PathRule) -> NodesRead {
    let roots = song::Roots {
        paths: rule,
        ..mem_world_roots()
    };
    load_nodes(&mem(files), &mut Cache::with_paths(rule), &roots).expect("nodes")
}

fn errors(n: &NodesRead) -> Vec<String> {
    n.findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .map(|f| f.file.clone())
        .collect()
}

const CHAIN: &str = "ws/proj/chains/gtr.k7x";
const BODY: &str = "start(true)\nsteps([\n  id(\"lane-1\").track(\"gtr\"),\n  id(\"amp-1\").plugin({ type: \"internal\", name: \"Chorus\" }),\n  to([chain(\"OUT\")]),\n])\n";
const LANE: &str = "ws/proj/tracks/gtr.k7x";

#[test]
fn an_item_and_a_lane_are_stamped_as_a_chain_nodes() {
    // Decision 99.
    let n = nodes(&[(CHAIN, BODY), (LANE, "extends(\"@meta/track\")\n")]);
    assert!(n.chain_project && n.findings.is_empty(), "{:?}", n.findings);
    let amp = &n.nodes["amp-1"];
    assert_eq!(
        (amp.name.as_deref(), amp.node.as_deref(), amp.file.as_str()),
        (Some("amp-1"), Some("amp-1"), "amp-1")
    );
    assert_eq!(
        (amp.home.as_deref(), amp.chain.as_deref(), amp.node_id.as_deref()),
        (Some("chains"), Some("gtr"), Some("amp-1"))
    );
    assert_eq!(amp.kind, Some(ModuleKind::Plugin));
    // An item keeps the module it was written as: no `type: plugin` default (decision 92).
    assert!(amp.authored.module_type.is_none());
    let lane = &n.nodes["gtr"];
    assert_eq!(
        (lane.home.as_deref(), lane.chain.as_deref(), lane.node_id.as_deref()),
        (Some("tracks"), Some("gtr"), Some("lane-1"))
    );
    assert!(lane.preset_rel.is_none() && lane.authored.extends.as_deref() == Some("@meta/track"));
    assert_eq!(amp.folder_abs.as_deref(), Some("/p/ws/proj"));
}

#[test]
fn a_lane_with_no_file_is_an_error_on_its_track_key_and_the_node_is_absent() {
    // Decision 98.
    let n = nodes(&[(CHAIN, BODY)]);
    assert_eq!(errors(&n), ["chains/gtr.k7x"]);
    assert_eq!((n.findings[0].line, n.findings[0].col), (3, 16));
    assert!(!n.nodes.contains_key("gtr") && n.nodes.contains_key("amp-1"));
}

#[test]
fn a_lane_file_that_does_not_decode_is_named_by_itself() {
    let n = nodes(&[(CHAIN, BODY), (LANE, "bogus_key(1)\n")]);
    assert_eq!(errors(&n), ["tracks/gtr.k7x"]);
    assert!(!n.nodes.contains_key("gtr"));
}

#[test]
fn a_picture_item_in_a_video_track_is_no_module() {
    let c = "start(true)\nsteps([id(\"v\").video_track(\"vt\"), to([chain(\"RENDER\")])])\n";
    let n = nodes(&[(CHAIN, c)]);
    assert!(n.nodes.is_empty() && n.findings.is_empty());
}

#[test]
fn a_known_plugin_format_with_no_name_is_an_error_at_the_items_plugin_key() {
    // Decision 23 over an item.
    let c = "start(true)\nsteps([\n  id(\"n\").plugin({ type: \"VST3\" }),\n])\n";
    let n = nodes(&[(CHAIN, c)]);
    assert_eq!(errors(&n), ["chains/gtr.k7x"]);
    assert_eq!((n.findings[0].line, n.findings[0].col), (3, 11));
}

fn with_preset(item: &str, extra: &[(&str, &str)]) -> NodesRead {
    with_preset_under(item, extra, song::PathRule::HOST)
}

fn with_preset_under(item: &str, extra: &[(&str, &str)], rule: song::PathRule) -> NodesRead {
    let c = format!("start(true)\nsteps([{item}])\n");
    let mut files = vec![(CHAIN, c.as_str())];
    files.extend_from_slice(extra);
    nodes_under(&files, rule)
}

const ITEM: &str = "id(\"a\").plugin({ type: \"internal\", name: \"X\" })";

#[test]
fn a_default_preset_is_the_ids_and_its_rel_is_present_even_when_the_file_is_not() {
    // Decision 100.
    let n = with_preset(ITEM, &[("ws/proj/vstpresets/a.vstpreset", "x")]);
    assert_eq!(
        n.nodes["a"].preset.as_deref(),
        Some("/p/ws/proj/vstpresets/a.vstpreset")
    );
    assert_eq!(n.nodes["a"].preset_rel.as_deref(), Some("vstpresets/a.vstpreset"));
    let n = with_preset(ITEM, &[]);
    assert!(n.nodes["a"].preset.is_none());
    assert_eq!(n.nodes["a"].preset_rel.as_deref(), Some("vstpresets/a.vstpreset"));
}

#[test]
fn a_named_preset_and_a_chosen_base() {
    let n = with_preset(
        &format!("{ITEM}.vstpreset(\"warm.vstpreset\")"),
        &[("ws/proj/vstpresets/warm.vstpreset", "x")],
    );
    assert_eq!(
        n.nodes["a"].preset.as_deref(),
        Some("/p/ws/proj/vstpresets/warm.vstpreset")
    );
    // A chosen `base:` still wins over the node's own state.
    let based = "id(\"a\").plugin({ type: \"internal\", name: \"X\" }).base(\"clean\")";
    let n = with_preset(
        based,
        &[
            ("ws/proj/vstpresets/a.vstpreset", "x"),
            ("ws/proj/vstpresets/base/clean.vstpreset", "x"),
        ],
    );
    assert_eq!(
        n.nodes["a"].preset.as_deref(),
        Some("/p/ws/proj/vstpresets/base/clean.vstpreset")
    );
}

#[test]
fn a_preset_name_that_escapes_gives_no_preset_and_no_rel() {
    // Decision 100 DEPARTS: `presetRel` is where the engine WRITES, so a `../../x` there is a write out of the project.
    for name in ["../../x", "/etc/passwd", "../x.vstpreset"] {
        let n = with_preset(&format!("{ITEM}.vstpreset(\"{name}\")"), &[("etc/passwd", "x")]);
        assert!(
            n.nodes["a"].preset.is_none() && n.nodes["a"].preset_rel.is_none(),
            "{name}"
        );
        assert!(n.findings.is_empty());
    }
}

#[test]
fn a_backslash_in_a_preset_name_is_a_character_under_the_posix_rule_and_a_way_out_under_the_windows_one() {
    // Decision 100: `presetRel` is where the engine WRITES, and Windows reads `\` as a separator.
    for name in ["..\\\\..\\\\x", "a\\\\b.vstpreset"] {
        for rule in [song::PathRule::Posix, song::PathRule::Windows] {
            let n = with_preset_under(&format!("{ITEM}.vstpreset(\"{name}\")"), &[], rule);
            let rel = n.nodes["a"].preset_rel.as_deref();
            if rule.is_windows() {
                assert!(rel.is_none(), "{name}");
            } else {
                assert_eq!(
                    rel,
                    Some(format!("vstpresets/{}", name.replace("\\\\", "\\")).as_str()),
                    "{name}"
                );
            }
            assert!(n.findings.is_empty());
        }
    }
}

#[test]
fn a_preset_is_never_read() {
    // Decision 100 and the read set: stat'ed only, so a bundle holds it by its length.
    let src = mem(&[
        (CHAIN, &format!("start(true)\nsteps([{ITEM}])\n")),
        ("ws/proj/vstpresets/a.vstpreset", "x"),
    ]);
    let spy = Spy {
        inner: &src,
        reads: Mutex::new(Vec::new()),
    };
    let n = load_nodes(&spy, &mut Cache::new(), &mem_world_roots()).expect("nodes");
    assert!(n.nodes["a"].preset.is_some());
    assert!(
        spy.reads
            .lock()
            .expect("lock")
            .iter()
            .all(|p| !p.contains("vstpresets"))
    );
}

#[test]
fn plugin_findings_over_an_item_are_named_by_its_chain_file() {
    // Decision 102 DEPARTS: `chains/<chain>.k7x`, at the item's `plugin` key.
    let c = "start(true)\nsteps([\n  id(\"bogus\").plugin({ type: \"builtin\", name: \"Sampler\" }),\n])\n";
    let w = mem_world(&[(CHAIN, c)]);
    let f = song::resolve::plugin_findings(&w.nodes.nodes, &w.resolved.modules);
    assert_eq!(
        f.iter()
            .map(|f| (f.file.as_str(), f.severity, f.line))
            .collect::<Vec<_>>(),
        [("chains/gtr.k7x", Severity::Error, 3)]
    );
}

#[test]
fn identity_is_never_inherited() {
    // Decision 101: chain, nodeId and presetRel stay the node's own through the fold.
    let w = mem_world(&[
        (
            CHAIN,
            "start(true)\nsteps([id(\"n1\").extends(\"@k7-core/audio/amp\")])\n",
        ),
        (
            "lib/k7-core/modules/audio/amp/module.k7x",
            "plugin({ type: \"internal\", name: \"A\" })\n",
        ),
    ]);
    let r = &w.resolved.modules["n1"];
    assert_eq!(
        (r.chain.as_deref(), r.node_id.as_deref(), r.home.as_deref()),
        (Some("gtr"), Some("n1"), Some("chains"))
    );
    assert_eq!(r.preset_rel.as_deref(), Some("vstpresets/n1.vstpreset"));
}
