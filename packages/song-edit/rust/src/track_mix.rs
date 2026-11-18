// track_mix.rs — mixer mutations: a track's scalar fields and its placements, rewritten in its file in place, a surgical edit and not a re-serialization.
// A reader would get wrong that `volume` is a LINEAR GAIN, not dB, and that a placement is addressed by its POSITION IN BEATS.

use crate::chain_item_edit::{lane_events_at, lane_folder};
use crate::clip_resize::{Bounds, MaterialKind, ResizeBefore, ResizeTo, resize_to_window};
use crate::error::{Res, fail};
use crate::fs::Fs;
use crate::material_file::material_window_of;
use crate::module_io::{ModuleFile, entity_file_for};
use crate::paths::join;
use crate::placement_move::{move_placement_at, shift_takes, shift_takes_at};
use crate::song_tempo::song_tempo;
use crate::start_time::{js_round, js_string, js_to_number, position, start_as, start_beats, to_position};
use crate::track_edit::edit_track_struct;
use crate::track_file::{
    add_placement, find_placement_at, placement_at, placement_values, remove_placement_at, set_placement_field,
    with_field,
};
use crate::workspace_doc::ws_text;
use k7x::edit::{Document, Scope, Value};
use song_schema::beats_to_duration_on;

pub const MIN_GAIN_DB: f64 = -60.0;
pub const MAX_GAIN_DB: f64 = 6.0;

/// Linear gain to dB: minus infinity at or below silence.
#[must_use]
pub fn gain_to_db(linear: f64) -> f64 {
    if linear <= 0.0 {
        f64::NEG_INFINITY
    } else {
        20.0 * linear.log10()
    }
}

/// dB to linear gain, clamped in dB so the ceiling is the one number the fader also uses.
#[must_use]
pub fn db_to_gain(db: f64) -> f64 {
    if db <= MIN_GAIN_DB {
        return 0.0;
    }
    10f64.powf(db.min(MAX_GAIN_DB) / 20.0)
}

/// A short human label for a stored pan (0..1, 0.5 is centre): "C", "L50", "R30".
#[must_use]
pub fn pan_to_label(pan: f64) -> String {
    let off = js_round((pan - 0.5) * 200.0);
    if off == 0.0 {
        "C".to_owned()
    } else if off < 0.0 {
        format!("L{}", -off)
    } else {
        format!("R{off}")
    }
}

fn num(n: f64) -> Value {
    Value::Num(n)
}

/// Move a placement to ANOTHER lane's file, carried over WHOLE and cut first.
///
/// # Errors
/// A lane with no file, a timecode that does not read, or the write's.
pub fn move_placement_to_track(
    fs: &mut dyn Fs,
    dir: &str,
    from_id: &str,
    to_id: &str,
    from_beats: f64,
    to_tc: &str,
    n: f64,
    d: f64,
) -> Res<bool> {
    let tempo = song_tempo(fs, dir)?;
    let to_beats = position(to_tc, n, d)?;
    let mut cut: Option<Value> = None;
    edit_track_struct(
        fs,
        dir,
        from_id,
        &format!("move a placement off track \"{from_id}\""),
        |doc| {
            let Some(at) = find_placement_at(doc, from_beats, n, d, tempo)? else {
                return Ok(());
            };
            cut = placement_at(doc, at)?;
            if cut.is_some() {
                remove_placement_at(doc, at)?;
            }
            Ok(())
        },
    )?;
    let Some(cut) = cut else { return Ok(false) };
    let start = match cut.get("start") {
        Some(own @ Value::Num(_)) => start_as(Some(own), to_beats, n, d, tempo)?,
        _ => Value::Str(to_tc.to_owned()),
    };
    let carried = with_field(&shift_takes(&cut, to_beats - from_beats, n, d)?, "start", Some(start));
    edit_track_struct(
        fs,
        dir,
        to_id,
        &format!("move a placement onto track \"{to_id}\""),
        |doc| add_placement(doc, &carried),
    )?;
    Ok(true)
}

/// Move the placement at `from_beats` to `to_tc`, leaving the rest of the file alone.
///
/// # Errors
/// A lane with no file, a timecode that does not read, or the write's.
pub fn move_track_placement(
    fs: &mut dyn Fs,
    dir: &str,
    id: &str,
    from_beats: f64,
    to_tc: &str,
    n: f64,
    d: f64,
) -> Res<bool> {
    let tempo = song_tempo(fs, dir)?;
    let to_beats = position(to_tc, n, d)?;
    let mut moved = false;
    edit_track_struct(fs, dir, id, "move a clip", |doc| {
        let Some(at) = find_placement_at(doc, from_beats, n, d, tempo)? else {
            return Ok(());
        };
        moved = true;
        move_placement_at(doc, at, to_beats, n, d, tempo, Some(to_tc))?;
        shift_takes_at(doc, at, to_beats - from_beats, n, d)
    })?;
    Ok(moved)
}

/// Move every placement starting in `[from_beats, to_beats]` by `delta_beats`, in one rewrite.
///
/// # Errors
/// A lane with no file, or the write's.
pub fn move_track_placements_in_range(
    fs: &mut dyn Fs,
    dir: &str,
    id: &str,
    from_beats: f64,
    to_beats: f64,
    delta_beats: f64,
    n: f64,
    d: f64,
) -> Res<usize> {
    let tempo = song_tempo(fs, dir)?;
    let mut moved = 0;
    edit_track_struct(fs, dir, id, "move the events in a range", |doc| {
        // An unparseable start can't fall in any range.
        let starts: Vec<Option<f64>> = placement_values(doc)?
            .iter()
            .map(|p| start_beats(p.get("start"), n, d, tempo, "1:1:1").ok())
            .collect();
        for (i, s) in starts.into_iter().enumerate() {
            let Some(s) = s.filter(|s| !(*s < from_beats - 1e-6 || *s > to_beats + 1e-6)) else {
                continue;
            };
            // By index, not by position: indices are stable under a start rewrite.
            move_placement_at(doc, i, s + delta_beats, n, d, tempo, None)?;
            shift_takes_at(doc, i, delta_beats, n, d)?;
            moved += 1;
        }
        Ok(())
    })?;
    Ok(moved)
}

/// Write one field of the placement at `from_beats`; None DELETES, which is how a caller says "default".
fn set_placement_field_at(
    fs: &mut dyn Fs,
    dir: &str,
    id: &str,
    from_beats: f64,
    n: f64,
    d: f64,
    field: &str,
    value: Option<Value>,
    what: &str,
) -> Res<bool> {
    let tempo = song_tempo(fs, dir)?;
    let mut hit = false;
    edit_track_struct(fs, dir, id, what, |doc| {
        let Some(at) = find_placement_at(doc, from_beats, n, d, tempo)? else {
            return Ok(());
        };
        hit = true;
        set_placement_field(doc, at, field, value)
    })?;
    Ok(hit)
}

/// Switch the placement at `from_beats` on or off: written when true, deleted when false.
///
/// # Errors
/// A lane with no file, or the write's.
pub fn set_track_placement_disabled(
    fs: &mut dyn Fs,
    dir: &str,
    id: &str,
    from_beats: f64,
    n: f64,
    d: f64,
    disabled: bool,
) -> Res<bool> {
    let v = disabled.then_some(Value::Bool(true));
    set_placement_field_at(fs, dir, id, from_beats, n, d, "disabled", v, "disable a clip")
}

/// Whether the placement at `from_beats` plays the material's stretch map: polarity inverted.
///
/// # Errors
/// A lane with no file, or the write's.
pub fn set_track_placement_warp(
    fs: &mut dyn Fs,
    dir: &str,
    id: &str,
    from_beats: f64,
    n: f64,
    d: f64,
    warp: bool,
) -> Res<bool> {
    let v = (!warp).then_some(Value::Bool(false));
    set_placement_field_at(fs, dir, id, from_beats, n, d, "warp", v, "set an event's warp")
}

/// How long the placement at `from_beats` fades, in beats; zero deletes, and a fade is a DURATION.
///
/// # Errors
/// A lane with no file, a beat out of range, or the write's.
pub fn set_track_placement_fade(
    fs: &mut dyn Fs,
    dir: &str,
    id: &str,
    from_beats: f64,
    n: f64,
    d: f64,
    which: &str,
    beats: f64,
) -> Res<bool> {
    let v = if beats > 0.0 {
        Some(Value::Str(
            beats_to_duration_on(beats, n, d).map_err(|e| crate::error::Error::Msg(e.to_string()))?,
        ))
    } else {
        None
    };
    set_placement_field_at(fs, dir, id, from_beats, n, d, which, v, "set an event's fade")
}

/// Which slice of its material the placement at `from_beats` plays: its `from` and `to`, in SECONDS.
///
/// # Errors
/// A lane with no file, or the write's.
pub fn set_track_placement_window(
    fs: &mut dyn Fs,
    dir: &str,
    id: &str,
    from_beats: f64,
    n: f64,
    d: f64,
    win: (f64, f64),
    dur: f64,
) -> Res<bool> {
    let tidy = |s: f64| js_round(s * 10000.0) / 10000.0;
    let from = (win.0 > 1e-4).then(|| num(tidy(win.0)));
    let to = (!(dur > 0.0 && win.1 >= dur - 1e-4)).then(|| num(tidy(win.1)));
    let mut set = |field: &str, v: Option<Value>| {
        set_placement_field_at(fs, dir, id, from_beats, n, d, field, v, "set an event's window")
    };
    // Both, always: an edge the drag decided against has to go, or it narrows what the other opened.
    let a = set("from", from)?;
    let b = set("to", to)?;
    Ok(a || b)
}

/// What a resize touched: ONE event, since the window it writes is the placement's own.
#[derive(Clone, Debug, PartialEq)]
pub struct ResizeReport {
    pub reference: String,
    pub material: String,
    pub kind: MaterialKind,
    pub loops: f64,
}

const EDGE_EPS: f64 = 1e-6;

/// Where a placement's material lives: `clips/<ref>` or `sequences/<ref>`, whichever exists.
fn material_for(fs: &dyn Fs, dir: &str, r: &str) -> Option<(MaterialKind, ModuleFile)> {
    if let Some(f) = entity_file_for(fs, &join(&[dir, "clips"]), "clips", r) {
        return Some((MaterialKind::Audio, f));
    }
    entity_file_for(fs, &join(&[dir, "sequences"]), "sequences", r).map(|f| (MaterialKind::Sequence, f))
}

fn raw_edge(v: Option<&Value>) -> Option<&Value> {
    v.filter(|x| matches!(x, Value::Num(_) | Value::Str(_)))
}

/// Re-time the event at `from_beats` to span `start_beats..end_beats`, or None when there is none.
///
/// # Errors
/// A lane with no file, a material that is neither a clip nor a sequence, or the write's.
pub fn resize_track_placement(
    fs: &mut dyn Fs,
    dir: &str,
    id: &str,
    from_beats: f64,
    n: f64,
    d: f64,
    start: f64,
    end: f64,
    tempo: f64,
) -> Res<Option<ResizeReport>> {
    let Some(own) = lane_events_at(fs, dir, id)? else {
        return fail(format!("track \"{id}\" has no file in {}/", lane_folder(fs, dir)));
    };
    let mut doc = Document::new(&ws_text(fs, &own.path)?);
    let (placement, at) = {
        let lane = doc.at(&own.base);
        let Some(at) = find_placement_at(&lane, from_beats, n, d, tempo)? else {
            return Ok(None);
        };
        (
            placement_values(&lane)?
                .into_iter()
                .nth(at)
                .unwrap_or(Value::Map(vec![])),
            at,
        )
    };
    let _ = at;
    let r = match placement.get("ref") {
        None | Some(Value::Null) => String::new(),
        Some(v) => js_string(v),
    };
    let Some((kind, file)) = material_for(fs, dir, &r) else {
        return fail(format!("'{r}' is neither a clip nor a sequence in this project"));
    };
    // As authored: a timecode window is read the way the compiler reads it, to the same numbers.
    let edge = |v: Option<&Value>| -> Res<Option<f64>> {
        Ok(match v {
            None => None,
            Some(Value::Num(x)) => Some(*x),
            Some(other) => {
                let beats = position(&js_string(other), n, d)?;
                Some(if kind == MaterialKind::Sequence {
                    beats
                } else {
                    beats * 60.0 / tempo
                })
            }
        })
    };
    let win = material_window_of(fs, &file.path)?;
    let mat_from = edge(win.from.as_ref())?.unwrap_or(0.0);
    let mat_to = edge(win.to.as_ref())?;
    let ev_from = edge(raw_edge(placement.get("from")))?;
    let ev_to = edge(raw_edge(placement.get("to")))?;
    let looped = match placement.get("loop") {
        None | Some(Value::Null) => 1.0,
        Some(v) => js_to_number(v),
    }
    .floor();
    let loops = if looped.is_nan() || looped == 0.0 {
        1.0
    } else {
        looped.max(1.0)
    };
    let next = resize_to_window(
        &ResizeBefore {
            kind,
            from: mat_from.max(ev_from.unwrap_or(0.0)),
            to: match (mat_to, ev_to) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            },
            loops,
            start_beats: from_beats,
            bounds: Some(Bounds {
                from: Some(mat_from),
                to: mat_to,
            }),
        },
        &ResizeTo {
            start_beats: start,
            end_beats: end,
            tempo,
        },
    );
    let fmt = |u: f64| -> Res<Value> {
        Ok(if kind == MaterialKind::Sequence {
            Value::Str(to_position(u, n, d)?)
        } else {
            num(u)
        })
    };
    edit_track_struct(fs, dir, id, "resize a clip", |doc: &mut Scope| {
        let Some(i) = find_placement_at(doc, from_beats, n, d, tempo)? else {
            return Ok(());
        };
        if (next.start_beats - from_beats).abs() > 1e-6 {
            move_placement_at(doc, i, next.start_beats, n, d, tempo, None)?;
        }
        // `loop: 1` is the default and is cleared rather than written, like every other absent field.
        set_placement_field(doc, i, "loop", (next.loops > 1.0).then(|| num(next.loops)))?;
        // A looped event's window does not move: its from/to stay as authored.
        if next.loops <= 1.0 {
            let from = if next.from > mat_from + EDGE_EPS {
                Some(fmt(next.from)?)
            } else {
                None
            };
            set_placement_field(doc, i, "from", from)?;
            let to = match mat_to {
                Some(m) if next.to >= m - EDGE_EPS => None,
                _ => Some(fmt(next.to)?),
            };
            set_placement_field(doc, i, "to", to)?;
        }
        Ok(())
    })?;
    Ok(Some(ResizeReport {
        reference: r,
        material: file.rel,
        kind,
        loops: next.loops,
    }))
}

/// Put `r` on lane `id` at `start`: a REFERENCE, landing on the set the lane is PLAYING.
///
/// # Errors
/// A lane with no file, or the write's.
pub fn add_track_placement(fs: &mut dyn Fs, dir: &str, id: &str, r: &str, start: &str) -> Res<String> {
    let p = Value::object(vec![
        ("ref".to_owned(), Value::Str(r.to_owned())),
        ("start".to_owned(), Value::Str(start.to_owned())),
    ]);
    edit_track_struct(fs, dir, id, &format!("put \"{r}\" on track \"{id}\""), |doc| {
        add_placement(doc, &p)
    })
}

/// Put SEVERAL placements on one lane in ONE rewrite: atomic, and in the caller's order.
///
/// # Errors
/// A lane with no file, or the write's.
pub fn add_track_placements(fs: &mut dyn Fs, dir: &str, id: &str, placements: &[Value]) -> Res<String> {
    edit_track_struct(
        fs,
        dir,
        id,
        &format!("put {} placement(s) on track \"{id}\"", placements.len()),
        |doc| placements.iter().try_for_each(|p| add_placement(doc, p)),
    )
}

/// Remove the placement at `from_beats`; the clip or sequence itself is NOT touched.
///
/// # Errors
/// A lane with no file, or the write's.
pub fn remove_track_placement(fs: &mut dyn Fs, dir: &str, id: &str, from_beats: f64, n: f64, d: f64) -> Res<bool> {
    let tempo = song_tempo(fs, dir)?;
    let mut removed = false;
    edit_track_struct(fs, dir, id, "remove a clip", |doc| {
        let Some(at) = find_placement_at(doc, from_beats, n, d, tempo)? else {
            return Ok(());
        };
        removed = true;
        remove_placement_at(doc, at)
    })?;
    Ok(removed)
}
