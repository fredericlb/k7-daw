// A glued lane row offers a `+` on each joined seam and one past its tail, into what the tail feeds.

use super::rows::{row, row_laid};
use super::*;
use crate::node::{EdgeInsert, Laid};
use crate::row_seams::{SEAM_END_ROOM, row_exit, row_seams};
use serde_json::json;

fn graph_of(use_: &[&str]) -> Value {
    let steps: Vec<Value> = use_.iter().map(|m| json!({"module": m})).collect();
    doc(
        json!({"lead": {"from": "gt_lead", "use": steps, "out": [{"to": "MIX", "gain": 1}]}}),
        json!(["gt_lead"]),
    )
}

fn glued_row(g: &Value, use_: &[&str], gap: Option<f64>) -> Laid {
    let mut o = json!({"asDrawn": true, "links": true, "track": "gt_lead"});
    if let Some(gap) = gap {
        o["gap"] = json!(gap);
    }
    row_laid(g, use_, o)
}

fn seams(r: &Laid, g: &Value, branch: bool) -> Value {
    let graph = graph(g);
    let exit = row_exit(r, r, &graph);
    serde_json::to_value(row_seams(r, exit, branch)).expect("seams")
}

fn between(a: &Value) -> Vec<Value> {
    list(&a["seams"]).into_iter().filter(|s| s["end"].is_null()).collect()
}

fn inserts(a: &Value) -> Vec<Value> {
    between(a).iter().map(|s| s["insert"].clone()).collect()
}

fn xw(n: &crate::node::Node) -> (f64, f64) {
    (n.x.unwrap_or(0.0), n.w)
}

#[test]
fn a_gap_of_zero_glues_the_cards_edge_to_edge() {
    let r = glued_row(&graph_of(&["amp", "verb"]), &["amp", "verb"], Some(0.0));
    for w in r.nodes.windows(2) {
        assert_eq!(xw(&w[1]).0, xw(&w[0]).0 + xw(&w[0]).1);
    }
}

#[test]
fn a_row_glued_onto_shared_borders_overlaps_each_card_by_a_pixel_and_still_finds_the_seams() {
    let g = graph_of(&["amp", "verb"]);
    let r = glued_row(&g, &["amp", "verb"], Some(-1.0));
    for w in r.nodes.windows(2) {
        assert_eq!(xw(&w[1]).0, xw(&w[0]).0 + xw(&w[0]).1 - 1.0);
    }
    let a = seams(&r, &g, false);
    assert_eq!(
        inserts(&a),
        [
            json!({"before": "gt_lead", "after": "amp"}),
            json!({"before": "amp", "after": "verb"})
        ]
    );
    assert_eq!(list(&a["glued"]).len(), r.edges.len());
}

#[test]
fn puts_a_seam_where_two_touching_cards_are_joined_and_marks_that_arrow_glued() {
    let g = graph_of(&["amp", "verb"]);
    let r = glued_row(&g, &["amp", "verb"], Some(0.0));
    let a = seams(&r, &g, false);
    assert_eq!(
        inserts(&a),
        [
            json!({"before": "gt_lead", "after": "amp"}),
            json!({"before": "amp", "after": "verb"})
        ]
    );
    assert_eq!(num(&list(&a["seams"])[1]["x"]), r.nodes[2].x.unwrap_or(0.0));
    assert_eq!(list(&a["glued"]).len(), r.edges.len());
}

#[test]
fn offers_nothing_on_a_row_with_air_between_its_cards() {
    let g = graph_of(&["amp", "verb"]);
    let r = glued_row(&g, &["amp", "verb"], None);
    assert!(between(&seams(&r, &g, false)).is_empty());
}

#[test]
fn adds_the_tail_past_the_last_card_and_room_for_it() {
    let g = graph_of(&["amp", "verb"]);
    let r = glued_row(&g, &["amp", "verb"], Some(0.0));
    let a = seams(&r, &g, false);
    let all = list(&a["seams"]);
    let tail = all.last().expect("a tail");
    assert_eq!(tail["end"], true);
    assert_eq!(tail["insert"], json!({"before": "verb", "after": "MIX"}));
    let last = r.nodes.last().expect("a card");
    assert!(num(&tail["x"]) > xw(last).0 + xw(last).1);
    assert_eq!(num(&a["width"]), r.width + SEAM_END_ROOM);
}

#[test]
fn the_exit_splices_before_a_mixer_the_rows_end_was_lifted_off_to() {
    let mut g = graph_of(&["amp", "strip"]);
    g["trackMixers"] = json!({"gt_lead": "strip"});
    let full = glued_row(&g, &["amp", "strip"], Some(0.0));
    let last = full.nodes.last().expect("the mixer").clone();
    assert!(last.track_face.is_some());
    let shown = Laid {
        nodes: full.nodes[..full.nodes.len() - 1].to_vec(),
        edges: full
            .edges
            .iter()
            .filter(|e| e.source != last.id && e.target != last.id)
            .cloned()
            .collect(),
        ..full.clone()
    };
    let exit = row_exit(&full, &shown, &graph(&g));
    assert_eq!(
        exit,
        Some(EdgeInsert {
            before: "amp".into(),
            after: "strip".into(),
            stack: None
        })
    );
}

fn stacked() -> Value {
    let mut g = graph_of(&["amp", "eq", "verb"]);
    g["stacks"] = json!([["amp", "eq"]]);
    g
}

fn stack_opts(gap: f64) -> Value {
    json!({"asDrawn": true, "links": true, "track": "gt_lead", "gap": gap, "stack": true})
}

#[test]
fn a_stack_in_the_row_is_one_column_its_cards_one_under_another_at_the_columns_width() {
    let r = list(&row(&stacked(), &["amp", "eq", "verb"], stack_opts(0.0))["nodes"]);
    let (amp, eq, verb) = (&r[1], &r[2], &r[3]);
    assert_eq!(eq["x"], amp["x"]);
    assert_eq!(num(&eq["y"]), num(&amp["y"]) + num(&amp["h"]));
    assert_eq!(eq["w"], amp["w"]);
    assert_eq!(num(&verb["x"]), num(&amp["x"]) + num(&amp["w"]));
}

#[test]
fn a_stack_in_the_row_glues_the_arrow_down_the_column_and_puts_a_branch_on_a_seam_whose_left_card_is_a_module() {
    let g = stacked();
    let r = row_laid(&g, &["amp", "eq", "verb"], stack_opts(0.0));
    let a = seams(&r, &g, true);
    let down = r
        .edges
        .iter()
        .find(|e| e.source.ends_with("amp") && e.target.ends_with("eq"))
        .expect("a cable");
    assert!(list(&a["glued"]).contains(&json!(down.id)));
    // The lane into the stack has no branch, a lane being a source; the stack's last card into verb has one.
    let pairs: Vec<(Value, Value)> = between(&a)
        .iter()
        .map(|s| (s["insert"].clone(), s["branch"].clone()))
        .collect();
    assert_eq!(
        pairs,
        [
            (json!({"before": "gt_lead", "after": "amp"}), Value::Null),
            (
                json!({"before": "eq", "after": "verb"}),
                json!({"before": "eq", "after": "eq", "stack": true})
            ),
        ]
    );
}

#[test]
fn a_stack_in_the_row_offers_no_branch_unless_asked() {
    let g = stacked();
    let r = row_laid(
        &g,
        &["amp", "eq", "verb"],
        json!({"asDrawn": true, "links": true, "gap": 0}),
    );
    assert!(
        list(&seams(&r, &g, false)["seams"])
            .iter()
            .all(|s| s["branch"].is_null())
    );
}
