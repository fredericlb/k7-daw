// reads/rack.rs — which notes reach each rack, in arrangement beats, so the pads can light with the playhead.
// A reader would get wrong that this is DERIVED: the engine reports no notes, and a live MIDI or strudel lane lights nothing.

use super::clip::clip_loop;
use super::model::{RClip, RGraph, RTrack};
use indexmap::IndexMap;
use serde::Serialize;

/// The shortest a pad stays lit, in beats: a floor on the display, not on the note.
const MIN_FLASH_BEATS: f64 = 0.25;

/// One note reaching a rack, in arrangement beats: when it starts, when it stops, which key.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Hit {
    pub from: f64,
    pub to: f64,
    pub key: f64,
}

/// One clip's notes in arrangement beats: a looped event tiles its window's notes across its placed length.
fn clip_hits(c: &RClip, out: &mut Vec<Hit>) {
    let mut hit = |at: f64, duration: f64, key: f64| {
        out.push(Hit { from: c.start + at, to: c.start + at + duration.max(MIN_FLASH_BEATS), key });
    };
    let Some(l) = clip_loop(&c.kind, c.length, c.loop_start, c.loop_end) else {
        for n in &c.notes {
            hit(n.time, n.duration, n.key);
        }
        return;
    };
    let span = if c.length > 0.0 { c.length } else { l.win };
    let mut k = 0.0;
    while k < l.passes {
        for n in c.notes.iter().filter(|n| n.time >= l.from && n.time < l.to) {
            let at = n.time - l.from + k * l.win;
            // A pass cut short by the placed length plays only the notes that start inside it.
            if at < span {
                hit(at, n.duration, n.key);
            }
        }
        k += 1.0;
    }
}

/// Every note reaching each rack node, keyed by module, sorted by start.
pub fn rack_hits(tracks: &[RTrack], graph: &RGraph) -> IndexMap<String, Vec<Hit>> {
    let mut out: IndexMap<String, Vec<Hit>> = IndexMap::new();
    for chain in graph.chains.values() {
        let Some(lane) = chain.from.as_deref().and_then(|f| tracks.iter().rev().find(|t| t.id == f)) else { continue };
        let racks: Vec<&str> =
            chain.r#use.iter().filter(|s| s.rack.as_ref().is_some_and(|r| !r.is_empty())).map(|s| s.module.as_str()).collect();
        if racks.is_empty() {
            continue;
        }
        let mut hits = Vec::new();
        for c in &lane.clips {
            clip_hits(c, &mut hits);
        }
        if hits.is_empty() {
            continue;
        }
        for m in racks {
            out.entry(m.to_owned()).or_default().extend(hits.iter().cloned());
        }
    }
    for hits in out.values_mut() {
        hits.sort_by(|a, b| a.from.partial_cmp(&b.from).unwrap_or(std::cmp::Ordering::Equal));
    }
    out
}
