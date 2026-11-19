// graph_hostile.rs — the passes of 2k on hostile and full-size input, each answered fast with the answer asserted (441).
// A reader would get wrong that the work is fed by hand: `Inputs` and `GraphDoc` are plain data, so 100,000 chains need no 100,000 files.

use crate::graph_support;

use graph_support::{fast, inputs_with, lane, world};
use indexmap::IndexMap;
use serde_json::json;
use song::graph::{GraphDoc, MAX_ISSUES, load_graph, validate_inner};
use song::lanes::inputs::Inputs;
use song::routing::{Chain, Out, UseStep};
use song::{Cache, Resolved};

const N: usize = 100_000;

fn step(module: &str) -> UseStep {
    UseStep {
        module: module.to_owned(),
        inputs: None,
        enabled: None,
    }
}

fn out(to: &str) -> Out {
    Out {
        to: to.to_owned(),
        gain: 1.0,
        from_port: None,
    }
}

fn chain(from: Option<&str>, steps: &[&str], outs: &[&str]) -> Chain {
    Chain {
        from: from.map(str::to_owned),
        from_port: None,
        use_steps: steps.iter().map(|s| step(s)).collect(),
        out: outs.iter().map(|o| out(o)).collect(),
    }
}

const SHADER: &str = "type(\"shader\")\n";

/// A small project: a lane `a` of a clip and a sequence, an `amp`, a live input, a picture lane, a shader and a module of 5,000 outputs.
fn small() -> (song::MemSource, song::Roots, Inputs) {
    let a = lane("events([ref(\"k\").start(\"1:1\"), ref(\"r\").start(\"2:1\")])\n");
    let amp = "plugin({ type: \"internal\", name: \"Chorus\" })\ninputs([{ name: \"in\", type: \"audio\" }])\n";
    let outputs: Vec<String> = (0..5_000)
        .map(|i| format!("{{ name: \"o{i}\", type: \"audio\" }}"))
        .collect();
    let wide = format!(
        "plugin({{ type: \"internal\", name: \"W\" }})\noutputs([{}])\n",
        outputs.join(", ")
    );
    let files = [
        ("clips/k.k7x", "source(\"clips/k.wav\")\n"),
        ("sequences/r.k7x", "notes([t(0).d(1).k(60)])\n"),
        ("nodes/a.k7x", a.as_str()),
        ("nodes/amp.k7x", amp),
        ("nodes/mic.k7x", "io(\"in\")\nchannel(\"ghost\")\n"),
        ("nodes/pic.k7x", "extends(\"@meta/picture-track\")\n"),
        ("nodes/wide.k7x", wide.as_str()),
        ("lib/k7-core/modules/shaders/sh/module.k7x", SHADER),
        ("nodes/sh.k7x", "extends(\"@k7-core/shaders/sh\")\n"),
        (
            "graph.k7x",
            "modules({ a: [\"amp\"], amp: [\"OUT\"], pic: [\"RENDER\"], mic: [\"a\"] })\n",
        ),
    ];
    let (src, roots) = world(&files);
    let i = inputs_with(&src, &roots, &json!({}));
    (src, roots, i)
}

/// The graph of `small()` with its chains replaced, validated, and the time it took asserted.
fn validated(what: &str, chains: IndexMap<String, Chain>) -> (Vec<song::graph::GraphIssue>, bool) {
    let (src, roots, i) = small();
    let mut g = load_graph(&src, &mut Cache::new(), &roots, &i).expect("graph");
    g.chains = chains;
    fast(what, 5, || validate_inner(&g, &i.vocabulary))
}

fn chains_of(list: Vec<(String, Chain)>) -> IndexMap<String, Chain> {
    list.into_iter().collect()
}

// 409: a ring of 100,000 chains gives one chainCycle and no stack overflow.
#[test]
fn a_ring_of_a_hundred_thousand_chains_is_one_cycle() {
    let ring = (0..N)
        .map(|n| (format!("c{n}"), chain(None, &[], &[&format!("c{}", (n + 1) % N)])))
        .collect();
    let (issues, more) = validated("ring", chains_of(ring));
    assert_eq!(
        issues.iter().map(|i| i.key()).collect::<Vec<_>>(),
        ["chainCycle|chain=c0"]
    );
    assert!(!more);
}

// 409: a ladder of 100,000 chains to OUT, every one from the lane, reaches the sink with one pass.
#[test]
fn a_ladder_of_a_hundred_thousand_chains_to_out_reaches_it() {
    let ladder = (0..N)
        .map(|n| {
            let to = if n + 1 == N {
                "OUT".to_owned()
            } else {
                format!("c{}", n + 1)
            };
            (format!("c{n}"), chain(Some("a"), &[], &[&to]))
        })
        .collect();
    let (issues, _) = validated("ladder", chains_of(ladder));
    assert_eq!(issues.iter().map(|i| i.key()).collect::<Vec<_>>(), Vec::<String>::new());
}

// 408: 100,000 outs naming a step hosted in a run of 100,000 steps ask the index, not the chains.
#[test]
fn outs_naming_a_hosted_step_ask_one_index() {
    let steps = vec!["amp"; N];
    let outs = vec!["amp#p"; N];
    let (issues, _) = validated(
        "hosted",
        chains_of(vec![
            ("run".into(), chain(None, &steps, &["OUT"])),
            ("x".into(), chain(None, &["amp"], &outs)),
        ]),
    );
    assert!(
        issues.iter().all(|i| i.code.name() != "outUnknown"),
        "{:?}",
        issues.first()
    );
}

// 408: a chain of 100,000 steps with 100,000 outs to a picture lane asks what it carries once; the issues stop at the cap (410).
#[test]
fn a_long_chain_with_many_outs_to_a_picture_lane_is_capped() {
    let steps = vec!["amp"; N];
    let outs = vec!["pic"; N];
    let (issues, more) = validated("long", chains_of(vec![("x".into(), chain(None, &steps, &outs))]));
    assert_eq!(issues.len(), MAX_ISSUES);
    assert!(more);
    assert!(issues.iter().all(|i| i.code.name() == "outIsPictureLane"));
}

// 408: 100,000 from-less picture runs each ask isStretch, which is a stretch and so no warning.
#[test]
fn a_hundred_thousand_stretches_are_each_asked_once() {
    let runs = (0..N)
        .map(|n| (format!("r{n}"), chain(None, &["sh"], &["pic"])))
        .collect();
    let (issues, _) = validated("stretches", chains_of(runs));
    assert_eq!(issues.iter().map(|i| i.key()).collect::<Vec<_>>(), Vec::<String>::new());
}

// 408: 100,000 chains from one lane holding audio and sequences ask the lane's kinds once; 1,000 issues and the sentence.
#[test]
fn a_hundred_thousand_chains_from_a_mixed_lane_are_capped() {
    let list = (0..N)
        .map(|n| (format!("c{n}"), chain(Some("a"), &["amp"], &["OUT"])))
        .collect();
    let (issues, more) = validated("mixed", chains_of(list));
    assert_eq!(issues.len(), MAX_ISSUES);
    assert!(more && issues.iter().all(|i| i.code.name() == "chainMixedLane"));
}

// 408 and 410: 100,000 outs leaving a module of 5,000 outputs by a port it lacks join the outputs once, cut.
#[test]
fn a_module_of_five_thousand_outputs_is_joined_once_and_cut() {
    let outs: Vec<Out> = (0..N)
        .map(|n| Out {
            to: "OUT".into(),
            gain: 1.0,
            from_port: Some(format!("p{n}")),
        })
        .collect();
    let c = Chain {
        from: None,
        from_port: None,
        use_steps: vec![step("wide")],
        out: outs,
    };
    let (issues, more) = validated("ports", chains_of(vec![("x".into(), c)]));
    assert_eq!(issues.len(), MAX_ISSUES);
    assert!(more);
    assert!(issues.iter().all(|i| i.params["outputs"].chars().count() <= 200));
    assert!(issues[0].params["outputs"].starts_with("o0, o1, o2"));
}

// 410: a group of 100,000 members all also in a second group gives 1,000 issues and the sentence.
#[test]
fn a_group_of_a_hundred_thousand_members_in_two_groups_is_capped_with_a_sentence() {
    let names: Vec<String> = (0..N).map(|n| format!("\"m{n}\"")).collect();
    let tracks = names.join(", ");
    let g1 = format!("extends(\"@meta/group-track\")\ntracks([{tracks}])\n");
    let files = [("nodes/g1.k7x", g1.as_str()), ("nodes/g2.k7x", g1.as_str())];
    let t = fast("groups", 5, || {
        graph_support::with_graph(&files, |g| (g.issues.len(), g.warnings.clone()))
    });
    assert_eq!(t.0, MAX_ISSUES);
    assert_eq!(t.1.len(), MAX_ISSUES + 1);
    assert_eq!(
        t.1.last().map(String::as_str),
        Some("more than 1000 warnings — the rest are left out")
    );
}

// 410: 100,000 assembly members, none of them a node, are capped.
#[test]
fn a_hundred_thousand_assembly_members_are_capped() {
    let (src, roots, i) = small();
    let mut g: GraphDoc<'_> = load_graph(&src, &mut Cache::new(), &roots, &i).expect("graph");
    let members = (0..N).map(|n| (format!("m{n}"), format!("ghost{n}"))).collect();
    g.assemblies.insert(
        "box".into(),
        song::assemblies::Assembly {
            uses: "@k7-core/x".into(),
            label: "x".into(),
            members,
            inputs: Vec::new(),
            outputs: Vec::new(),
            params: IndexMap::new(),
            missing: None,
        },
    );
    let (issues, more) = fast("assembly", 5, || validate_inner(&g, &i.vocabulary));
    assert_eq!(issues.len(), MAX_ISSUES);
    assert!(more);
}

// 410: a chain name of 64 KiB over 1,000 unknown steps is quoted cut, in a bounded number of issues.
#[test]
fn a_name_of_a_megabyte_is_quoted_cut() {
    let big = "n".repeat(1 << 16);
    let steps = vec!["ghost/x"; 1_000];
    let (issues, _) = validated("big name", chains_of(vec![(big, chain(None, &steps, &["OUT"]))]));
    assert_eq!(issues.len(), MAX_ISSUES.min(1_000));
    assert!(
        issues
            .iter()
            .all(|i| i.params.values().all(|v| v.chars().count() <= 200))
    );
    assert!(issues.iter().all(|i| i.text.len() < 1_000));
}

// 411: 100,000 inputs with an unknown label on a rig of 4,096 labels give the cap and the sentence, one index built.
#[test]
fn a_hundred_thousand_inputs_with_an_unknown_label_are_capped_in_time() {
    let (src, roots, mut i) = small();
    let proto: Resolved = i.local.modules.get("mic").expect("mic").clone();
    for n in 0..N {
        i.local.modules.insert(format!("in{n}"), proto.clone());
    }
    i.rig = song::lanes::rig::Rig::from_json(
        &json!({"channels": {"in": (0..4_096).map(|n| json!({"ch": [1], "label": format!("l{n}")})).collect::<Vec<_>>()}}),
    );
    let g = fast("labels", 5, || {
        load_graph(&src, &mut Cache::new(), &roots, &i).expect("graph")
    });
    let labels = g
        .warnings
        .iter()
        .filter(|w| w.contains("is not an input label"))
        .count();
    assert!(
        labels > 0 && g.warnings.len() <= MAX_ISSUES + 1,
        "{} {labels}",
        g.warnings.len()
    );
    assert!(g.warnings.iter().any(|w| w.contains("have: l0, l1")));
}

// build: 100,000 lanes, each fed to OUT, are 100,000 tracks and chains in time.
#[test]
fn a_hundred_thousand_lanes_build_in_time() {
    let (src, roots, mut i) = small();
    let proto: Resolved = i.local.modules.get("a").expect("a").clone();
    let mut modules: IndexMap<String, Vec<String>> = IndexMap::new();
    for n in 0..N {
        i.local.modules.insert(format!("l{n}"), proto.clone());
        modules.insert(format!("l{n}"), vec!["OUT".into()]);
    }
    i.routing.segments.modules = Some(modules);
    let g = fast("lanes", 5, || {
        load_graph(&src, &mut Cache::new(), &roots, &i).expect("graph")
    });
    assert_eq!(g.tracks.len(), N + 1);
    assert_eq!(g.chains.len(), N);
}

// build: 100,000 disabled lanes, each feeding a chain, are dropped with their chains in time.
#[test]
fn a_hundred_thousand_disabled_lanes_drop_their_chains() {
    let (src, roots, mut i) = small();
    let off: Resolved = {
        let (s2, r2) = world(&[("nodes/off.k7x", lane("disabled(true)\n").as_str())]);
        inputs_with(&s2, &r2, &json!({}))
            .local
            .modules
            .get("off")
            .expect("off")
            .clone()
    };
    let mut modules: IndexMap<String, Vec<String>> = IndexMap::new();
    for n in 0..N {
        i.local.modules.insert(format!("d{n}"), off.clone());
        modules.insert(format!("d{n}"), vec!["OUT".into()]);
    }
    i.routing.segments.modules = Some(modules);
    let g = fast("disabled", 5, || {
        load_graph(&src, &mut Cache::new(), &roots, &i).expect("graph")
    });
    assert_eq!(g.disabled.len(), N);
    assert!(g.chains.is_empty() && g.issues.is_empty());
}

// 441: a run of 100,000 inputs ending in 100,000 destinations finds where it lands once, not once per input.
#[test]
fn a_run_of_a_hundred_thousand_inputs_over_a_hundred_thousand_outs_finds_its_landing_once() {
    let steps = vec!["mic"; N];
    let outs = vec!["OUT"; N];
    let (issues, _) = validated("inputs", chains_of(vec![("run".into(), chain(None, &steps, &outs))]));
    assert!(
        issues.iter().all(|i| !i.key().starts_with("inputTrack")),
        "{:?}",
        issues.iter().map(song::graph::GraphIssue::key).collect::<Vec<_>>()
    );
}

// 410: a module name of 64 KiB, quoted by a thousand steps, is cut as a chain name is.
#[test]
fn a_module_name_of_a_megabyte_is_quoted_cut() {
    let big = "m".repeat(1 << 16);
    let steps = vec![big.as_str(); 1_000];
    let (issues, _) = validated(
        "big module",
        chains_of(vec![("run".into(), chain(None, &steps, &["OUT"]))]),
    );
    assert!(!issues.is_empty() && issues.len() <= MAX_ISSUES);
    assert!(
        issues
            .iter()
            .all(|i| i.params.values().all(|v| v.chars().count() <= 200))
    );
    assert!(issues.iter().all(|i| i.text.len() < 1_000));
}

// 410: a port name of 64 KiB on a thousand cables from a module of 5,000 outputs is cut in the issue and in the sentence.
#[test]
fn a_port_name_of_a_megabyte_is_quoted_cut() {
    let big = "p".repeat(1 << 16);
    let mut run = chain(None, &["wide"], &[]);
    run.out = (0..1_000)
        .map(|_| Out {
            to: "OUT".to_owned(),
            gain: 1.0,
            from_port: Some(big.clone()),
        })
        .collect();
    let (issues, _) = validated("big port", chains_of(vec![("run".into(), run)]));
    assert!(issues.iter().any(|i| i.key().starts_with("outBadPort")));
    assert!(issues.len() <= MAX_ISSUES);
    assert!(
        issues
            .iter()
            .all(|i| i.params.values().all(|v| v.chars().count() <= 200))
    );
    assert!(issues.iter().all(|i| i.text.len() < 2_000));
}
