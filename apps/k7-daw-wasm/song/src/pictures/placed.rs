// pictures/placed.rs — the shots a placed timeline puts in the PICTURE: each `pictures:` lane lands on the song's lane of its name.
// A reader would get wrong that the host is found by NAME, not by the lane the timeline is placed on: that lane is a sound lane.

use super::{LaneShots, ShotIn, Spend, edge_weight, seconds_of, weigh};
use crate::clips::{MAX_NEST, Placer, js_min};
use crate::files::Placement;

/// Placed shots by picture lane name, in song seconds, each with an explicit length.
pub type PlacedShots<'t> = LaneShots<'t>;

/// One lane's shots moved `from` seconds on and cut at `end`; a shot with no length gets the one the picture would give it (519).
fn place_shots<'t>(
    timeline: &'t str,
    shots: &'t [Placement],
    (from, end): (f64, f64),
    nested: bool,
    spend: &mut Spend,
) -> Option<Vec<ShotIn<'t>>> {
    let mut out = Vec::new();
    for (i, s) in shots.iter().enumerate() {
        let next = shots.get(i + 1).and_then(|n| n.start.as_ref());
        if !spend.charge(1 + edge_weight(s.start.as_ref()) + edge_weight(next)) {
            return None;
        }
        let own = seconds_of(s.start.as_ref());
        let start = from + own;
        let room = end - start;
        if room <= 1e-6 {
            continue;
        }
        // The picture's rule: a length, else the next entry's start, else open, open being the timeline's end here.
        let stated = s.length.map_or(0.0, |l| l.0);
        let length = if stated > 0.0 {
            stated
        } else {
            shots.get(i + 1).map_or(room, |n| seconds_of(n.start.as_ref()) - own)
        };
        if length.is_nan() || length <= 0.0 {
            continue;
        }
        if !spend.charge(1) {
            return None;
        }
        out.push(ShotIn {
            event: s,
            start,
            length: js_min(length, room),
            timeline: Some(timeline),
            nested,
        });
    }
    Some(out)
}

/// Every shot the timelines among `events` place, by picture lane name: recursively, each cut at its timeline's end (518).
pub fn timeline_shots<'t, 'a: 't>(
    events: &'t [Placement],
    placer: &mut Placer<'a>,
    spend: &mut Spend,
    out: &mut LaneShots<'t>,
) {
    let file = spend.file().to_owned();
    walk(
        events,
        placer,
        spend,
        out,
        &Where {
            file: &file,
            within: None,
        },
        &mut Vec::new(),
    );
}

/// Where a walk is: the file it stops in, and the timeline around it.
struct Where<'w> {
    file: &'w str,
    /// Beats: where the enclosing timeline starts and the beat past which nothing of it plays.
    within: Option<(f64, f64)>,
}

fn walk<'t, 'a: 't>(
    events: &'t [Placement],
    placer: &mut Placer<'a>,
    spend: &mut Spend,
    out: &mut LaneShots<'t>,
    at: &Where<'_>,
    path: &mut Vec<&'t str>,
) -> bool {
    let mats = placer.materials();
    let grid = *placer.grid();
    let bps = grid.tempo / 60.0;
    for p in events {
        let r: &'t str = p.ref_.as_ref().map_or("", |r| r.0.as_str());
        if !spend.charge(weigh(r)) {
            spend.limit(at.file);
            return false;
        }
        let Some(t) = mats.timeline(r) else { continue };
        // A cycle is checkReferences' to report, and a nest past the depth is cut without a word.
        if p.disabled == Some(true) || path.contains(&r) || path.len() >= MAX_NEST {
            continue;
        }
        if !spend.charge(edge_weight(p.start.as_ref())) {
            spend.limit(at.file);
            return false;
        }
        let start = at.within.map_or(0.0, |w| w.0) + grid.position(p.start.as_ref());
        let end = js_min(
            start + placer.timeline_length(r),
            at.within.map_or(f64::INFINITY, |w| w.1),
        );
        let file = mats.timeline_file(r);
        for lane in t.pictures.iter().flatten() {
            if !spend.charge(1) {
                spend.limit(&file);
                return false;
            }
            if lane.disabled == Some(true) {
                continue;
            }
            if !spend.charge(weigh(&lane.lane.0)) {
                spend.limit(&file);
                return false;
            }
            let events: &'t [Placement] = lane.events.as_deref().unwrap_or(&[]);
            let Some(shots) = place_shots(r, events, (start / bps, end / bps), at.within.is_some(), spend) else {
                spend.limit(&file);
                return false;
            };
            if !shots.is_empty() {
                out.entry(lane.lane.0.as_str()).or_default().extend(shots);
            }
        }
        path.push(r);
        let inner = Where {
            file: &file,
            within: Some((start, end)),
        };
        for lane in &t.lanes {
            if !spend.charge(1) {
                spend.limit(&file);
                path.pop();
                return false;
            }
            if lane.disabled == Some(true) {
                continue;
            }
            let events: &'t [Placement] = lane.events.as_deref().unwrap_or(&[]);
            if !walk(events, placer, spend, out, &inner, path) {
                path.pop();
                return false;
            }
        }
        path.pop();
    }
    true
}

/// The song's picture lanes with the placed shots put FIRST, so a lane's own open shot does not end at a placed one (521); a stop is named by that lane's file.
pub fn with_timeline_shots<'t>(
    lanes: impl Iterator<Item = (&'t str, &'t [Placement])>,
    mut placed: LaneShots<'t>,
    spend: &mut Spend,
    file_of: &dyn Fn(&str) -> String,
) -> LaneShots<'t> {
    let mut out = LaneShots::new();
    for (name, events) in lanes {
        let held = placed.get(name).map_or(0, Vec::len);
        let edges: usize = events.iter().map(|e| edge_weight(e.start.as_ref())).sum();
        if !spend.charge(weigh(name) + held + events.len() + edges) {
            spend.limit(&file_of(name));
            break;
        }
        let mut shots = placed.swap_remove(name).unwrap_or_default();
        shots.extend(events.iter().map(ShotIn::own));
        out.insert(name, shots);
    }
    out
}
