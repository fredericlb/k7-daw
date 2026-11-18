// clips/comp.rs — a comped event compiled: its own clip and its takes' clips SLICED into the pieces the comp plays; after compClips.ts.
// A reader would get wrong that one slice is two units: AUDIO moves its window into the FILE (seconds), MIDI over the NOTES (beats).

use super::doc::{ClipDoc, ClipKind, CompMark, CrossfadeMark, EventDoc};
use super::id::{js_min, placement_id};
use super::takes::{CompSpan, Piece, SeamFade, TakeRef, by_start, comp_pieces, seam_crossfades};
use super::{Budget, Grid};
use crate::files::Placement;
use std::collections::HashMap;

/// `c` from `offset` beats into it, for `length` beats (None: to its end), placed at song beat `start`.
pub fn slice_clip(c: &ClipDoc, offset: f64, length: Option<f64>, start: f64, grid: &Grid) -> ClipDoc {
    let rest = if c.length > 0.0 { c.length - offset } else { 0.0 };
    let len = length.unwrap_or(rest);
    // A piece starts and ends mid-performance: the event's own fades belong to its two ends, not to every cut.
    let mut out = ClipDoc {
        id: placement_id(&c.name, start),
        start,
        length: len,
        fade_in: 0.0,
        fade_out: 0.0,
        ..c.clone()
    };
    if c.kind == ClipKind::Midi {
        let from = (if c.loop_end > 0.0 { c.loop_start } else { 0.0 }) + offset;
        out.loop_start = from;
        out.loop_end = from + len;
        return out;
    }
    let source_from = c.source_from.unwrap_or(0.0) + grid.seconds(offset);
    let end = match length {
        None => c.source_to,
        Some(l) => Some(source_from + grid.seconds(l)),
    };
    out.source_to = match (end, c.source_to) {
        (None, _) => None,
        (Some(e), None) => Some(e),
        (Some(e), Some(t)) => Some(js_min(e, t)),
    };
    // A stretch map is laid on the WHOLE clip's timeline; a slice of it would play the wrong beats.
    out.warp = None;
    out.source_from = Some(source_from);
    out
}

/// The event's clips: its own alone, or the pieces of its comp, each marked with the event it belongs to.
/// `take_clip` compiles a take's ref and start text; None past the budget.
pub fn comp_clips(
    p: &Placement,
    own: ClipDoc,
    grid: &Grid,
    budget: &mut Budget,
    take_clip: &mut dyn FnMut(&str, &str, &mut Budget) -> Option<ClipDoc>,
) -> Option<Vec<ClipDoc>> {
    let spans = p.comp.as_deref().unwrap_or(&[]);
    // A looped event is one window repeated: there is no single span of it a take could replace.
    if spans.is_empty() || p.loop_.is_some_and(|l| l.0 > 1) {
        return Some(vec![own]);
    }
    let takes_in = p.takes.as_deref().unwrap_or(&[]);
    let takes: Vec<TakeRef> = takes_in
        .iter()
        .map(|t| TakeRef {
            ref_: t.ref_.0.clone(),
            start: grid.position_text(&t.start.0),
        })
        .collect();
    let comp: Vec<CompSpan> = spans
        .iter()
        .map(|s| CompSpan {
            take: s.take.0.clone(),
            from: grid.position_text(&s.from.0),
            to: grid.position_text(&s.to.0),
        })
        .collect();
    // Notes do not fade: an overlap at a MIDI seam would play both takes' notes twice, so it gets none.
    let xfades: Vec<SeamFade> = if own.kind == ClipKind::Audio {
        p.crossfades
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|x| SeamFade {
                at: grid.position_text(&x.at.0),
                length: grid.duration(&x.length.0),
            })
            .collect()
    } else {
        Vec::new()
    };
    let mut clip_of: HashMap<String, ClipDoc> = HashMap::new();
    clip_of.insert(own.name.clone(), own.clone());
    for t in takes_in {
        if t.ref_.0.is_empty() {
            continue;
        }
        let made = take_clip(&t.ref_.0, &t.start.0, budget);
        if budget.capped() {
            return None;
        }
        if let Some(c) = made {
            clip_of.insert(t.ref_.0.clone(), c);
        }
    }
    let base = Piece {
        ref_: own.name.clone(),
        start: own.start,
        offset: 0.0,
        length: (own.length > 0.0).then_some(own.length),
    };
    // How much of each material there is, beats: what a seam crossfade may reach into past a piece's end.
    let material_length = |r: &str| clip_of.get(r).filter(|c| c.length > 0.0).map(|c| c.length);
    let pieces = comp_pieces(&base, &takes, &comp, budget)?;
    let pieces = seam_crossfades(&pieces, &xfades, &material_length, budget)?;
    let first = pieces.iter().fold(f64::INFINITY, |m, q| js_min(m, q.piece.start));
    let last = pieces.iter().fold(f64::NEG_INFINITY, |m, q| {
        let end = q.piece.length.map_or(q.piece.start, |l| q.piece.start + l);
        super::id::js_max(m, end)
    });
    let mut out = Vec::new();
    for fp in &pieces {
        let piece = &fp.piece;
        let Some(c) = clip_of.get(&piece.ref_) else { continue };
        // The event's words are copied onto every piece, so they are charged with it (decision 157).
        if !budget.charge(c.weight() + own.event.as_ref().map_or(0, EventDoc::weight)) {
            return None;
        }
        let end = piece.length.map(|l| piece.start + l);
        // The event's own fades belong to its two ENDS; a seam's are the crossfade's.
        let fade_in = fp
            .fade_in
            .unwrap_or(if piece.start == first { own.fade_in } else { 0.0 });
        let fade_out = fp.fade_out.unwrap_or(if end.is_some_and(|e| e == last) {
            own.fade_out
        } else {
            0.0
        });
        let mut s = slice_clip(c, piece.offset, piece.length, piece.start, grid);
        s.fade_in = fade_in;
        s.fade_out = fade_out;
        let seam_in = fp.fade_in.is_some_and(|f| f != 0.0);
        let seam_out = fp.fade_out.is_some_and(|f| f != 0.0);
        if seam_in || seam_out {
            s.crossfade = Some(CrossfadeMark {
                in_: seam_in.then_some(true),
                out: seam_out.then_some(true),
            });
        }
        s.comp = Some(CompMark {
            of: own.id.clone(),
            take: piece.ref_.clone(),
        });
        if own.event.is_some() {
            s.event.clone_from(&own.event);
        }
        // Every piece carries the event's mute, takes included (decision 160).
        if own.disabled == Some(true) {
            s.disabled = Some(true);
        }
        out.push(s);
    }
    Some(out)
}

/// Mark the fades that are halves of a CROSSFADE: two clips of a lane overlapping by exactly their two fades.
pub fn mark_crossfades(clips: Vec<ClipDoc>, budget: &mut Budget) -> Vec<ClipDoc> {
    const EPS: f64 = 1e-6;
    let mut out = clips;
    let mut audio: Vec<usize> = (0..out.len())
        .filter(|&i| out[i].kind == ClipKind::Audio && out[i].length > 0.0)
        .collect();
    audio.sort_by(|&a, &b| by_start(out[a].start, out[b].start));
    let starts: Vec<f64> = audio.iter().map(|&i| out[i].start).collect();
    for &a in &audio {
        let (a_start, a_end, a_fade) = (out[a].start, out[a].start + out[a].length, out[a].fade_out);
        // The seam sits where a's fade begins: only clips starting there can match, so they are found, not scanned.
        let target = a_end - a_fade;
        let slack = 4.0 * EPS + target.abs() * 1e-12;
        let mut j = starts.partition_point(|s| *s < target - slack);
        while j < audio.len() && starts[j] <= target + slack {
            if !budget.charge(1) {
                return out;
            }
            let b = audio[j];
            j += 1;
            if out[b].start <= a_start {
                continue;
            }
            let overlap = a_end - out[b].start;
            if overlap > EPS && (a_fade - overlap).abs() < EPS && (out[b].fade_in - overlap).abs() < EPS {
                out[a].crossfade.get_or_insert_with(CrossfadeMark::default).out = Some(true);
                out[b].crossfade.get_or_insert_with(CrossfadeMark::default).in_ = Some(true);
            }
        }
    }
    out
}

/// The clips with `[from, to)` (song beats) cut out of them: the hole a take is recorded into.
pub fn cut_hole(clips: &[ClipDoc], from: f64, to: f64, grid: &Grid) -> Vec<ClipDoc> {
    let mut out = Vec::new();
    for c in clips {
        let end = if c.length > 0.0 {
            c.start + c.length
        } else {
            f64::INFINITY
        };
        if end <= from || c.start >= to {
            out.push(c.clone());
            continue;
        }
        if c.start < from {
            out.push(slice_clip(c, 0.0, Some(from - c.start), c.start, grid));
        }
        if end > to {
            let rest = end.is_finite().then_some(end - to);
            out.push(slice_clip(c, to - c.start, rest, to, grid));
        }
    }
    out
}
