// chain_project.rs — a chain project read: its chains in name order, hidden ones set aside, nodes by name (decisions 89 to 98, 103).
// A reader would get wrong that a case here runs over MemSource: names Windows cannot hold or fold are not laid on a disk.

use crate::support;

use song::routing::{ChainProject, is_chain_project, load_routing, read_chain_project, read_layout};
use song::{Cache, Severity};
use support::{mem, mem_world_roots};

const P: &str = "/p/ws/proj";
const LEAF: &str = "extends(\"@k7-core/audio/amp\")";

fn chain(items: &str) -> String {
    format!("start(true)\nsteps([{items}, to([chain(\"OUT\")])])\n")
}

fn read(files: &[(&str, &str)]) -> ChainProject {
    let src = mem(files);
    read_chain_project(&src, &mut Cache::new(), P).expect("a project")
}

fn errors(p: &ChainProject) -> Vec<String> {
    p.findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .map(|f| f.file.clone())
        .collect()
}

#[test]
fn a_project_is_a_chain_project_when_chains_is_a_folder() {
    // Decision 89.
    let src = mem(&[
        ("ws/proj/chains/a.k7x", "steps([])\n"),
        ("ws/proj/graph.k7x", "modules({a:[\"b\"]})\n"),
    ]);
    assert!(is_chain_project(&src, P).expect("probe"));
    let r = load_routing(&src, &mut Cache::new(), &mem_world_roots()).expect("routing");
    // A graph.k7x beside it is ignored.
    assert!(r.chain_project && r.segments.modules.as_ref().is_some_and(|m| m.is_empty()));
    let src = mem(&[("ws/proj/graph.k7x", "modules({a:[\"b\"]})\n")]);
    assert!(!is_chain_project(&src, P).expect("probe"));
    let r = load_routing(&src, &mut Cache::new(), &mem_world_roots()).expect("routing");
    assert_eq!(r.segments.modules.expect("modules")["a"], ["b"]);
    let none = load_routing(
        &mem(&[("ws/proj/project.k7x", "")]),
        &mut Cache::new(),
        &mem_world_roots(),
    )
    .expect("routing");
    assert!(!none.chain_project && none.segments == song::routing::Segments::default() && none.findings.is_empty());
}

#[test]
fn chains_are_read_in_the_order_of_their_names_not_their_files() {
    // chainProject.test.ts "reading order": `polysynth-copy` sorts before `polysynth` by file name and not by chain name.
    let a = chain(&format!("id(\"a\").{LEAF}"));
    let p = read(&[
        ("ws/proj/chains/polysynth-copy.k7x", &a),
        ("ws/proj/chains/polysynth.k7x", &chain(&format!("id(\"b\").{LEAF}"))),
    ]);
    assert_eq!(p.chains.keys().collect::<Vec<_>>(), ["polysynth", "polysynth-copy"]);
}

#[test]
fn a_hidden_chain_is_set_aside_and_a_broken_one_still_counts() {
    // chainProject.test.ts "is read back hidden once written so" (read half), decision 95.
    let hidden = format!("hidden(true)\n{}", chain(&format!("id(\"h\").{LEAF}")));
    let p = read(&[
        ("ws/proj/chains/h.k7x", &hidden),
        ("ws/proj/chains/x.k7x", &chain(&format!("id(\"x\").{LEAF}"))),
    ]);
    assert!(p.hidden.contains_key("h") && !p.chains.contains_key("h") && !p.nodes.contains_key("h"));
    let p = read(&[("ws/proj/chains/broken.k7x", "hidden(true)\nsteps([[]])\n")]);
    assert_eq!(errors(&p), ["chains/broken.k7x"]);
}

#[test]
fn nodes_are_named_by_lane_else_id_and_say_where_they_sit() {
    let c = "start(true)\nsteps([id(\"l1\").track(\"gtr\"), id(\"a\").extends(\"x\"), [id(\"b\").extends(\"x\"), id(\"c\").extends(\"x\")], {sub: \"s\"}, to([chain(\"OUT\")])])\nsubchains([{id: \"s\", steps: [id(\"d\").extends(\"x\")]}])\n";
    let p = read(&[("ws/proj/chains/g.k7x", c)]);
    let n = &p.nodes;
    assert_eq!(
        (
            n["gtr"].id.as_str(),
            n["gtr"].track.as_deref(),
            n["gtr"].step,
            n["gtr"].item
        ),
        ("l1", Some("gtr"), 0, None)
    );
    assert_eq!((n["c"].step, n["c"].item), (2, Some(1)));
    assert_eq!((n["d"].sub.as_deref(), n["d"].chain.as_str()), (Some("s"), "g"));
}

#[test]
fn a_node_name_used_twice_is_an_error_on_the_later_item_and_the_later_value_wins() {
    // Decision 96.
    let one = chain(&format!("id(\"n\").{LEAF}"));
    let two = chain(&format!("id(\"n\").{LEAF}.params({{ Mix: 1 }})"));
    let p = read(&[("ws/proj/chains/a.k7x", &one), ("ws/proj/chains/b.k7x", &two)]);
    assert_eq!(errors(&p), ["chains/b.k7x"]);
    assert_eq!(p.nodes["n"].chain, "b");
    assert_eq!(p.nodes.len(), 1);
}

#[test]
fn a_lane_whose_file_extends_the_picture_track_is_a_picture_lane() {
    // Decision 97, with the corner: a lane file that does not decode is not picture.
    let c = "start(true)\nsteps([id(\"l\").track(\"main\"), id(\"v\").video_track(\"vt\"), to([chain(\"RENDER\")])])\n";
    let p = read(&[
        ("ws/proj/chains/m.k7x", c),
        ("ws/proj/tracks/main.k7x", "extends(\"@meta/picture-track\")\n"),
    ]);
    assert_eq!(
        (p.nodes["main"].picture, p.nodes["vt"].picture),
        (Some(true), Some(true))
    );
    let p = read(&[
        ("ws/proj/chains/m.k7x", c),
        ("ws/proj/tracks/main.k7x", "extends(\"@meta/track\")\n"),
    ]);
    assert_eq!(p.nodes["main"].picture, None);
    let p = read(&[
        ("ws/proj/chains/m.k7x", c),
        ("ws/proj/tracks/main.k7x", "extends(\"@meta/picture-track\"\n"),
    ]);
    assert_eq!(p.nodes["main"].picture, None);
}

#[test]
fn a_chain_that_does_not_decode_is_an_error_named_by_its_file_and_left_out() {
    let p = read(&[
        ("ws/proj/chains/bad.k7x", "steps([\"a\"])\n"),
        ("ws/proj/chains/ok.k7x", &chain(&format!("id(\"o\").{LEAF}"))),
    ]);
    assert_eq!(errors(&p), ["chains/bad.k7x"]);
    assert_eq!(p.chains.keys().collect::<Vec<_>>(), ["ok"]);
}

#[test]
fn a_file_is_read_by_the_extension_in_any_case() {
    // Decision 95: the file read is the one listed, so `a.K7X` reads.
    let src = mem(&[("ws/proj/chains/a.K7X", &chain(&format!("id(\"a\").{LEAF}")))]);
    let p = read_chain_project(&src, &mut Cache::new(), P).expect("project");
    assert!(p.chains.contains_key("a"));
}

#[test]
fn layout_pins_are_read_and_a_bad_file_is_an_error_named_layout_k7x() {
    // Decision 103.
    let ok = mem(&[("ws/proj/layout.k7x", "positions({ a: { x: 1, y: 2 } })\nother(1)\n")]);
    let (pos, bad) = read_layout(&ok, &mut Cache::new(), P).expect("layout");
    assert!(bad.is_none() && pos["a"].x == 1.0);
    let src = mem(&[
        ("ws/proj/layout.k7x", "positions({ a: { x: 1 } })\n"),
        ("ws/proj/chains/a.k7x", "steps([])\n"),
    ]);
    let (pos, bad) = read_layout(&src, &mut Cache::new(), P).expect("layout");
    assert!(pos.is_empty() && bad.is_some_and(|f| f.file == "layout.k7x"));
    let r = load_routing(&src, &mut Cache::new(), &mem_world_roots()).expect("routing");
    assert!(
        r.findings
            .iter()
            .any(|f| f.file == "layout.k7x" && f.severity == Severity::Error)
    );
    let none = mem(&[("ws/proj/chains/a.k7x", "steps([])\n")]);
    let (pos, bad) = read_layout(&none, &mut Cache::new(), P).expect("layout");
    assert!(pos.is_empty() && bad.is_none());
    let nopos = mem(&[("ws/proj/layout.k7x", "other(1)\n")]);
    assert!(read_layout(&nopos, &mut Cache::new(), P).expect("layout").0.is_empty());
}

#[test]
fn a_graph_that_does_not_read_is_default_segments_and_one_error() {
    // Decision 110.
    let src = mem(&[("ws/proj/graph.k7x", "busses({})\n")]);
    let r = load_routing(&src, &mut Cache::new(), &mem_world_roots()).expect("routing");
    assert!(r.segments == song::routing::Segments::default());
    assert_eq!(
        r.findings.iter().map(|f| f.file.as_str()).collect::<Vec<_>>(),
        ["graph.k7x"]
    );
}

#[test]
fn a_routing_over_the_budget_is_one_error_and_empty() {
    // Decision 105, at the level of load_routing.
    let items = |p: &str| {
        (0..5000)
            .map(|i| format!("id(\"{p}{i}\").{LEAF}"))
            .collect::<Vec<_>>()
            .join(",")
    };
    let big = format!(
        "start(true)\nsteps([id(\"h\").{LEAF}, [{}], [{}]])\n",
        items("a"),
        items("b")
    );
    let src = mem(&[("ws/proj/chains/big.k7x", &big)]);
    let r = load_routing(&src, &mut Cache::new(), &mem_world_roots()).expect("routing");
    assert!(r.capped && r.segments == song::routing::Segments::default());
    assert_eq!(
        r.findings
            .iter()
            .filter(|f| f.severity == Severity::Error)
            .map(|f| f.file.as_str())
            .collect::<Vec<_>>(),
        ["chains/big.k7x"]
    );
}
