// The TypeScript tests of `insertFor` and `jackRef` (cables.test.ts) and of `chainExit` (rowSeams.test.ts).

use super::*;
use crate::node::Node;

fn card(id: &str, kind: Kind) -> Node {
    Node {
        x: Some(0.0),
        y: Some(0.0),
        w: 10.0,
        h: 10.0,
        ..Node::plain(id, id, kind)
    }
}

fn index(nodes: &[Node]) -> HashMap<&str, &Node> {
    nodes.iter().map(|n| (n.id.as_str(), n)).collect()
}

fn insert(before: &str, after: &str) -> Option<EdgeInsert> {
    Some(EdgeInsert {
        before: before.into(),
        after: after.into(),
        stack: None,
    })
}

#[test]
fn offers_an_insert_between_two_modules_and_across_chains_too() {
    let nodes = [card("a", Kind::Module), card("b", Kind::Module)];
    assert_eq!(insert_for("a", "b", &index(&nodes)), insert("a", "b"));
}

#[test]
fn offers_one_on_the_empty_master_run_whose_two_ends_are_both_sinks() {
    let sink = |name: &str| Node {
        label: name.into(),
        ..card(&format!("snk:{name}"), Kind::Sink)
    };
    let nodes = [sink("MIX"), sink("OUT")];
    assert_eq!(insert_for("snk:MIX", "snk:OUT", &index(&nodes)), insert("MIX", "OUT"));
}

#[test]
fn refuses_between_two_non_modules_and_an_unknown_card() {
    // Two lanes never have an arrow, so a `+` there would address nothing.
    let nodes = [card("a", Kind::Track), card("b", Kind::Track)];
    assert_eq!(insert_for("a", "b", &index(&nodes)), None);
    assert_eq!(insert_for("a", "ghost", &index(&nodes)), None);
}

#[test]
fn names_a_node_as_the_chains_spell_it_and_suffixes_the_port_a_jack_names() {
    let named = |kind, id: &str, label: &str, module: Option<&str>| Node {
        label: label.into(),
        module: module.map(str::to_owned),
        ..card(id, kind)
    };
    assert_eq!(
        jack_ref(&named(Kind::Module, "m1", "reverb (long)", Some("reverb")), None),
        "reverb"
    );
    assert_eq!(jack_ref(&named(Kind::Track, "trk:gtr", "gtr", None), None), "gtr");
    assert_eq!(jack_ref(&named(Kind::Sink, "sink", "OUT", None), None), "OUT");
    assert_eq!(
        jack_ref(&named(Kind::Module, "m1", "split", Some("split")), Some("b")),
        "split#b"
    );
}

#[test]
fn a_box_answers_with_the_member_behind_the_plug_never_with_itself() {
    use crate::node::{AssemblyOf, PlugAt};
    let mut b = card("asm:A", Kind::Assembly);
    b.module = Some("A".into());
    b.assembly_of = Some(AssemblyOf {
        uses: "tpl".into(),
        open: false,
        members: vec![],
        plugs: [(
            "mix".to_owned(),
            PlugAt {
                node: "fadeA".into(),
                port: None,
            },
        )]
        .into_iter()
        .collect(),
        params: None,
        missing: None,
    });
    assert_eq!(jack_ref(&b, Some("mix")), "fadeA");
    // Its bare name is still its own: that is what a pin and `graph move` address.
    assert_eq!(jack_ref(&b, None), "A");
}

fn graph(json: serde_json::Value) -> Graph {
    serde_json::from_value(json).expect("a graph")
}

#[test]
fn the_exit_enters_a_chain_target_at_its_first_module() {
    let g = graph(serde_json::json!({"chains": {
        "lead": {"from": "gt_lead", "use": [{"module": "amp"}], "out": [{"to": "bus", "gain": 1}]},
        "bus": {"use": [{"module": "glue"}], "out": [{"to": "MIX", "gain": 1}]}
    }}));
    assert_eq!(chain_exit(&g, "amp"), insert("amp", "glue"));
}

#[test]
fn the_exit_refuses_a_fork_and_names_the_port_it_leaves_by() {
    let fork = graph(serde_json::json!({"chains": {
        "lead": {"use": [{"module": "amp"}], "out": [{"to": "MIX", "gain": 1}, {"to": "CUE", "gain": 1}]}
    }}));
    assert_eq!(chain_exit(&fork, "amp"), None);
    let ported = graph(serde_json::json!({"chains": {
        "lead": {"use": [{"module": "amp"}], "out": [{"to": "MIX", "gain": 1, "fromPort": "wet"}]}
    }}));
    assert_eq!(chain_exit(&ported, "amp"), insert("amp#wet", "MIX"));
}
