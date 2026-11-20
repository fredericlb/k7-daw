// graph/reach.rs — which chains reach a sink, and the first chain a cycle is reachable from, each in one pass over the chain edges.
// A reader would get wrong the answer of `first_cycle`: the START in key order, not a member of the cycle (409).

use super::index::Index;
use crate::routing::split_port;

/// The edges of the chain graph by position: an out continues in the chain `chain_at` names.
fn edges(idx: &Index<'_>, i: usize) -> impl Iterator<Item = usize> {
    let g = idx.graph();
    let outs = g.chains.get_index(i).map_or(&[][..], |(_, c)| c.out.as_slice());
    outs.iter().filter_map(|o| idx.chain_at(&o.to))
}

/// Every chain that reaches a sink, one reverse reachability from the chains with an out at a sink (409).
pub fn reaching_sink(idx: &Index<'_>) -> Vec<bool> {
    let g = idx.graph();
    let n = g.chains.len();
    let mut reverse: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut reaches = vec![false; n];
    let mut queue: Vec<usize> = Vec::new();
    for (i, (_, c)) in g.chains.iter().enumerate() {
        if c.out.iter().any(|o| idx.is_sink(split_port(&o.to).0)) {
            reaches[i] = true;
            queue.push(i);
        }
        for next in edges(idx, i) {
            if let Some(r) = reverse.get_mut(next) {
                r.push(i);
            }
        }
    }
    while let Some(at) = queue.pop() {
        for &prev in reverse.get(at).map_or(&[][..], Vec::as_slice) {
            if !reaches[prev] {
                reaches[prev] = true;
                queue.push(prev);
            }
        }
    }
    reaches
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mark {
    Fresh,
    Open,
    Done,
}

/// The first chain, in key order, from which a cycle is reachable: one iterative three-colour walk, `Done` kept across starts (409).
pub fn first_cycle(idx: &Index<'_>) -> Option<usize> {
    let n = idx.graph().chains.len();
    let mut mark = vec![Mark::Fresh; n];
    for start in 0..n {
        if mark[start] != Mark::Fresh {
            continue;
        }
        // Each frame is a chain and the edges it has left to follow.
        let mut stack: Vec<(usize, Vec<usize>)> = vec![(start, edges(idx, start).collect())];
        mark[start] = Mark::Open;
        while let Some((at, left)) = stack.last_mut() {
            let at = *at;
            match left.pop() {
                None => {
                    mark[at] = Mark::Done;
                    stack.pop();
                }
                Some(next) => match mark[next] {
                    Mark::Open => return Some(start),
                    Mark::Done => {}
                    Mark::Fresh => {
                        mark[next] = Mark::Open;
                        stack.push((next, edges(idx, next).collect()));
                    }
                },
            }
        }
    }
    None
}
