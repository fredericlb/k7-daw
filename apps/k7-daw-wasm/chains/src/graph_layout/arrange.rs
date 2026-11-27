// arrange.rs — what the layout does to the engine's answer: ranks snapped to rows, the master run set aside, frames cleared.
// A reader would get wrong that these never move a pinned card: the pins are applied after the rows and before the pushes (docs/graphBuild.md).

use crate::curve_placement::{Box2, row_sort_keys};
use crate::geom::MIX_OUT;
use crate::node::{Edge, Kind, Node, Pt, Signal};
use indexmap::IndexMap;
use std::collections::{HashMap, HashSet};

/// The gap between one rank's row and the next, and the stack's below.
pub const RANK_SEP: f64 = 70.0;
/// The gap between two siblings side by side, which the row sweep re-uses to part a collision.
pub const NODE_SEP: f64 = 60.0;

pub fn x(n: &Node) -> f64 {
    n.x.unwrap_or(0.0)
}

pub fn y(n: &Node) -> f64 {
    n.y.unwrap_or(0.0)
}

pub fn boxes(nodes: &[Node]) -> Vec<Box2> {
    nodes
        .iter()
        .map(|n| Box2 {
            id: n.id.clone(),
            x: x(n),
            y: y(n),
            w: n.w,
            h: n.h,
        })
        .collect()
}

/// A member of a box carries the box's name; an empty name is no box, as in the page.
pub fn member_of(n: &Node) -> Option<&str> {
    n.assembly.as_deref().filter(|a| !a.is_empty())
}

fn min_of(it: impl Iterator<Item = f64>) -> f64 {
    it.fold(f64::INFINITY, f64::min)
}

fn max_of(it: impl Iterator<Item = f64>) -> f64 {
    it.fold(f64::NEG_INFINITY, f64::max)
}

/// Nodes overlapping an OPEN frame step aside by a constant shift per side, recorded in `pushed`.
pub fn push_aside_frames(mut nodes: Vec<Node>) -> Vec<Node> {
    let frames: Vec<usize> = (0..nodes.len())
        .filter(|&i| nodes[i].kind == Kind::Assembly && nodes[i].assembly_of.as_ref().is_some_and(|a| a.open))
        .collect();
    if frames.is_empty() {
        return nodes;
    }
    let mut moved: HashMap<String, f64> = HashMap::new();
    let at = |n: &Node, moved: &HashMap<String, f64>| x(n) + moved.get(&n.id).copied().unwrap_or(0.0);
    for &fi in &frames {
        let f = nodes[fi].clone();
        let left = x(&f) - NODE_SEP;
        let right = x(&f) + f.w + NODE_SEP;
        let mid = x(&f) + f.w / 2.0;
        // A member rides with its own frame, and a frame is not pushed by itself.
        let hit: Vec<&Node> = nodes
            .iter()
            .enumerate()
            .filter(|(i, n)| *i != fi && member_of(n).is_none() && y(n) + n.h > y(&f) && y(n) < y(&f) + f.h)
            .map(|(_, n)| n)
            .filter(|n| at(n, &moved) + n.w > left && at(n, &moved) < right)
            .collect();
        if hit.is_empty() {
            continue;
        }
        // Nearer side, so nothing is dragged across the frame it was beside.
        let (go_right, go_left): (Vec<&Node>, Vec<&Node>) =
            hit.into_iter().partition(|n| at(n, &moved) + n.w / 2.0 >= mid);
        let mut shifts: Vec<(String, f64)> = Vec::new();
        if !go_right.is_empty() {
            let dx = right - min_of(go_right.iter().map(|n| at(n, &moved)));
            if dx > 0.0 {
                shifts.extend(go_right.iter().map(|n| (n.id.clone(), dx)));
            }
        }
        if !go_left.is_empty() {
            let dx = max_of(go_left.iter().map(|n| at(n, &moved) + n.w)) - left;
            if dx > 0.0 {
                shifts.extend(go_left.iter().map(|n| (n.id.clone(), -dx)));
            }
        }
        for (id, dx) in shifts {
            *moved.entry(id).or_insert(0.0) += dx;
        }
    }
    for n in &mut nodes {
        if let Some(&dx) = moved.get(&n.id)
            && dx != 0.0
        {
            n.pushed.get_or_insert(Pt { x: x(n), y: y(n) });
            n.x = Some(x(n) + dx);
        }
    }
    nodes
}

/// Put every node on the LINE OF ITS OWN RANK, then sweep each row for collisions.
pub fn snap_to_rank_rows(
    mut placed: Vec<Node>,
    rank_of: &HashMap<String, f64>,
    curves: &IndexMap<String, String>,
) -> Vec<Node> {
    let mut ranks: Vec<f64> = rank_of.values().copied().collect();
    ranks.sort_by(f64::total_cmp);
    ranks.dedup();
    if ranks.is_empty() {
        return placed;
    }
    let mut row_y: HashMap<u64, f64> = HashMap::new();
    let mut floor = f64::NEG_INFINITY;
    for &r in &ranks {
        let of: Vec<&Node> = placed.iter().filter(|n| rank_of.get(&n.id) == Some(&r)).collect();
        if of.is_empty() {
            continue;
        }
        let top = min_of(of.iter().map(|n| y(n))).max(floor);
        row_y.insert(r.to_bits(), top);
        floor = top + max_of(of.iter().map(|n| n.h)) + RANK_SEP;
    }
    for n in &mut placed {
        if let Some(&ry) = rank_of.get(&n.id).and_then(|r| row_y.get(&r.to_bits())) {
            n.y = Some(ry);
        }
    }
    let keys = row_sort_keys(&boxes(&placed), curves);
    let key: HashMap<&str, f64> = placed.iter().zip(&keys).map(|(n, k)| (n.id.as_str(), *k)).collect();
    let mut moved: HashMap<String, f64> = HashMap::new();
    for &r in &ranks {
        let mut row: Vec<&Node> = placed.iter().filter(|n| rank_of.get(&n.id) == Some(&r)).collect();
        row.sort_by(|a, b| key[a.id.as_str()].total_cmp(&key[b.id.as_str()]));
        let mut right = f64::NEG_INFINITY;
        for n in row {
            let nx = x(n).max(right);
            if nx != x(n) {
                moved.insert(n.id.clone(), nx);
            }
            right = nx + n.w + NODE_SEP;
        }
    }
    for n in &mut placed {
        if let Some(&nx) = moved.get(&n.id) {
            n.x = Some(nx);
        }
    }
    placed
}

/// The MASTER RUN, the sum and what follows it, shifted as a block right of the rest with the sum on the track row.
pub fn master_run_aside(mut nodes: Vec<Node>, edges: &[Edge]) -> Vec<Node> {
    let Some(sum) = nodes.iter().find(|n| n.kind == Kind::Sink && n.label == MIX_OUT) else {
        return nodes;
    };
    let sum_y = y(sum);
    let mut run: HashSet<String> = HashSet::from([sum.id.clone()]);
    // The sum is ranked where a SOURCE is, so this walks DOWN.
    loop {
        let before = run.len();
        for e in edges {
            if run.contains(&e.source) {
                run.insert(e.target.clone());
            }
        }
        if run.len() == before {
            break;
        }
    }
    let others: Vec<&Node> = nodes.iter().filter(|n| !run.contains(&n.id)).collect();
    // A graph that is nothing BUT the master run has no rest for it to stand beside.
    if others.is_empty() {
        return nodes;
    }
    let mine: Vec<&Node> = nodes.iter().filter(|n| run.contains(&n.id)).collect();
    let dx = max_of(others.iter().map(|n| x(n) + n.w)) + NODE_SEP - min_of(mine.iter().map(|n| x(n)));
    let lanes: Vec<&&Node> = others.iter().filter(|n| n.kind == Kind::Track).collect();
    let dy = if lanes.is_empty() {
        max_of(others.iter().map(|n| y(n) + n.h)) + RANK_SEP - min_of(mine.iter().map(|n| y(n)))
    } else {
        min_of(lanes.iter().map(|n| y(n))) - sum_y
    };
    if dx == 0.0 && dy == 0.0 {
        return nodes;
    }
    for n in &mut nodes {
        if run.contains(&n.id) {
            n.x = Some(x(n) + dx);
            n.y = Some(y(n) + dy);
        }
    }
    nodes
}

/// Push every receiver below what feeds it, after the pins and only as far as needed, x untouched.
pub fn push_below_inputs(mut nodes: Vec<Node>, edges: &[Edge]) -> Vec<Node> {
    let index: HashMap<&str, usize> = nodes.iter().enumerate().map(|(i, n)| (n.id.as_str(), i)).collect();
    let mut out: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut waiting: HashMap<usize, usize> = HashMap::new();
    for e in edges {
        // A curve drives a param and an event fires one: it sits BESIDE its node, so neither pushes anything down.
        if e.automation.is_some() || e.signal == Some(Signal::Event) {
            continue;
        }
        let (Some(&s), Some(&t)) = (index.get(e.source.as_str()), index.get(e.target.as_str())) else {
            continue;
        };
        // A node feeding itself constrains nothing, and would deadlock the walk below.
        if s == t {
            continue;
        }
        out.entry(s).or_default().push(t);
        *waiting.entry(t).or_insert(0) += 1;
    }
    if waiting.is_empty() {
        return nodes;
    }
    let mut ys: Vec<f64> = nodes.iter().map(y).collect();
    let mut ready: Vec<usize> = (0..nodes.len())
        .filter(|i| waiting.get(i).copied().unwrap_or(0) == 0)
        .collect();
    // Kahn: a node's y is final only once every input's is, which is what makes one pass enough.
    let mut i = 0;
    while i < ready.len() {
        let id = ready[i];
        i += 1;
        let bottom = ys[id] + nodes[id].h;
        for &to in out.get(&id).map(Vec::as_slice).unwrap_or(&[]) {
            if ys[to] < bottom {
                ys[to] = bottom;
            }
            let left = waiting.get(&to).copied().unwrap_or(1) - 1;
            waiting.insert(to, left);
            if left == 0 {
                ready.push(to);
            }
        }
    }
    // A node in a CYCLE is left alone.
    for (n, ny) in nodes.iter_mut().zip(ys) {
        if ny != y(n) {
            n.y = Some(ny);
        }
    }
    nodes
}
