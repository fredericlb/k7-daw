// The layout over built graphs: one row per rank, one line of tracks, the pins, and a receiver kept below what feeds it.

use super::*;
use serde_json::json;
use std::collections::BTreeMap;

fn f(n: &Value, k: &str) -> f64 {
    num(&n[k])
}

fn none() -> Value {
    Value::Null
}

fn deep_and_shallow() -> Built {
    built(
        &doc(
            json!({
                "long": {"from": "vox", "use": [{"module": "comp"}, {"module": "eq"}, {"module": "verb"}], "out": [{"to": "OUT", "gain": 1}]},
                "short": {"from": "bass", "use": [{"module": "drive"}], "out": [{"to": "OUT", "gain": 1}]}
            }),
            json!(["vox", "bass"]),
        ),
        json!({}),
    )
}

fn rank_of(b: &Built, id: &str) -> Option<i64> {
    b.node(id)["rank"].as_f64().map(|r| r as i64)
}

#[test]
fn gives_each_rank_one_y() {
    let b = deep_and_shallow();
    let l = list(&b.laid(none(), none())["nodes"]);
    let mut rows: BTreeMap<i64, HashSet<String>> = BTreeMap::new();
    for n in &l {
        if let Some(r) = rank_of(&b, &text(&n["id"])) {
            rows.entry(r).or_default().insert(n["y"].to_string());
        }
    }
    assert!(rows.values().all(|ys| ys.len() == 1));
}

#[test]
fn puts_the_short_chains_module_at_the_top_beside_the_deep_ones_first() {
    let l = list(&deep_and_shallow().laid(none(), none())["nodes"]);
    let y = |m: &str| f(module(&l, m), "y");
    assert_eq!(y("drive"), y("comp"));
    assert!(y("drive") < y("verb"));
}

#[test]
fn keeps_the_rows_in_order_and_never_lets_one_sit_on_the_one_above() {
    let b = deep_and_shallow();
    let l = list(&b.laid(none(), none())["nodes"]);
    let ranks: std::collections::BTreeSet<i64> = l.iter().filter_map(|n| rank_of(&b, &text(&n["id"]))).collect();
    let mut prev_bottom = f64::NEG_INFINITY;
    for r in ranks {
        let of: Vec<&Value> = l.iter().filter(|n| rank_of(&b, &text(&n["id"])) == Some(r)).collect();
        assert!(of.iter().map(|n| f(n, "y")).fold(f64::INFINITY, f64::min) >= prev_bottom);
        prev_bottom = of
            .iter()
            .map(|n| f(n, "y") + f(n, "h"))
            .fold(f64::NEG_INFINITY, f64::max);
    }
}

#[test]
fn parts_two_nodes_the_collapse_would_have_stacked() {
    let l = list(&deep_and_shallow().laid(none(), none())["nodes"]);
    for (i, a) in l.iter().enumerate() {
        for b in &l[i + 1..] {
            let hit = f(a, "x") < f(b, "x") + f(b, "w")
                && f(b, "x") < f(a, "x") + f(a, "w")
                && f(a, "y") < f(b, "y") + f(b, "h")
                && f(b, "y") < f(a, "y") + f(a, "h");
            assert!(!hit, "{} overlaps {}", a["id"], b["id"]);
        }
    }
}

#[test]
fn reports_an_extent_that_covers_where_the_nodes_actually_are() {
    let laid = deep_and_shallow().laid(none(), none());
    let l = list(&laid["nodes"]);
    assert!(f(&laid, "width") >= l.iter().map(|n| f(n, "x") + f(n, "w")).fold(f64::MIN, f64::max));
    assert!(f(&laid, "height") >= l.iter().map(|n| f(n, "y") + f(n, "h")).fold(f64::MIN, f64::max));
}

fn uneven_tracks() -> Built {
    built(
        &doc(
            json!({
                "long": {"from": "vox", "use": [{"module": "comp"}, {"module": "eq"}, {"module": "mx-vox"}], "out": [{"to": "OUT", "gain": 1}]},
                "short": {"from": "bass", "use": [{"module": "mx-bass"}], "out": [{"to": "OUT", "gain": 1}]}
            }),
            json!(["vox", "bass"]),
        ),
        json!({}),
    )
}

#[test]
fn puts_every_track_on_one_line_at_the_top_with_the_x_left_alone() {
    let l = list(&uneven_tracks().laid(none(), none())["nodes"]);
    let tracks: Vec<&Value> = l.iter().filter(|n| n["kind"] == "track").collect();
    assert_eq!(tracks.len(), 2);
    assert_eq!(
        tracks.iter().map(|n| n["y"].to_string()).collect::<HashSet<_>>().len(),
        1
    );
    let top = f(tracks[0], "y");
    assert!(l.iter().filter(|n| n["kind"] == "module").all(|n| f(n, "y") >= top));
    // Two tracks, two different columns: this aligns a line, it does not build a rank.
    assert_eq!(
        tracks.iter().map(|n| n["x"].to_string()).collect::<HashSet<_>>().len(),
        2
    );
}

fn vox_lane() -> Built {
    built(
        &doc(
            json!({"long": {"from": "vox", "use": [{"module": "comp"}, {"module": "eq"}], "out": [{"to": "OUT", "gain": 1}]}}),
            json!(["vox"]),
        ),
        json!({}),
    )
}

#[test]
fn a_pinned_module_sits_where_its_pin_says_and_what_is_above_keeps_the_layouts_answer() {
    let b = vox_lane();
    let free = list(&b.laid(none(), none())["nodes"]);
    let l = list(&b.laid(json!({"comp": {"x": 640, "y": 480}}), none())["nodes"]);
    let comp = module(&l, "comp");
    assert_eq!((f(comp, "x"), f(comp, "y")), (640.0, 480.0));
    let vox = l.iter().find(|n| n["label"] == "vox").expect("vox");
    let was = card(&free, &text(&vox["id"]));
    assert_eq!((f(vox, "x"), f(vox, "y")), (f(was, "x"), f(was, "y")));
    // What it feeds follows it down: a cable may not climb.
    assert!(f(module(&l, "eq"), "y") >= 480.0 + f(comp, "h"));
}

#[test]
fn a_pinned_node_still_counts_toward_the_graphs_measured_size() {
    let laid = vox_lane().laid(json!({"comp": {"x": 2000, "y": 1500}}), none());
    assert!(f(&laid, "width") > 2000.0 && f(&laid, "height") > 1500.0);
}

#[test]
fn a_graph_nobody_dragged_begins_at_the_origin() {
    let laid = vox_lane().laid(none(), none());
    assert_eq!((f(&laid, "left"), f(&laid, "top")), (0.0, 0.0));
}

#[test]
fn a_pin_above_the_first_track_is_reported_so_the_camera_can_still_frame_it() {
    let laid = vox_lane().laid(json!({"vox": {"x": -200, "y": -300}}), none());
    assert!(f(&laid, "top") <= -300.0 && f(&laid, "left") <= -200.0);
    assert!(f(&laid, "height") - f(&laid, "top") > 300.0);
}

#[test]
fn a_pin_for_a_name_the_graph_does_not_draw_changes_nothing() {
    let b = vox_lane();
    let at = |l: &Value| -> Vec<(f64, f64)> { list(&l["nodes"]).iter().map(|n| (f(n, "x"), f(n, "y"))).collect() };
    assert_eq!(
        at(&b.laid(json!({"ghost": {"x": 1, "y": 1}}), none())),
        at(&b.laid(none(), none()))
    );
}

fn memory(l: &Value) -> Value {
    let m: serde_json::Map<String, Value> = list(&l["nodes"])
        .iter()
        .map(|n| (text(&n["id"]), json!({"x": n["x"], "y": n["y"]})))
        .collect();
    Value::Object(m)
}

#[test]
fn prev_holds_a_drawn_node_in_place_through_an_edit() {
    let b = vox_lane();
    let prev = memory(&b.laid(none(), none()));
    // The same graph minus one cable: the kind of change that would re-rank everything.
    let fewer = b.laid_without(none(), prev.clone(), Some(b.edges.len() - 1));
    for n in list(&fewer["nodes"]) {
        assert_eq!(json!({"x": n["x"], "y": n["y"]}), prev[text(&n["id"])]);
    }
}

#[test]
fn a_pin_outranks_prev_the_file_outranks_the_session() {
    let b = vox_lane();
    let prev = memory(&b.laid(none(), none()));
    let l = list(&b.laid(json!({"comp": {"x": 900, "y": 700}}), prev)["nodes"]);
    let comp = module(&l, "comp");
    assert_eq!((f(comp, "x"), f(comp, "y")), (900.0, 700.0));
}

fn below_what_feeds_it(laid: &Value) {
    let l = list(&laid["nodes"]);
    for e in list(&laid["edges"]) {
        let (from, to) = (card(&l, &text(&e["source"])), card(&l, &text(&e["target"])));
        if from["id"] != to["id"] {
            assert!(
                f(to, "y") >= f(from, "y") + f(from, "h"),
                "{} -> {}",
                from["label"],
                to["label"]
            );
        }
    }
}

#[test]
fn a_receiver_stays_below_what_feeds_it_with_no_pins_at_all() {
    below_what_feeds_it(&vox_lane().laid(none(), none()));
}

#[test]
fn a_pin_that_would_put_a_module_above_its_input_is_pushed_back_down() {
    below_what_feeds_it(&vox_lane().laid(json!({"comp": {"x": 40, "y": -900}}), none()));
}

#[test]
fn a_pin_on_the_same_line_as_its_input_is_pushed_down_too() {
    let b = vox_lane();
    let vox = list(&b.laid(none(), none())["nodes"])
        .into_iter()
        .find(|n| n["label"] == "vox")
        .expect("vox");
    let l = list(&b.laid(json!({"comp": {"x": 400, "y": vox["y"]}}), none())["nodes"]);
    assert!(f(module(&l, "comp"), "y") >= f(&vox, "y") + f(&vox, "h"));
}

#[test]
fn a_pushed_pin_moves_no_further_than_the_rule_needs_and_never_sideways() {
    let l = list(&vox_lane().laid(json!({"comp": {"x": 640, "y": -900}}), none())["nodes"]);
    let (comp, vox) = (module(&l, "comp"), l.iter().find(|n| n["label"] == "vox").expect("vox"));
    assert_eq!(f(comp, "x"), 640.0);
    assert_eq!(f(comp, "y"), f(vox, "y") + f(vox, "h"));
}

#[test]
fn a_pin_that_already_obeys_stays_exactly_where_it_was_put() {
    let l = list(&vox_lane().laid(json!({"comp": {"x": 640, "y": 4000}}), none())["nodes"]);
    let comp = module(&l, "comp");
    assert_eq!((f(comp, "x"), f(comp, "y")), (640.0, 4000.0));
}
