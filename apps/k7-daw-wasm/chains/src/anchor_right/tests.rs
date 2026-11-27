// A row that ENDS on its lane's mixer hands that card over to be pinned right; any other row is left whole.

use super::*;
use crate::graph::Graph;
use crate::module_row::{RowOptions, module_row};
use serde_json::json;

// The shipped strip's shape: one column of controls, and the engine's three stage sliders on its header.
fn graph(uses: &[&str]) -> Graph {
    let strip_ui = json!({"cols": 1, "rows": 7, "widgets": {"v": {"type": "fader", "col": 0, "row": 0, "rowSpan": 3, "bind": "Volume"}}});
    let steps: Vec<_> = uses
        .iter()
        .map(|m| {
            if *m == "strip" {
                json!({"module": m, "ui": strip_ui, "stage": ["@input-volume", "@mix", "@output-volume"]})
            } else {
                json!({"module": m})
            }
        })
        .collect();
    serde_json::from_value(json!({
        "chains": {"lead": {"from": "gt_lead", "use": steps, "out": [{"to": "MIX", "gain": 1}]}},
        "tracks": ["gt_lead"],
        "trackMixers": {"gt_lead": "strip"},
        "warnings": []
    }))
    .expect("a graph")
}

fn row(uses: &[&str]) -> Laid {
    let opts = RowOptions {
        as_drawn: Some(true),
        links: Some(true),
        track: Some("gt_lead".into()),
        ..RowOptions::default()
    };
    let modules: Vec<String> = uses.iter().map(|s| (*s).to_owned()).collect();
    module_row(&graph(uses), &modules, &opts)
}

fn modules(l: &Laid) -> Vec<Option<&str>> {
    l.nodes.iter().map(|n| n.module.as_deref()).collect()
}

#[test]
fn the_trailing_mixer_is_lifted_out_of_the_row_bare() {
    let split = split_trailing_mixer(&row(&["amp", "strip"])).expect("a split");
    assert_eq!(modules(&split.row), [Some("gt_lead"), Some("amp")]);
    assert_eq!(split.mixer.nodes.len(), 1);
    assert_eq!(split.mixer.nodes[0].module.as_deref(), Some("strip"));
    assert_eq!(split.mixer.nodes[0].bare, Some(true));
}

#[test]
fn every_cable_that_met_the_mixer_is_dropped_and_the_others_kept() {
    let split = split_trailing_mixer(&row(&["amp", "strip"])).expect("a split");
    let id = &split.mixer.nodes[0].id;
    assert!(!split.row.edges.iter().any(|e| &e.source == id || &e.target == id));
    assert!(!split.row.edges.is_empty());
    assert!(split.mixer.edges.is_empty());
}

#[test]
fn the_rows_height_is_kept_so_one_height_fit_draws_both_at_one_scale() {
    let whole = row(&["amp", "strip"]);
    let split = split_trailing_mixer(&whole).expect("a split");
    assert_eq!(split.mixer.height, whole.height);
    assert_eq!(split.mixer.top, whole.top);
    assert_eq!(split.mixer.nodes[0].h, whole.nodes[2].h);
}

#[test]
fn the_row_keeps_its_margin_and_the_mixer_sits_flush_right_with_none() {
    let whole = row(&["amp", "strip"]);
    let split = split_trailing_mixer(&whole).expect("a split");
    let margin = whole.nodes[0].x.unwrap_or(0.0);
    let amp = &whole.nodes[1];
    assert_eq!(split.row.width, amp.x.unwrap_or(0.0) + amp.w + margin);
    assert_eq!(split.mixer.width, split.mixer.nodes[0].w);
    assert_eq!(split.mixer.nodes[0].x, Some(0.0));
    assert!(whole.nodes[2].w > 0.0);
}

#[test]
fn the_bare_card_is_sized_by_its_panel() {
    let whole = row(&["amp", "strip"]);
    let split = split_trailing_mixer(&whole).expect("a split");
    let (w, _) = node_size(whole.nodes[2].ui.as_ref(), false);
    assert_eq!(split.mixer.nodes[0].w, w);
}

#[test]
fn a_row_whose_last_card_is_not_the_mixer_is_left_alone() {
    assert!(split_trailing_mixer(&row(&["strip", "amp"])).is_none());
}

#[test]
fn a_row_that_is_all_mixer_is_left_alone() {
    let only = module_row(
        &graph(&["strip"]),
        &["strip".to_owned()],
        &RowOptions {
            as_drawn: Some(true),
            ..RowOptions::default()
        },
    );
    assert!(split_trailing_mixer(&only).is_none());
}
