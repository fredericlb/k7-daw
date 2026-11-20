// clips/takes.rs — an event's takes and its comp, in beats: which material plays over which span; after song-schema's takes.ts.
// A reader would get wrong that a comp span is in SONG time: a take carries where it starts, and plays from (span start - take start).

use super::Budget;
use super::id::{js_max, js_min};
use std::cmp::Ordering;
use std::collections::HashMap;

/// A take on an event: its material, and the song beat its material's first beat lands on.
#[derive(Clone, Debug, PartialEq)]
pub struct TakeRef {
    pub ref_: String,
    pub start: f64,
}

/// From `from` to `to` (song beats) the event plays `take`; "" plays nothing.
#[derive(Clone, Debug, PartialEq)]
pub struct CompSpan {
    pub take: String,
    pub from: f64,
    pub to: f64,
}

/// What plays: `ref_` at song beat `start`, read from `offset` beats into it, for `length` beats (None: to its end).
#[derive(Clone, Debug, PartialEq)]
pub struct Piece {
    pub ref_: String,
    pub start: f64,
    pub offset: f64,
    pub length: Option<f64>,
}

/// A crossfade at a seam of the comp: centred on song beat `at`, `length` beats long.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SeamFade {
    pub at: f64,
    pub length: f64,
}

/// A piece with the fades a seam crossfade gives it, beats.
#[derive(Clone, Debug, PartialEq)]
pub struct FadedPiece {
    pub piece: Piece,
    pub fade_in: Option<f64>,
    pub fade_out: Option<f64>,
}

const EPS: f64 = 1e-9;

/// Ascending by start, a NaN last: JavaScript's subtraction would call a NaN equal to everything, which is no order.
pub(crate) fn by_start(a: f64, b: f64) -> Ordering {
    match a.partial_cmp(&b) {
        Some(o) => o,
        None => a.is_nan().cmp(&b.is_nan()),
    }
}

/// Cut `[from, to)` out of the pieces, shortening and splitting whatever crosses it; None past the budget.
fn cut(pieces: Vec<Piece>, from: f64, to: f64, budget: &mut Budget) -> Option<Vec<Piece>> {
    if !budget.charge(pieces.len()) {
        return None;
    }
    let mut out = Vec::with_capacity(pieces.len() + 1);
    for p in pieces {
        let end = p.length.map_or(f64::INFINITY, |l| p.start + l);
        if end <= from + EPS || p.start >= to - EPS {
            out.push(p);
            continue;
        }
        if p.start < from - EPS {
            out.push(Piece {
                length: Some(from - p.start),
                ..p.clone()
            });
        }
        if end > to + EPS {
            out.push(Piece {
                ref_: p.ref_.clone(),
                start: to,
                offset: p.offset + (to - p.start),
                length: p.length.map(|_| end - to),
            });
        }
    }
    Some(out)
}

/// The pieces an event plays: its own material, each comp span laid over it in order, later spans winning.
pub fn comp_pieces(base: &Piece, takes: &[TakeRef], comp: &[CompSpan], budget: &mut Budget) -> Option<Vec<Piece>> {
    let mut start_of: HashMap<&str, f64> = takes.iter().map(|t| (t.ref_.as_str(), t.start)).collect();
    start_of.insert(base.ref_.as_str(), base.start - base.offset);
    let mut pieces = vec![base.clone()];
    for span in comp {
        let take_start = start_of.get(span.take.as_str()).copied();
        // Nothing before a take's first beat: a span reaching earlier is cut to where the take begins.
        let from = take_start.map_or(span.from, |t| js_max(span.from, t));
        if span.to <= from + EPS {
            continue;
        }
        pieces = cut(pieces, from, span.to, budget)?;
        if let Some(t) = take_start
            && !span.take.is_empty()
        {
            pieces.push(Piece {
                ref_: span.take.clone(),
                start: from,
                offset: from - t,
                length: Some(span.to - from),
            });
        }
    }
    pieces.sort_by(|a, b| by_start(a.start, b.start));
    Some(pieces)
}

/// Overlap the two pieces meeting at each crossfade by its length, centred on the seam, as far as their material reaches.
pub fn seam_crossfades(
    pieces: &[Piece],
    xfades: &[SeamFade],
    material_length: &dyn Fn(&str) -> Option<f64>,
    budget: &mut Budget,
) -> Option<Vec<FadedPiece>> {
    let mut out: Vec<FadedPiece> = pieces
        .iter()
        .map(|p| FadedPiece {
            piece: p.clone(),
            fade_in: None,
            fade_out: None,
        })
        .collect();
    for x in xfades {
        if !budget.charge(out.len().saturating_mul(2)) {
            return None;
        }
        let p = out
            .iter()
            .position(|q| q.piece.length.is_some_and(|l| (q.piece.start + l - x.at).abs() < EPS));
        let q = out.iter().position(|r| (r.piece.start - x.at).abs() < EPS);
        let (Some(pi), Some(qi)) = (p, q) else { continue };
        let Some(plen) = out[pi].piece.length else { continue };
        if pi == qi {
            continue;
        }
        // Half on each side, no more than either piece is long, nor than the material on either side of the seam.
        let room = material_length(&out[pi].piece.ref_);
        let mut half = x.length / 2.0;
        half = js_min(half, out[qi].piece.offset);
        half = js_min(half, plen / 2.0);
        if let Some(ql) = out[qi].piece.length {
            half = js_min(half, ql / 2.0);
        }
        if let Some(room) = room {
            half = js_min(half, room - (out[pi].piece.offset + plen));
        }
        if half.is_nan() || half <= EPS {
            continue;
        }
        out[pi].piece.length = Some(plen + half);
        out[pi].fade_out = Some(2.0 * half);
        let q = &mut out[qi];
        q.piece.start -= half;
        q.piece.offset -= half;
        q.piece.length = q.piece.length.map(|l| l + half);
        q.fade_in = Some(2.0 * half);
    }
    Some(out)
}
