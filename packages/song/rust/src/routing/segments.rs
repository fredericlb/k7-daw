// routing/segments.rs — a chain project read into the SEGMENT graph everything downstream reads, and each link's level.
// A reader would get wrong that the budget weighs the OUTPUT: a parallel step of N before one of M writes N×M edges.

use super::chain_file::{ChainFile, Item, Link, Step};
use super::scope::{ChainUnit, Scope, chain_units};
use super::types::{Position, Segments, TapFeed};
use super::{Budget, Capped, port_ref};
use indexmap::IndexMap;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// The segments of a chain project and each link's level, keyed `<from>><to>`.
#[derive(Debug, Default)]
pub struct Built {
    pub segments: Segments,
    pub levels: IndexMap<String, f64>,
}

/// The picture lane a chain file's own steps hold, if any; its subchains may be fade STRETCHES.
pub fn picture_lane_of(c: Option<&ChainFile>, is_picture: &dyn Fn(&str) -> bool) -> Option<String> {
    c?.steps.iter().flat_map(Step::items).find_map(|i| match i {
        Item::VideoTrack { video_track, .. } => Some(video_track.clone()),
        Item::Track { track, .. } if is_picture(track) => Some(track.clone()),
        _ => None,
    })
}

/// Where a link LANDS: a chain's first node (`#port` when named), a built-in end, or nothing; an explicit stack, depth-first.
fn targets_of(
    scope: &Scope,
    from: &ChainUnit,
    link: &Link,
    seen: &mut HashSet<String>,
    budget: &mut Budget,
) -> Result<Vec<String>, ()> {
    let mut out = Vec::new();
    let mut stack: Vec<(&ChainUnit, &Link)> = vec![(from, link)];
    while let Some((from, l)) = stack.pop() {
        if !budget.charge(1) {
            return Err(());
        }
        let Some(key) = scope.resolve_link(from, &l.chain) else {
            continue;
        };
        let Some(u) = scope.unit(&key) else {
            // A built-in end keeps the port it names too: `RENDER#audio` is not `RENDER`.
            out.push(port_ref(&key, l.port.as_deref()));
            continue;
        };
        if seen.contains(&u.key) {
            continue;
        }
        match u.steps.first() {
            Some(first) if !first.is_end() => {
                if let Some(head) = first.items().first() {
                    out.push(port_ref(head.name(), l.port.as_deref()));
                }
            }
            _ => {
                // A chain of links only passes its input straight on.
                seen.insert(u.key.clone());
                stack.extend(u.links().iter().rev().map(|x| (u, x)));
            }
        }
    }
    Ok(out)
}

/// The tap feeds of rack nodes whose taps are routed to subchains of their own file; each such subchain is also `linked`.
fn tap_feeds(scope: &Scope, called: &HashMap<&str, HashSet<&str>>, linked: &mut HashSet<String>) -> Vec<TapFeed> {
    let mut feeds = Vec::new();
    for u in &scope.units {
        for item in u.steps.iter().flat_map(Step::items) {
            let Item::Node(n) = item else { continue };
            for (tap, sub) in n.module.tap_outs.iter().flat_map(|k| k.0.iter()) {
                let Some(key) = scope.resolve_link(u, &sub.0) else {
                    continue;
                };
                let Some(target) = scope.unit(&key) else { continue };
                // Only a subchain that is not run in place: one called as a step has no run of its own to be fed.
                let Some(id) = target.sub else { continue };
                if called.get(target.parent).is_some_and(|c| c.contains(id)) {
                    continue;
                }
                let Some(head) = target
                    .steps
                    .iter()
                    .find(|x| !x.is_end())
                    .and_then(|s| s.items().first())
                else {
                    continue;
                };
                feeds.push(TapFeed {
                    node: item.name().to_owned(),
                    tap: tap.clone(),
                    to: head.name().to_owned(),
                });
                linked.insert(key);
            }
        }
    }
    feeds
}

/// The chain files as segments: one `modules` map, the bypass list, the buses, the tap feeds and each link's level.
pub fn to_segments(
    chains: &IndexMap<String, Arc<ChainFile>>,
    is_picture: &dyn Fn(&str) -> bool,
    positions: &IndexMap<String, Position>,
    budget: &mut Budget,
) -> Result<Built, Capped> {
    let scope = chain_units(chains);
    let called: HashMap<&str, HashSet<&str>> = chains.iter().map(|(n, c)| (n.as_str(), c.called_subchains())).collect();
    let mut linked: HashSet<String> = HashSet::new();
    for u in &scope.units {
        linked.extend(u.links().iter().filter_map(|l| scope.resolve_link(u, &l.chain)));
    }
    let feeds = tap_feeds(&scope, &called, &mut linked);
    let mut modules: IndexMap<String, Vec<String>> = IndexMap::new();
    // One scan of a file's own steps for its picture lane, however many subchains ask.
    let mut pictures: HashMap<&str, Option<String>> = HashMap::new();
    let (mut levels, mut bypassed, mut bus_only) = (IndexMap::new(), Vec::new(), Vec::new());
    for u in &scope.units {
        let capped = || Capped {
            file: format!("chains/{}.k7x", u.parent),
        };
        // A subchain RUN in place is a stretch of its parent, compiled there and not again on its own.
        if u.sub
            .is_some_and(|s| called.get(u.parent).is_some_and(|c| c.contains(s)))
        {
            continue;
        }
        let steps: Vec<&Step> = if u.sub.is_some() {
            u.steps.iter().collect()
        } else {
            u.file.expand_calls()
        };
        // A picture lane's subchain that nothing links and that links nowhere is a fade's STRETCH: it lands on the lane.
        let stretch_to = if u.sub.is_some() && !linked.contains(&u.key) && u.steps.last().is_none_or(|s| !s.is_end()) {
            pictures
                .entry(u.parent)
                .or_insert_with(|| picture_lane_of(chains.get(u.parent).map(|c| &**c), is_picture))
                .clone()
        } else {
            None
        };
        let start = if u.sub.is_none() { u.file.start } else { None };
        let visible = start.is_some()
            || linked.contains(&u.key)
            || stretch_to.is_some()
            || steps
                .iter()
                .any(|s| s.items().iter().any(|i| positions.contains_key(i.name())));
        if !visible {
            continue;
        }
        let node_steps: Vec<Vec<&str>> = steps
            .iter()
            .filter(|s| !s.is_end())
            .map(|s| s.items().iter().map(Item::name).collect())
            .collect();
        let mut ends: Vec<(String, Option<f64>)> = Vec::new();
        if let Some(t) = stretch_to {
            ends.push((t, None));
        } else {
            for l in u.links() {
                let mut seen = HashSet::from([u.key.clone()]);
                let found = targets_of(&scope, u, l, &mut seen, budget).map_err(|()| capped())?;
                ends.extend(found.into_iter().map(|t| (t, l.level)));
            }
        }
        let mut seq: Vec<Vec<&str>> = Vec::with_capacity(node_steps.len() + 1);
        if let Some(sum) = start.and_then(|s| s.sum()) {
            seq.push(vec![sum]);
        }
        seq.extend(node_steps.iter().filter(|s| !s.is_empty()).cloned());
        for (i, names) in seq.iter().enumerate() {
            let next = seq.get(i + 1);
            let width = next.map_or(ends.len(), Vec::len);
            let edges = names.len().checked_mul(width).and_then(|n| n.checked_add(names.len()));
            if !edges.is_some_and(|n| budget.charge(n)) {
                return Err(capped());
            }
            for n in names {
                match next {
                    Some(next) => modules.insert((*n).to_owned(), next.iter().map(|s| (*s).to_owned()).collect()),
                    None => {
                        for (t, level) in &ends {
                            if let Some(level) = level {
                                levels.insert(format!("{n}>{t}"), *level);
                            }
                        }
                        modules.insert((*n).to_owned(), ends.iter().map(|e| e.0.clone()).collect())
                    }
                };
            }
        }
        for s in &steps {
            for i in s.items().iter().filter(|i| i.bypass()) {
                if !budget.charge(1) {
                    return Err(capped());
                }
                bypassed.push(i.name().to_owned());
            }
        }
        if u.sub.is_none()
            && u.file.bus_only == Some(true)
            && let Some(first) = node_steps.first().and_then(|s| s.first())
        {
            bus_only.push((*first).to_owned());
        }
    }
    let segments = Segments {
        modules: Some(modules),
        bypassed: (!bypassed.is_empty()).then_some(bypassed),
        bus_only: (!bus_only.is_empty()).then_some(bus_only),
        tap_feeds: (!feeds.is_empty()).then_some(feeds),
        positions: (!positions.is_empty()).then(|| positions.clone()),
    };
    Ok(Built { segments, levels })
}
