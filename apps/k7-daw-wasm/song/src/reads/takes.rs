// reads/takes.rs — what the TAKES view draws off the open project: an event's takes as rows, which spans its comp plays, the lines a pick writes.
// A reader would get wrong that every span here is in SONG beats: a take states where its material starts, the comp states song positions.

use super::model::RClip;
use crate::clips::{material_ref_of, placement_start_of};
use serde::Serialize;
use song_schema::{beats_to_duration_on, beats_to_position_on, parse_duration_on, parse_position_on, round_beat};
use std::cmp::Ordering;

const EPS: f64 = 1e-6;

/// One row of the view: a take, or the event's own material, starting at `start` beats.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TakeRow {
    #[serde(rename = "ref")]
    #[cfg_attr(feature = "ts", ts(rename = "ref"))]
    pub ref_: String,
    pub start: f64,
    pub own: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CompBeats {
    pub take: String,
    pub from: f64,
    pub to: f64,
}

/// The crossfade at a seam: centred on `at`, `length` beats long.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Crossfade {
    pub at: f64,
    pub length: f64,
}

/// An event with its takes, everything in song beats; `pieces` are sorted indices, see `pieceHost`.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", ts(rename_all = "camelCase"))]
pub struct TakesEvent {
    pub track: String,
    #[serde(rename = "ref")]
    #[cfg_attr(feature = "ts", ts(rename = "ref"))]
    pub ref_: String,
    pub start: f64,
    pub at: String,
    #[cfg_attr(feature = "ts", ts(type = "\"audio\" | \"midi\""))]
    pub kind: String,
    pub rows: Vec<TakeRow>,
    pub comp: Vec<CompBeats>,
    pub pieces: Vec<usize>,
    /// The lane clip whose own `pieces` the indices point into, when it has them; else they point into the lane's clips.
    #[cfg_attr(feature = "ts", ts(optional))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub piece_host: Option<usize>,
    pub crossfades: Vec<Crossfade>,
    pub from: f64,
    pub to: f64,
}

fn js_min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() { f64::NAN } else { a.min(b) }
}

fn js_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() { f64::NAN } else { a.max(b) }
}

fn by_start(clips: &[RClip], idx: &mut [usize]) {
    idx.sort_by(|&a, &b| {
        (clips[a].start - clips[b].start)
            .partial_cmp(&0.0)
            .unwrap_or(Ordering::Equal)
    });
}

/// The event `event_id` among a lane's `clips`, with its takes and comp; none when no clip of it is there. A timecode that does not read is an error.
pub fn takes_event_of(
    track: &str,
    event_id: &str,
    clips: &[RClip],
    num: f64,
    den: f64,
) -> Result<Option<TakesEvent>, String> {
    let pieces: Vec<usize> = (0..clips.len())
        .filter(|&i| clips[i].id == event_id || clips[i].comp.as_ref().is_some_and(|c| c.of == event_id))
        .collect();
    let Some(&first) = pieces.first() else { return Ok(None) };
    let Some(start) = placement_start_of(event_id).filter(|s| s.is_finite()) else {
        return Ok(None);
    };
    let ref_ = material_ref_of(event_id).to_owned();
    let pos = |tc: &str| parse_position_on(tc, num, den).map_err(|e| e.to_string());
    let ev = pieces.iter().find_map(|&i| clips[i].event.as_ref());
    let mut rows = vec![TakeRow {
        ref_: ref_.clone(),
        start,
        own: true,
    }];
    let mut comp = Vec::new();
    let mut crossfades = Vec::new();
    if let Some(ev) = ev {
        for t in &ev.takes {
            rows.push(TakeRow {
                ref_: t.ref_.clone(),
                start: pos(&t.start)?,
                own: false,
            });
        }
        for s in &ev.comp {
            comp.push(CompBeats {
                take: s.take.clone(),
                from: pos(&s.from)?,
                to: pos(&s.to)?,
            });
        }
        for x in &ev.crossfades {
            crossfades.push(Crossfade {
                at: pos(&x.at)?,
                length: parse_duration_on(&x.length, num, den).map_err(|e| e.to_string())?,
            });
        }
    }
    let mut from = start;
    let mut to = start;
    for &i in &pieces {
        from = js_min(from, clips[i].start);
        to = js_max(to, clips[i].start + clips[i].length);
    }
    let own_pieces = clips[first].pieces.as_deref();
    let (mut idx, host) = match own_pieces {
        Some(p) => ((0..p.len()).collect::<Vec<_>>(), Some(first)),
        None => (pieces, None),
    };
    by_start(own_pieces.unwrap_or(clips), &mut idx);
    Ok(Some(TakesEvent {
        track: track.to_owned(),
        ref_,
        start,
        at: beats_to_position_on(start, num, den).map_err(|e| e.to_string())?,
        kind: clips[first].kind.clone(),
        rows,
        comp,
        pieces: idx,
        piece_host: host,
        crossfades,
        from,
        to,
    }))
}

/// The spans of song time row `ref` is heard over: its comp spans, and for the event's own, what no span covers.
pub fn heard_spans(comp: &[(String, f64, f64)], own: bool, ref_: &str, start: f64, row_end: f64) -> Vec<(f64, f64)> {
    if !own {
        return comp.iter().filter(|s| s.0 == ref_).map(|s| (s.1, s.2)).collect();
    }
    let mut spans = vec![(start, row_end)];
    for (_, from, to) in comp {
        spans = spans
            .into_iter()
            .flat_map(|h| {
                if *to <= h.0 || *from >= h.1 {
                    vec![h]
                } else {
                    let mut v = Vec::new();
                    if *from > h.0 {
                        v.push((h.0, *from));
                    }
                    if *to < h.1 {
                        v.push((*to, h.1));
                    }
                    v
                }
            })
            .collect();
    }
    spans
}

/// Where two pieces of the comp meet inside the event: the places a crossfade can go.
pub fn comp_seams(start: f64, to: f64, comp: &[(f64, f64)]) -> Vec<f64> {
    let mut all: Vec<f64> = comp
        .iter()
        .flat_map(|s| [s.0, s.1])
        .filter(|&b| b > start + EPS && b < to - EPS)
        .map(round_beat)
        .collect();
    all.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
    all.dedup();
    all
}

fn pos(b: f64, num: f64, den: f64) -> Result<String, String> {
    beats_to_position_on(b, num, den).map_err(|e| e.to_string())
}

/// The line that plays `ref_` over a span.
pub fn comp_line(track: &str, at: &str, ref_: &str, from: f64, to: f64, num: f64, den: f64) -> Result<String, String> {
    Ok(format!(
        "event comp {track} {at} {ref_} {} {}",
        pos(from, num, den)?,
        pos(to, num, den)?
    ))
}

/// The line that arms the next take: punched into the selection, or from the playhead to the stop.
pub fn arm_line(track: &str, at: &str, from: f64, to: Option<f64>, num: f64, den: f64) -> Result<String, String> {
    let tail = match to {
        Some(t) => format!(" {}", pos(t, num, den)?),
        None => String::new(),
    };
    Ok(format!("event take arm {track} {at} {}{tail}", pos(from, num, den)?))
}

/// The line that crossfades the seam over `beats`; 0 removes it.
pub fn comp_fade_line(track: &str, at: &str, seam: f64, beats: f64, num: f64, den: f64) -> Result<String, String> {
    let len = if beats > 0.0 {
        beats_to_duration_on(beats, num, den).map_err(|e| e.to_string())?
    } else {
        "0".to_owned()
    };
    Ok(format!("event comp fade {track} {at} {} {len}", pos(seam, num, den)?))
}

/// The take a piece plays: its comp marker, else its own material.
fn take_of_piece(p: &RClip) -> &str {
    match &p.comp {
        Some(c) if !c.take.is_empty() => &c.take,
        _ => &p.name,
    }
}

/// A piece's edges dragged to `from..to` (song beats), as comp lines.
pub fn piece_edge_lines(
    track: &str,
    at: &str,
    event_ref: &str,
    pieces: &[RClip],
    piece_id: &str,
    from: f64,
    to: f64,
    num: f64,
    den: f64,
) -> Result<Vec<String>, String> {
    let Some(piece) = pieces.iter().position(|p| p.id == piece_id) else {
        return Ok(Vec::new());
    };
    if to <= from {
        return Ok(Vec::new());
    }
    let piece_at = &pieces[piece];
    let start = piece_at.start;
    let end = piece_at.start + piece_at.length;
    let neighbour = |q: Option<&RClip>, at: f64| -> String {
        match q {
            Some(q) if ((if at == start { q.start + q.length } else { q.start }) - at).abs() < 2.0 => {
                take_of_piece(q).to_owned()
            }
            _ => event_ref.to_owned(),
        }
    };
    let take = take_of_piece(piece_at);
    let line = |r: &str, f: f64, t: f64| comp_line(track, at, r, f, t, num, den);
    let mut lines = Vec::new();
    if from > start + EPS {
        let before = piece.checked_sub(1).and_then(|i| pieces.get(i));
        lines.push(line(&neighbour(before, start), start, from)?);
    }
    if to < end - EPS {
        lines.push(line(&neighbour(pieces.get(piece + 1), end), to, end)?);
    }
    if from < start - EPS || to > end + EPS {
        lines.push(line(take, from, to)?);
    }
    Ok(lines)
}
