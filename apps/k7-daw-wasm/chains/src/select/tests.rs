// Shift+click selection, and the nodes a control on one of them drives: all or none.

use super::*;

fn node(module: &str) -> SelectNode {
    SelectNode {
        kind: "module".into(),
        module: Some(module.into()),
        uses: Some("@k7-core/audio/amp".into()),
        w: 100.0,
        h: 100.0,
        ..SelectNode::default()
    }
}

fn names(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn a_selection_grows_from_the_single_one_and_a_second_click_takes_a_node_back_out() {
    let two = toggle_selection(&[], Some("amp_1"), "amp_2");
    assert_eq!(two, names(&["amp_1", "amp_2"]));
    assert_eq!(toggle_selection(&two, Some("amp_1"), "amp_2"), names(&["amp_1"]));
    assert_eq!(toggle_selection(&[], None, "amp_2"), names(&["amp_2"]));
}

#[test]
fn every_other_selected_node_is_linked_when_all_are_one_ref_on_one_engine() {
    let (a, b, c) = (node("amp_1"), node("amp_2"), node("amp_3"));
    let sel = names(&["amp_1", "amp_2", "amp_3"]);
    assert_eq!(linked_nodes(&[a.clone(), b, c], &sel, &a), names(&["amp_2", "amp_3"]));
}

#[test]
fn a_mixed_selection_links_nothing_another_ref_or_another_engine() {
    let (a, b) = (node("amp_1"), node("amp_2"));
    let sel = names(&["amp_1", "amp_2", "amp_3"]);
    let other = SelectNode {
        uses: Some("@k7-core/audio/drive".into()),
        ..node("amp_3")
    };
    assert!(linked_nodes(&[a.clone(), b.clone(), other], &sel, &a).is_empty());
    let engine = SelectNode {
        engine: Some("fast".into()),
        ..node("amp_3")
    };
    assert!(linked_nodes(&[a.clone(), b, engine], &sel, &a).is_empty());
}

#[test]
fn nothing_links_from_a_node_outside_the_selection_nor_with_one_node_selected() {
    let (a, b, c) = (node("amp_1"), node("amp_2"), node("amp_3"));
    let sel = names(&["amp_1", "amp_2", "amp_3"]);
    assert!(linked_nodes(&[a.clone(), b.clone(), c], &sel, &node("amp_9")).is_empty());
    assert!(linked_nodes(&[a.clone(), b], &names(&["amp_1"]), &a).is_empty());
}

#[test]
fn a_selected_name_that_is_no_card_makes_the_selection_mixed() {
    let (a, b) = (node("amp_1"), node("amp_2"));
    let sel = names(&["amp_1", "amp_2", "chain_x"]);
    assert!(linked_nodes(&[a.clone(), b], &sel, &a).is_empty());
}

#[test]
fn a_baked_node_is_left_out_and_the_rest_still_move() {
    let (a, b) = (node("amp_1"), node("amp_2"));
    let frozen = SelectNode {
        frozen: Some("at".into()),
        ..node("amp_3")
    };
    let sel = names(&["amp_1", "amp_2", "amp_3"]);
    assert_eq!(linked_nodes(&[a.clone(), b, frozen], &sel, &a), names(&["amp_2"]));
}

fn at(module: &str, x: f64, kind: &str) -> SelectNode {
    SelectNode {
        kind: kind.into(),
        x,
        h: 50.0,
        ..node(module)
    }
}

#[test]
fn a_box_catches_every_module_card_it_touches_dragged_in_either_direction() {
    let nodes = [
        at("a", 0.0, "module"),
        at("b", 200.0, "module"),
        at("c", 400.0, "module"),
    ];
    let b = DragBox {
        x0: 150.0,
        y0: 10.0,
        x1: 250.0,
        y1: 20.0,
    };
    assert_eq!(nodes_in_box(&nodes, b), names(&["b"]));
    let back = DragBox {
        x0: 450.0,
        y0: 40.0,
        x1: 50.0,
        y1: -5.0,
    };
    assert_eq!(nodes_in_box(&nodes, back), names(&["a", "b", "c"]));
}

#[test]
fn a_box_leaves_out_what_is_not_a_module_card() {
    let nodes = [at("a", 0.0, "module"), at("lane", 0.0, "track"), at("out", 0.0, "sink")];
    let b = DragBox {
        x0: 0.0,
        y0: 0.0,
        x1: 50.0,
        y1: 50.0,
    };
    assert_eq!(nodes_in_box(&nodes, b), names(&["a"]));
}
