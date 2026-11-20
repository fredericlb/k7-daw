//! diff.rs — a k7x file's change as values set or deleted at key paths (`diffK7x` of `k7xOps.ts`).
//! Keyed lists (locate.rs) become item ops when their common items keep their order; otherwise a list is one value.
//! What a reader gets wrong: an op's key order is the wire's (`op, path, k7x, was, after`), since ops are `Value`s.

use std::collections::HashMap;

use k7x::{Bump, Node, parse_tree};
use serde_json::{Value, json};

use super::locate::{fragment, item_segments};

/// The ops turning one text into another, and whether any names a list item.
pub struct Diff {
    pub ops: Vec<Value>,
    pub keyed: bool,
}

struct Sources<'t> {
    a: &'t str,
    b: &'t str,
    lists: bool,
    keyed: bool,
}

fn with(path: &[String], seg: &str) -> Vec<String> {
    let mut p = path.to_vec();
    p.push(seg.to_owned());
    p
}

fn diff_node(a: &Node, b: &Node, path: &[String], src: &mut Sources, out: &mut Vec<Value>) {
    if fragment(src.a, a) == fragment(src.b, b) {
        return;
    }
    if let (Node::Map(ma), Node::Map(mb)) = (a, b) {
        let keys_b: Vec<&str> = mb.entries.iter().map(|e| e.key).collect();
        // A duplicate key keeps its last value, as a JS Map built from the entries does.
        let in_a: HashMap<&str, &Node> = ma.entries.iter().map(|e| (e.key, &e.value)).collect();
        for e in ma.entries {
            if !keys_b.contains(&e.key) {
                out.push(json!({
                    "op": "delete",
                    "path": with(path, e.key),
                    "was": fragment(src.a, &e.value),
                }));
            }
        }
        for e in mb.entries {
            match in_a.get(e.key) {
                Some(before) => diff_node(before, &e.value, &with(path, e.key), src, out),
                None => out.push(json!({
                    "op": "set",
                    "path": with(path, e.key),
                    "k7x": fragment(src.b, &e.value),
                    "was": null,
                })),
            }
        }
        return;
    }
    if src.lists && diff_list(a, b, path, src, out) {
        return;
    }
    out.push(json!({
        "op": "set",
        "path": path,
        "k7x": fragment(src.b, b),
        "was": fragment(src.a, a),
    }));
}

/// A keyed list's change as item ops, its common items in the same order; false when it must stay one value.
fn diff_list(a: &Node, b: &Node, path: &[String], src: &mut Sources, out: &mut Vec<Value>) -> bool {
    let name = path.last().map(String::as_str);
    let (Some(ka), Some(kb)) = (item_segments(name, Some(a)), item_segments(name, Some(b))) else {
        return false;
    };
    let (Node::List { items: ia, .. }, Node::List { items: ib, .. }) = (a, b) else {
        return false;
    };
    let in_b: HashMap<&str, usize> = kb
        .iter()
        .enumerate()
        .map(|(i, k)| (k.as_str(), i))
        .collect();
    let in_a: HashMap<&str, usize> = ka
        .iter()
        .enumerate()
        .map(|(i, k)| (k.as_str(), i))
        .collect();
    let common_a = ka.iter().filter(|k| in_b.contains_key(k.as_str()));
    let common_b = kb.iter().filter(|k| in_a.contains_key(k.as_str()));
    if common_a.zip(common_b).any(|(x, y)| x != y) {
        return false;
    }
    src.keyed = true;
    for (i, k) in ka.iter().enumerate() {
        if !in_b.contains_key(k.as_str()) {
            out.push(json!({
                "op": "delete",
                "path": with(path, k),
                "was": fragment(src.a, &ia[i]),
            }));
        }
    }
    for (i, k) in kb.iter().enumerate() {
        if let Some(&j) = in_a.get(k.as_str()) {
            diff_node(&ia[j], &ib[i], &with(path, k), src, out);
        }
    }
    for (i, k) in kb.iter().enumerate() {
        if in_a.contains_key(k.as_str()) {
            continue;
        }
        let after = if i == 0 {
            Value::Null
        } else {
            Value::from(kb[i - 1].as_str())
        };
        out.push(json!({
            "op": "set",
            "path": with(path, k),
            "k7x": fragment(src.b, &ib[i]),
            "was": null,
            "after": after,
        }));
    }
    true
}

/// The ops turning `before` into `after`; None when either does not read.
pub fn diff_k7x(before: &str, after: &str, lists: bool) -> Option<Diff> {
    let (bump_a, bump_b) = (Bump::new(), Bump::new());
    let a = Node::Map(parse_tree(before, &bump_a).ok()?);
    let b = Node::Map(parse_tree(after, &bump_b).ok()?);
    let mut src = Sources {
        a: before,
        b: after,
        lists,
        keyed: false,
    };
    let mut ops = Vec::new();
    diff_node(&a, &b, &[], &mut src, &mut ops);
    Some(Diff {
        ops,
        keyed: src.keyed,
    })
}
