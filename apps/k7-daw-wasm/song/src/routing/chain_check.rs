// routing/chain_check.rs — the refinements a chain file makes after its fields read, in the order the schema raises them.
// A reader would get wrong that a file reports ONE finding: the first of these, then the first of its items' module checks.

use super::chain_file::{CHAIN_SINKS, ChainFile, Item, Step};
use crate::files::Check;
use crate::files::keys;
use k7x::PathSeg;
use std::collections::HashSet;

type Found = (Vec<PathSeg>, String);

/// The rules every list of steps follows: one node first, links last. Paths are relative to the list.
fn step_issues(steps: &[Step]) -> Option<(usize, String)> {
    if matches!(steps.first(), Some(Step::Parallel(_))) {
        return Some((
            0,
            "the first step is one node — its ports are what the chain is linked into".to_owned(),
        ));
    }
    let last = steps.len().saturating_sub(1);
    steps
        .iter()
        .enumerate()
        .find(|(i, s)| s.is_end() && *i != last)
        .map(|(i, _)| (i, "a step of links ends the chain — it can only be the last".to_owned()))
}

fn has_lane(steps: &[Step]) -> bool {
    steps.iter().any(|s| s.items().iter().any(|i| i.lane().is_some()))
}

fn steps_path(prefix: &[PathSeg], i: usize) -> Vec<PathSeg> {
    let mut p = prefix.to_vec();
    p.push(PathSeg::Index(i));
    p
}

/// The first rule a chain file breaks, with the key path it is about.
pub(super) fn check(c: &ChainFile) -> Option<Found> {
    if let Some(found) = levels_check(c) {
        return Some(found);
    }
    if c.start.is_some() && c.ports.as_ref().is_some_and(|p| !p.is_empty()) {
        return Some((
            keys(&["ports"]),
            "a chain with a start is linked into by nothing, so it has no ports".to_owned(),
        ));
    }
    if let Some((i, message)) = step_issues(&c.steps) {
        return Some((steps_path(&keys(&["steps"]), i), message));
    }
    if c.start != Some(super::chain_file::Start::Own) && has_lane(&c.steps) {
        return Some((
            keys(&["steps"]),
            "a lane is a source: only a chain with `start: true` holds one".to_owned(),
        ));
    }
    let subs = c.subchains.as_deref().unwrap_or_default();
    let ids: HashSet<&str> = subs.iter().map(|s| s.id.as_str()).collect();
    let mut called: HashSet<&str> = HashSet::new();
    for (i, s) in c.steps.iter().enumerate() {
        let Step::Call { sub } = s else { continue };
        let mut path = steps_path(&keys(&["steps"]), i);
        path.push(PathSeg::Key("sub".to_owned()));
        if i == 0 {
            return Some((path, "the first step is one node, not a subchain run".to_owned()));
        }
        if !ids.contains(sub.as_str()) {
            return Some((path, format!("this chain has no subchain \"{sub}\"")));
        }
        if !called.insert(sub.as_str()) {
            return Some((path, format!("subchain \"{sub}\" is run twice")));
        }
    }
    let linked: HashSet<&str> = std::iter::once(&c.steps)
        .chain(subs.iter().map(|s| &s.steps))
        .filter_map(|steps| steps.last().and_then(Step::end))
        .flat_map(|e| e.to.iter().map(|l| l.chain.as_str()))
        .collect();
    let mut seen: HashSet<&str> = HashSet::new();
    for (j, sc) in subs.iter().enumerate() {
        let base = vec![PathSeg::Key("subchains".to_owned()), PathSeg::Index(j)];
        let issue = |rest: Vec<PathSeg>, m: String| -> Option<Found> {
            let mut p = base.clone();
            p.extend(rest);
            Some((p, m))
        };
        let id_path = || keys(&["id"]);
        let steps_key = || keys(&["steps"]);
        if !seen.insert(sc.id.as_str()) {
            return issue(id_path(), format!("two subchains are called \"{}\"", sc.id));
        }
        if CHAIN_SINKS.contains(&sc.id.as_str()) {
            return issue(
                id_path(),
                format!("\"{}\" is a built-in end, not a subchain's name", sc.id),
            );
        }
        if let Some((i, m)) = step_issues(&sc.steps) {
            return issue(steps_path(&steps_key(), i), m);
        }
        if has_lane(&sc.steps) {
            return issue(steps_key(), "a lane is a source: a subchain cannot hold one".to_owned());
        }
        if let Some(i) = sc.steps.iter().position(Step::is_call) {
            return issue(
                steps_path(&steps_key(), i),
                "a subchain runs no subchain: they are one level deep".to_owned(),
            );
        }
        if called.contains(sc.id.as_str()) {
            if !sc.steps.iter().any(|s| !s.items().is_empty()) {
                return issue(
                    steps_key(),
                    "a subchain run in place holds at least one node".to_owned(),
                );
            }
            if sc.steps.last().is_some_and(Step::is_end) {
                return issue(
                    steps_key(),
                    "a subchain run in place returns to its parent: it ends with no `to:`".to_owned(),
                );
            }
            if linked.contains(sc.id.as_str()) {
                return issue(
                    id_path(),
                    format!("subchain \"{}\" is run in place, so nothing links to it", sc.id),
                );
            }
        }
        if let Some(end) = sc.steps.last().and_then(Step::end) {
            for (k, l) in end.to.iter().enumerate() {
                if !ids.contains(l.chain.as_str()) && !CHAIN_SINKS.contains(&l.chain.as_str()) {
                    let mut p = steps_path(&steps_key(), sc.steps.len() - 1);
                    p.extend([
                        PathSeg::Key("to".to_owned()),
                        PathSeg::Index(k),
                        PathSeg::Key("chain".to_owned()),
                    ]);
                    return issue(
                        p,
                        format!(
                            "a subchain links only to its parent's subchains or a built-in end, not \"{}\"",
                            l.chain
                        ),
                    );
                }
            }
        }
    }
    modules_check(c)
}

/// The rule the schema checks on an end step in place: several links each carry a `level`.
fn levels_check(c: &ChainFile) -> Option<Found> {
    let lists = c.step_lists();
    for (list_at, (sub, steps)) in lists.enumerate() {
        let base = match sub {
            None => keys(&["steps"]),
            Some(_) => {
                let mut p = vec![PathSeg::Key("subchains".to_owned()), PathSeg::Index(list_at - 1)];
                p.push(PathSeg::Key("steps".to_owned()));
                p
            }
        };
        for (s, step) in steps.iter().enumerate() {
            if let Step::End(e) = step
                && e.to.len() >= 2
                && e.to.iter().any(|l| l.level.is_none())
            {
                return Some((
                    steps_path(&base, s),
                    "a step linking several chains gives each link a `level`".to_owned(),
                ));
            }
        }
    }
    None
}

/// Each node item checked as a module file would be, at the item's own path.
fn modules_check(c: &ChainFile) -> Option<Found> {
    for (list_at, (sub, steps)) in c.step_lists().enumerate() {
        let base = match sub {
            None => keys(&["steps"]),
            Some(_) => vec![
                PathSeg::Key("subchains".to_owned()),
                PathSeg::Index(list_at - 1),
                PathSeg::Key("steps".to_owned()),
            ],
        };
        for (s, step) in steps.iter().enumerate() {
            let parallel = matches!(step, Step::Parallel(_));
            for (k, item) in step.items().iter().enumerate() {
                let Item::Node(n) = item else { continue };
                let Some((rest, message)) = n.module.check() else {
                    continue;
                };
                let mut p = steps_path(&base, s);
                if parallel {
                    p.push(PathSeg::Index(k));
                }
                p.extend(rest);
                return Some((p, message));
            }
        }
    }
    None
}
