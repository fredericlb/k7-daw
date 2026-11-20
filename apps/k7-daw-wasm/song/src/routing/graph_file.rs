// routing/graph_file.rs — `graph.k7x` decoded into `Segments`: `runs` expanded first, the rest strict and located.
// A reader would get wrong that `runs` is read apart: each run adds its segments after `modules`' own list, deduplicated.

use super::BUDGET;
use super::hand::{R, fail, finite, get, list, names, non_empty, object, strict};
use super::types::{Position, Segments, TapFeed};
use crate::files::Finding;
use indexmap::IndexMap;
use k7x::{Bump, MapNode, Node, parse_tree};
use std::collections::HashSet;

const KEYS: [&str; 6] = ["modules", "bypassed", "bus_only", "tap_feeds", "positions", "runs"];

/// A destination: non-empty, and neither half of a `#port` empty.
fn dest(node: &Node) -> R<String> {
    let s = non_empty(node)?;
    if s.starts_with('#') || s.ends_with('#') {
        return fail(
            node.start(),
            "a port ref is `<module>#<port>` — neither half may be empty",
        );
    }
    Ok(s.to_owned())
}

fn modules_of(node: &Node) -> R<IndexMap<String, Vec<String>>> {
    let mut out = IndexMap::new();
    for e in object(node)?.entries {
        if e.key.is_empty() {
            return fail(e.key_start, "expected a non-empty key");
        }
        let dests = list(&e.value)?.iter().map(dest).collect::<R<Vec<_>>>()?;
        out.insert(e.key.to_owned(), dests);
    }
    Ok(out)
}

fn tap_feeds_of(node: &Node) -> R<Vec<TapFeed>> {
    list(node)?
        .iter()
        .map(|n| {
            let m = object(n)?;
            strict(&m, &["node", "tap", "to"])?;
            let field = |k: &str| -> R<String> {
                let e = super::hand::require(&m, k)?;
                non_empty(&e.value).map(str::to_owned)
            };
            Ok(TapFeed {
                node: field("node")?,
                tap: field("tap")?,
                to: field("to")?,
            })
        })
        .collect()
}

pub(crate) fn positions_of(node: &Node) -> R<IndexMap<String, Position>> {
    let mut out = IndexMap::new();
    for e in object(node)?.entries {
        if e.key.is_empty() {
            return fail(e.key_start, "expected a non-empty key");
        }
        let m = object(&e.value)?;
        strict(&m, &["x", "y"])?;
        let n = |k: &str| finite(&super::hand::require(&m, k)?.value);
        out.insert(e.key.to_owned(), Position { x: n("x")?, y: n("y")? });
    }
    Ok(out)
}

/// The runs as written: a list of lists of two or more names, the first a name and the rest destinations.
fn runs_of(runs: &Node) -> R<Vec<Vec<String>>> {
    let bad = |at: usize| -> R<Vec<Vec<String>>> { fail(at, "a list of runs, each a list of two or more node names") };
    let Node::List { items, .. } = runs else {
        return bad(runs.start());
    };
    let mut out = Vec::with_capacity(items.len());
    let mut weight = 0usize;
    for run in *items {
        let Node::List { items: names, .. } = run else {
            return bad(run.start());
        };
        if names.len() < 2 {
            return bad(run.start());
        }
        weight = weight.saturating_add(names.len());
        if weight > BUDGET {
            return fail(runs.start(), format!("`runs` holds more than {BUDGET} segments"));
        }
        let mut seen: Vec<String> = Vec::with_capacity(names.len());
        for (i, n) in names.iter().enumerate() {
            // Only a destination is a port ref: a run's first name is a key, as `modules`' keys are.
            let read = if i == 0 {
                non_empty(n).map(str::to_owned)
            } else {
                dest(n)
            };
            match read {
                Ok(s) => seen.push(s),
                Err(_) => return bad(n.start()),
            }
        }
        out.push(seen);
    }
    Ok(out)
}

/// The segments `runs` add: each name sends to the next, after what `modules` already says.
fn expand_runs(runs: &[Vec<String>], modules: &mut IndexMap<String, Vec<String>>) {
    let mut have: HashSet<(String, String)> = modules
        .iter()
        .flat_map(|(k, v)| v.iter().map(move |d| (k.clone(), d.clone())))
        .collect();
    for seen in runs {
        for pair in seen.windows(2) {
            let dests = modules.entry(pair[0].clone()).or_default();
            if have.insert((pair[0].clone(), pair[1].clone())) {
                dests.push(pair[1].clone());
            }
        }
    }
}

fn read(root: &MapNode) -> R<Segments> {
    // `runs` first: a bad one is located on itself, whatever else the file holds (decision 90).
    let runs = match get(root, "runs") {
        Some(e) => Some(runs_of(&e.value).map_err(|f| super::hand::Fail { at: e.key_start, ..f })?),
        None => None,
    };
    strict(root, &KEYS)?;
    let mut out = Segments::default();
    if let Some(e) = get(root, "modules") {
        out.modules = Some(modules_of(&e.value)?);
    }
    if let Some(runs) = runs {
        let mut modules = out.modules.take().unwrap_or_default();
        expand_runs(&runs, &mut modules);
        out.modules = Some(modules);
    }
    if let Some(e) = get(root, "bypassed") {
        out.bypassed = Some(names(&e.value)?);
    }
    if let Some(e) = get(root, "bus_only") {
        out.bus_only = Some(names(&e.value)?);
    }
    if let Some(e) = get(root, "tap_feeds") {
        out.tap_feeds = Some(tap_feeds_of(&e.value)?);
    }
    if let Some(e) = get(root, "positions") {
        out.positions = Some(positions_of(&e.value)?);
    }
    Ok(out)
}

/// `graph.k7x`'s text as its segments, or its first finding.
pub fn decode_graph(file: &str, text: &str) -> Result<Segments, Finding> {
    let bump = Bump::new();
    let root = parse_tree(text, &bump).map_err(|e| Finding::syntax(file, &e))?;
    read(&root).map_err(|f| f.finding(file, text))
}
