//! k7x_scale.rs — a keyed list of thousands of items settles in linear time, and a replay past its budget falls back to a whole text.
//! What a reader gets wrong: the bounds are generous for a debug build; the quadratic they guard against takes minutes.

use std::time::{Duration, Instant};

use k7_sync::k7x::{apply_k7x, diff_k7x, item_segments, k7x_op_for, settle_k7x};
use k7x::{Bump, Node, parse_tree};
use serde_json::Value;

fn clip(notes: usize, shift: usize) -> String {
    let items: Vec<String> = (0..notes)
        .map(|i| format!("  {{ t: {i}, k: {}, v: 0.5 }},\n", 20 + shift + i % 40))
        .collect();
    format!(
        "clip({{\n  name: \"big\",\n  notes: [\n{}  ],\n}})\n",
        items.concat()
    )
}

fn within<T>(bound: Duration, f: impl FnOnce() -> T) -> T {
    let t = Instant::now();
    let out = f();
    assert!(
        t.elapsed() < bound,
        "took {:?}, bound {bound:?}",
        t.elapsed()
    );
    out
}

#[test]
fn keying_fifty_thousand_items_is_linear() {
    let text = clip(50_000, 0);
    let bump = Bump::new();
    let root = Node::Map(parse_tree(&text, &bump).unwrap());
    let Node::Map(m) = &root else { unreachable!() };
    let Node::Map(args) = &m.entries[0].value else {
        panic!("args")
    };
    let list = &args
        .entries
        .iter()
        .find(|e| e.key == "notes")
        .unwrap()
        .value;
    let segs = within(Duration::from_secs(3), || {
        item_segments(Some("notes"), Some(list))
    });
    assert_eq!(segs.unwrap().len(), 50_000);
}

#[test]
fn transposing_two_thousand_notes_keeps_the_text() {
    let (before, after) = (clip(2_000, 0), clip(2_000, 7));
    assert!(diff_k7x(&before, &after, true).unwrap().ops.len() >= 2_000);
    let op = within(Duration::from_secs(30), || {
        k7x_op_for("song.k7/nodes/clip.k7x", &before, &after)
    });
    // Either item ops that replay to the text, or None, which makes the change a whole text.
    if let Some(op) = op {
        let ops = op["ops"].as_array().unwrap();
        assert_eq!(apply_k7x(&before, ops).unwrap(), after);
    }
}

#[test]
fn settling_item_ops_against_a_big_list_is_fast() {
    let (before, after) = (clip(2_000, 0), clip(2_000, 7));
    let ops: Vec<Value> = diff_k7x(&before, &after, true).unwrap().ops;
    // A slice keeps the debug run short: the settle's locate cost is what scales with the list.
    let op = serde_json::json!({ "kind": "k7x", "file": "f.k7x", "ops": &ops[..60] });
    let settled = within(Duration::from_secs(30), || {
        settle_k7x(Some(&before), &op).unwrap()
    });
    assert!(settled.aside.is_empty());
    assert!(settled.apply.is_some());
}
