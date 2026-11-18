// chain_segments.rs — a chain project as the segments a `graph.k7x` would hold, through song's `to_segments`.
// A reader would get wrong that the pins are the file's own: they ride through as written, so the layout write can tell a change.

use crate::chain_project::ChainProject;
use crate::error::{Res, fail};
use indexmap::IndexMap;
use k7x::edit::{StringifyOptions, Value, stringify};
use song::routing::{Budget, Position, to_segments as song_to_segments};

/// The segments of a chain project as data in the graph file's key order, and each link's level keyed `<from>><to>`.
///
/// # Errors
/// A project whose segments pass the routing's budget.
pub fn to_segments(cp: &ChainProject, positions: &Value) -> Res<(Value, IndexMap<String, f64>)> {
    let pins: IndexMap<String, Position> = match positions {
        Value::Map(m) => m
            .iter()
            .filter_map(|(k, p)| match (p.get("x"), p.get("y")) {
                (Some(Value::Num(x)), Some(Value::Num(y))) => Some((k.clone(), Position { x: *x, y: *y })),
                _ => None,
            })
            .collect(),
        _ => IndexMap::new(),
    };
    let is_picture = |lane: &str| cp.nodes.get(lane).is_some_and(|n| n.picture);
    let built = match song_to_segments(&cp.typed, &is_picture, &pins, &mut Budget::default()) {
        Ok(b) => b,
        Err(c) => return fail(format!("{}: the routing is too large to read", c.file)),
    };
    let s = built.segments;
    let strs = |v: &[String]| Value::List(v.iter().map(|x| Value::Str(x.clone())).collect());
    let mut graph: Vec<(String, Value)> = vec![];
    let modules = s.modules.unwrap_or_default();
    graph.push((
        "modules".to_owned(),
        Value::Map(modules.iter().map(|(k, v)| (k.clone(), strs(v))).collect()),
    ));
    if let Some(b) = s.bypassed {
        graph.push(("bypassed".to_owned(), strs(&b)));
    }
    if let Some(b) = s.bus_only {
        graph.push(("bus_only".to_owned(), strs(&b)));
    }
    if let Some(feeds) = s.tap_feeds {
        let one = |f: song::routing::TapFeed| {
            Value::Map(vec![
                ("node".to_owned(), Value::Str(f.node)),
                ("tap".to_owned(), Value::Str(f.tap)),
                ("to".to_owned(), Value::Str(f.to)),
            ])
        };
        graph.push((
            "tap_feeds".to_owned(),
            Value::List(feeds.into_iter().map(one).collect()),
        ));
    }
    if matches!(positions, Value::Map(m) if !m.is_empty()) {
        graph.push(("positions".to_owned(), positions.clone()));
    }
    Ok((Value::Map(graph), built.levels))
}

/// The chain project as the segments k7x text a `graph.k7x` would hold: what every graph writer reads.
///
/// # Errors
/// What `to_segments` fails with, or a number the text cannot spell.
pub fn chain_segments_text(cp: &ChainProject, positions: &Value) -> Res<String> {
    let (graph, _) = to_segments(cp, positions)?;
    Ok(stringify(&graph, &StringifyOptions::default())?)
}
