// views_io.rs — the I/O panel's ports, after ioPorts.test.ts: inherited modules and their nodes, channels, labels, places.
// A reader would get wrong that `@meta/IN`, `OUT` and `CUE` are always listed: the vocabulary carries the app's own io roles.

use crate::views_world;

use serde_json::json;
use song::lanes::inputs::Inputs;
use song::views::io_ports::{IoDir, IoPort, io_ports};
use views_world::{inputs, inputs_with};

fn rig() -> serde_json::Value {
    json!({ "channels": { "in": [{ "label": "guitar", "ch": [3] }], "out": [] } })
}

fn desk() -> Inputs {
    inputs_with(
        &[
            ("project.k7x", "project({ name: \"t\" })\n"),
            ("ws/_modules/io/IN/module.k7x", "io(\"in\")\n"),
            ("ws/_modules/io/IN/in-1.k7x", "channel([1])\n"),
            ("ws/_modules/io/IN/in-2.k7x", "channel([1, 2])\n"),
            ("ws/_modules/io/IN/gtr.k7x", "channel(\"guitar\")\n"),
            ("ws/_modules/io/IN/ghost.k7x", "channel(\"nosuchinput\")\n"),
            ("ws/_modules/io/OUT/module.k7x", "io(\"out\")\n"),
        ],
        &rig(),
    )
}

fn by(ports: &[IoPort], r: &str) -> IoPort {
    ports
        .iter()
        .find(|p| p.ref_ == r)
        .cloned()
        .unwrap_or_else(|| panic!("no port {r}"))
}

#[test]
fn lists_the_inherited_io_modules_and_their_nodes() {
    // ioPorts.test.ts "lists the inherited io modules AND their nodes" (decision 622).
    let refs: Vec<String> = io_ports(&desk()).ports.into_iter().map(|p| p.ref_).collect();
    for r in [
        "@workspace/io/IN",
        "@workspace/io/IN:in-1",
        "@workspace/io/IN:in-2",
        "@workspace/io/OUT",
    ] {
        assert!(refs.iter().any(|x| x == r), "{r} in {refs:?}");
    }
}

#[test]
fn puts_the_inputs_first_and_each_group_by_code_point() {
    // ioPorts.test.ts "puts the inputs first, so the panel reads in order" (decision 622).
    let ports = io_ports(&desk()).ports;
    let last_in = ports.iter().rposition(|p| p.dir == IoDir::In).expect("an input");
    let first_out = ports.iter().position(|p| p.dir == IoDir::Out).expect("an output");
    assert!(first_out > last_in);
    for dir in [IoDir::In, IoDir::Out] {
        let refs: Vec<&str> = ports.iter().filter(|p| p.dir == dir).map(|p| p.ref_.as_str()).collect();
        let mut sorted = refs.clone();
        sorted.sort_unstable();
        assert_eq!(refs, sorted);
    }
}

#[test]
fn resolves_a_channel_whether_numbers_or_a_label() {
    // ioPorts.test.ts "resolves a channel, whether it is numbers or a preferences label" (622).
    let ports = io_ports(&desk()).ports;
    assert_eq!(by(&ports, "@workspace/io/IN:in-1").channels, [1]);
    assert_eq!(by(&ports, "@workspace/io/IN:in-2").channels, [1, 2]);
    let gtr = by(&ports, "@workspace/io/IN:gtr");
    assert_eq!(gtr.channels, [3]);
    assert_eq!(gtr.label.as_deref(), Some("guitar"));
    assert_eq!(gtr.unknown_label, None);
}

#[test]
fn says_when_a_label_names_an_input_this_rig_does_not_have() {
    // ioPorts.test.ts "says when a label names an input this rig doesn't have" (622).
    let ghost = by(&io_ports(&desk()).ports, "@workspace/io/IN:ghost");
    assert_eq!(ghost.unknown_label, Some(true));
    assert!(ghost.channels.is_empty());
    assert_eq!(ghost.label.as_deref(), Some("nosuchinput"));
}

#[test]
fn marks_the_generic_which_declares_no_channel() {
    // ioPorts.test.ts "marks the generic, which declares no channel of its own" (622).
    let ports = io_ports(&desk()).ports;
    let generic = by(&ports, "@workspace/io/IN");
    assert!(generic.generic && generic.channels.is_empty());
    assert!(!by(&ports, "@workspace/io/IN:in-1").generic);
}

#[test]
fn carries_the_namespace_and_a_name_worth_showing() {
    // ioPorts.test.ts "carries the namespace and a name worth showing" (622).
    let p = by(&io_ports(&desk()).ports, "@workspace/io/IN:in-1");
    assert_eq!((p.name.as_str(), p.namespace.as_str()), ("IN:in-1", "workspace"));
    // A bare ref has no namespace and is its own name.
    let own = inputs(&[("nodes/OUT.k7x", "io(\"out\")\n")]);
    let o = by(&io_ports(&own).ports, "OUT");
    assert_eq!((o.name.as_str(), o.namespace.as_str()), ("OUT", ""));
}

#[test]
fn still_lists_a_projects_own_io_when_nothing_is_inherited() {
    // ioPorts.test.ts "still lists a project's OWN io when nothing is inherited" (622).
    let own = inputs(&[("nodes/OUT.k7x", "io(\"out\")\n")]);
    let ports = io_ports(&own).ports;
    let o = by(&ports, "OUT");
    assert_eq!(o.dir, IoDir::Out);
    assert!(o.generic);
}

#[test]
fn a_node_is_an_input_through_what_it_extends() {
    // decision 622: the node is resolved first, since `extends: "@meta/IN"` is an input only through its parent.
    let p = inputs(&[("nodes/mic.k7x", "extends(\"@meta/IN\")\nchannel([5])\n")]);
    let ports = io_ports(&p).ports;
    let mic = by(&ports, "mic");
    assert_eq!((mic.dir, mic.channels.clone()), (IoDir::In, vec![5]));
    // The app's own roles are listed, and only the audio ones: no file, camera or midi.
    let refs: Vec<&str> = ports.iter().map(|p| p.ref_.as_str()).collect();
    for r in ["@meta/IN", "@meta/OUT", "@meta/CUE"] {
        assert!(refs.contains(&r), "{r}");
    }
    for r in ["@meta/FILE", "@meta/CAM", "@meta/MIDI"] {
        assert!(!refs.contains(&r), "{r}");
    }
}

#[test]
fn an_unresolvable_node_is_read_as_written_and_costs_only_itself() {
    // decision 622: a node extending nothing that exists is no port, and the others still list.
    let p = inputs(&[
        ("nodes/lost.k7x", "extends(\"@nowhere/io/IN\")\n"),
        ("nodes/ok.k7x", "io(\"out\")\n"),
    ]);
    let refs: Vec<String> = io_ports(&p).ports.into_iter().map(|p| p.ref_).collect();
    assert!(refs.iter().any(|r| r == "ok") && !refs.iter().any(|r| r == "lost"));
}

#[test]
fn a_chain_projects_node_is_shown_where_it_sits() {
    // decision 623: a node is named by an id; what a person reads is `chain · step n`, and it is never a generic.
    let chain = "start(true)\nsteps([id(\"n1\").io(\"in\").channel([2]), to([chain(\"OUT\")])])\n";
    let p = inputs(&[("chains/rec.k7x", chain)]);
    let ports = io_ports(&p).ports;
    let n = by(&ports, "n1");
    assert_eq!(n.name, "rec · step 1");
    assert!(!n.generic);
    assert_eq!(n.channels, [2]);
}

#[test]
fn the_budget_and_a_wide_channel_list_are_bounded() {
    // decision 624: a channel list is cut at the rig's 64, and a plain project is far under the budget.
    let rig = json!({ "channels": { "in": [{ "label": "wide", "ch": (1..=200).collect::<Vec<u32>>() }], "out": [] } });
    let p = inputs_with(&[("nodes/w.k7x", "extends(\"@meta/IN\")\nchannel(\"wide\")\n")], &rig);
    let w = by(&io_ports(&p).ports, "w");
    assert_eq!(w.channels.len(), 64);
    let all = io_ports(&desk());
    assert!(!all.capped && all.findings.is_empty() && all.spent > 0 && all.spent < 1000);
}
