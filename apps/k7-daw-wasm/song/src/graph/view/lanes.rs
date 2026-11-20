// graph/view/lanes.rs — what each lane shows: its colour, its signal, its clips with their widths, its other attribute sets, what is frozen.
// A reader would get wrong that a clip's width comes from the COMPILE (`Handed`): a project that does not compile draws clips without one.

use super::web::{WebAltChoice, WebTrackClip};
use super::{ClipExtent, Handed, Marks};
use crate::graph::meter::Meter;
use crate::graph::{ClipKind, Spend, Track};
use crate::resolve::{alternatives_of, current_alternative};
use indexmap::IndexMap;
use std::collections::HashMap;

/// Beats drawn for a clip whose compiled length is 0.
const NATURAL_BEATS: f64 = 4.0;

/// The kind of a placement as the graph spells it.
fn kind_name(k: ClipKind) -> &'static str {
    k.name()
}

/// A lane's extents by start, the last one winning, `-0` made `0` (432).
fn by_start(extents: &[ClipExtent]) -> HashMap<u64, f64> {
    extents
        .iter()
        .map(|c| (if c.start == 0.0 { 0.0f64 } else { c.start }.to_bits(), c.length))
        .collect()
}

/// A lane's clips in playing order, each with its width when the project compiles.
pub(super) fn track_clips(
    t: &Track<'_>,
    handed: &Handed,
    meter: &Meter,
    spend: &mut Spend,
) -> Option<Vec<WebTrackClip>> {
    if t.placements.is_empty() {
        return None;
    }
    let known = handed.extents.as_ref().and_then(|e| e.get(t.name)).map(|e| by_start(e));
    let mut sorted: Vec<&crate::graph::GraphPlacement<'_>> = t.placements.iter().collect();
    sorted.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap_or(std::cmp::Ordering::Equal));
    let mut out = Vec::with_capacity(sorted.len());
    for p in sorted {
        if !spend.bytes(p.ref_.len()) {
            break;
        }
        let key = if p.start == 0.0 { 0.0f64 } else { p.start }.to_bits();
        let len = known
            .as_ref()
            .and_then(|k| k.get(&key))
            .filter(|l| l.is_finite())
            .map(|l| if *l > 0.0 { *l } else { NATURAL_BEATS });
        out.push(WebTrackClip {
            ref_: p.ref_.to_owned(),
            start: p.start,
            tc: meter.spell(p.start),
            len,
            kind: kind_name(p.kind).to_owned(),
        });
    }
    (!out.is_empty()).then_some(out)
}

/// A lane's other attribute sets, from the module it IS, only past one; absent once the budget is spent (419).
pub(super) fn track_alternative(g: &crate::graph::GraphDoc<'_>, id: &str, spend: &mut Spend) -> Option<WebAltChoice> {
    if spend.capped() {
        return None;
    }
    let m = g.modules.get(id)?;
    let a = &m.m.authored;
    let all = alternatives_of(a);
    if all.len() < 2 {
        return None;
    }
    let alternative = all.get(current_alternative(a))?.name.clone();
    let bytes = all.iter().map(|x| x.name.len() + 16).sum::<usize>() + alternative.len() + id.len();
    if !spend.bytes(bytes) {
        return None;
    }
    Some(WebAltChoice {
        alternatives: all.into_iter().map(|x| x.name).collect(),
        alternative,
    })
}

/// The mixer strips that fit the budget, in order (419).
pub(super) fn paid_mixers(mixers: IndexMap<String, String>, spend: &mut Spend) -> IndexMap<String, String> {
    let mut out = IndexMap::new();
    for (k, v) in mixers {
        if !spend.bytes(k.len() + v.len()) {
            break;
        }
        out.insert(k, v);
    }
    out
}

/// The tap feeds that fit the budget, in order (419).
pub(super) fn paid_tap_feeds(feeds: &[crate::routing::TapFeed], spend: &mut Spend) -> Vec<crate::routing::TapFeed> {
    let mut out = Vec::new();
    for f in feeds {
        if !spend.bytes(f.node.len() + f.tap.len() + f.to.len() + 16) {
            break;
        }
        out.push(f.clone());
    }
    out
}

/// The stacks that fit the budget, in order (419).
pub(super) fn paid_stacks(stacks: Vec<Vec<String>>, spend: &mut Spend) -> Vec<Vec<String>> {
    let mut out = Vec::new();
    for s in stacks {
        if !spend.bytes(s.iter().map(|n| n.len() + 16).sum::<usize>()) {
            break;
        }
        out.push(s);
    }
    out
}

/// What a lane puts on a wire when a run leaves it BARE: midi when it holds sequences alone.
pub(super) fn signal(t: &Track<'_>) -> crate::modules::PortType {
    if !t.placements.is_empty() && t.placements.iter().all(|p| p.kind == ClipKind::Midi) {
        crate::modules::PortType::Midi
    } else {
        crate::modules::PortType::Audio
    }
}

/// What is baked, by node name: per plan in order every lane and module inside, then the point itself (431).
pub(super) fn frozen(handed: &Handed, spend: &mut Spend) -> Marks {
    let mut out: IndexMap<String, String> = IndexMap::new();
    'plans: for p in &handed.manual {
        for n in p.lanes.iter().chain(&p.modules) {
            if !spend.bytes(n.len()) {
                break 'plans;
            }
            out.insert(n.clone(), "inside".to_owned());
        }
        if !spend.bytes(p.at.len()) {
            break;
        }
        out.insert(p.at.clone(), "at".to_owned());
    }
    out
}

/// What an automatic freeze bakes (`ready` and not hosted), and what one would bake (waiting): each node names the freeze's VST module (431).
pub(super) fn auto(handed: &Handed, spend: &mut Spend) -> (Marks, Marks) {
    let (mut baked, mut waiting): (Marks, Marks) = (IndexMap::new(), IndexMap::new());
    'freezes: for f in &handed.auto {
        let into = if f.ready && !f.hosted {
            &mut baked
        } else if f.hosted {
            continue;
        } else {
            &mut waiting
        };
        for n in f.lanes.iter().chain(&f.modules) {
            if !spend.bytes(n.len() + f.module.len()) {
                break 'freezes;
            }
            into.insert(n.clone(), f.module.clone());
        }
    }
    (baked, waiting)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::BUDGET;
    use crate::routing::TapFeed;

    /// A budget with `left` units to spare.
    fn nearly_spent(left: usize) -> Spend {
        let mut s = Spend::new();
        s.charge(BUDGET - left);
        s
    }

    // 419: what a list sends is paid by its entries' bytes, and the list stops where the budget does.
    #[test]
    fn a_list_sent_is_paid_by_its_bytes_and_stops_at_the_budget() {
        let big = "n".repeat(64 * 100);
        let feeds: Vec<TapFeed> = (0..10)
            .map(|_| TapFeed {
                node: big.clone(),
                tap: "t".into(),
                to: "x".into(),
            })
            .collect();
        let sent = paid_tap_feeds(&feeds, &mut nearly_spent(250));
        assert_eq!(sent.len(), 2);
        let stacks: Vec<Vec<String>> = (0..10).map(|_| vec![big.clone()]).collect();
        assert_eq!(paid_stacks(stacks, &mut nearly_spent(250)).len(), 2);
        let mixers = (0..10).map(|n| (format!("l{n}"), big.clone())).collect();
        assert_eq!(paid_mixers(mixers, &mut nearly_spent(250)).len(), 2);
        let mut spent = Spend::new();
        paid_stacks(vec![vec![big.clone()]], &mut spent);
        assert!(spent.used() > 100, "{}", spent.used());
    }
}
