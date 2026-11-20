// event_loop.rs — an event's LOOP as the clip panels edit it: switched on or off, and its two points moved.
// A reader would get wrong what the points are: the event's WINDOW, so a looped event keeps its length and its pass count gives.

use crate::chain_item_edit::lane_events_at;
use crate::clip_resize::MaterialKind;
use crate::error::{Res, fail};
use crate::fs::Fs;
use crate::material_file::material_window_of;
use crate::material_notes::{EDGE_EPS, loops_of, ref_of};
use crate::module_io::entity_file_for;
use crate::paths::join;
use crate::start_time::{above, js_round, js_to_number, position, to_position};
use crate::track_edit::edit_track_struct;
use crate::track_file::{find_placement_at, placement_values, set_placement_field};
use crate::workspace_doc::{ws_data, ws_text};
use k7x::edit::{Document, Value};

/// The passes a looped event starts with: one more than none, the least that is a loop at all.
const FIRST_LOOP: f64 = 2.0;

/// Where an event addressed by lane and start stands: its material, its window as the compiler reads it, its passes.
struct Placed {
    kind: MaterialKind,
    mat_from: f64,
    mat_to: Option<f64>,
    from: f64,
    to: Option<f64>,
    loops: f64,
}

/// An event's address: a lane, the beat it starts on and the project's meter and tempo.
#[derive(Clone, Copy)]
pub struct At<'a> {
    pub dir: &'a str,
    pub id: &'a str,
    pub from_beats: f64,
    pub num: f64,
    pub den: f64,
    pub tempo: f64,
}

/// The bar-rounded end of a sequence's last note, as nodeProject.ts computes it; None for one with no notes.
fn sequence_extent(fs: &dyn Fs, path: &str, num: f64, den: f64) -> Res<Option<f64>> {
    let doc = ws_data(fs, path)?;
    let notes = match doc.get("notes") {
        Some(Value::List(l)) => l.as_slice(),
        _ => &[],
    };
    let n = |v: Option<&Value>| js_to_number(v.filter(|x| !matches!(x, Value::Null)).unwrap_or(&Value::Num(0.0)));
    let last = notes
        .iter()
        .map(|x| n(x.get("t")) + n(x.get("d")))
        .filter(|e| e.is_finite())
        .fold(0.0_f64, f64::max);
    let bar = num * 4.0 / den;
    Ok((last > 0.0).then(|| (last / bar).ceil() * bar))
}

fn placed(fs: &dyn Fs, a: &At) -> Res<Option<Placed>> {
    let Some(own) = lane_events_at(fs, a.dir, a.id)? else {
        return fail(format!("track \"{}\" has no lane file", a.id));
    };
    let mut doc = Document::new(&ws_text(fs, &own.path)?);
    let lane = doc.at(&own.base);
    let Some(at) = find_placement_at(&lane, a.from_beats, a.num, a.den, a.tempo)? else {
        return Ok(None);
    };
    let p = placement_values(&lane)?
        .into_iter()
        .nth(at)
        .unwrap_or(Value::Map(vec![]));
    let ref_ = ref_of(&p);
    let clip = entity_file_for(fs, &join(&[a.dir, "clips"]), "clips", &ref_);
    let seq = if clip.is_some() {
        None
    } else {
        entity_file_for(fs, &join(&[a.dir, "sequences"]), "sequences", &ref_)
    };
    let Some(file) = clip.clone().or(seq) else {
        return fail(format!("'{ref_}' is neither a clip nor a sequence in this project"));
    };
    let kind = if clip.is_some() {
        MaterialKind::Audio
    } else {
        MaterialKind::Sequence
    };
    // As authored, read the way the compiler reads it: a timecode is a position, a number seconds of a file.
    let edge = |v: Option<&Value>| -> Res<Option<f64>> {
        match v {
            Some(Value::Num(n)) => Ok(Some(*n)),
            Some(Value::Str(s)) => {
                let beats = position(s, a.num, a.den)?;
                Ok(Some(if kind == MaterialKind::Sequence {
                    beats
                } else {
                    beats * 60.0 / a.tempo
                }))
            }
            _ => Ok(None),
        }
    };
    let win = material_window_of(fs, &file.path)?;
    let mat_from = edge(win.from.as_ref())?.unwrap_or(0.0);
    // A sequence that states no end ends where the compiler says: its last note, rounded up to a bar.
    let mat_to = match edge(win.to.as_ref())? {
        Some(t) => Some(t),
        None if kind == MaterialKind::Sequence => sequence_extent(fs, &file.path, a.num, a.den)?,
        None => None,
    };
    let ev_from = edge(p.get("from"))?;
    let ev_to = edge(p.get("to"))?;
    let to = match (mat_to, ev_to) {
        (Some(m), Some(e)) => Some(m.min(e)),
        (m, e) => m.or(e),
    };
    Ok(Some(Placed {
        kind,
        mat_from,
        mat_to,
        from: mat_from.max(ev_from.unwrap_or(0.0)),
        to,
        loops: loops_of(&p),
    }))
}

/// Writes fields of the placement at the event's start in one edit; None deletes, which is how a default is said.
fn write(fs: &mut dyn Fs, a: &At, fields: &[(&str, Option<Value>)], what: &str) -> Res<()> {
    edit_track_struct(fs, a.dir, a.id, what, |doc| {
        let Some(i) = find_placement_at(doc, a.from_beats, a.num, a.den, a.tempo)? else {
            return Ok(());
        };
        for (k, v) in fields {
            set_placement_field(doc, i, k, v.clone())?;
        }
        Ok(())
    })?;
    Ok(())
}

/// What a loop switch did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LoopSwitch {
    pub loops: f64,
    pub changed: bool,
}

/// Switches the event's loop on (`FIRST_LOOP` passes) or off (one); `file_seconds` gives an audio window with no end one.
///
/// # Errors
/// A lane with no file, a material that is neither clip nor sequence, an audio file whose length is not known, or a write's.
pub fn set_track_placement_loop(
    fs: &mut dyn Fs,
    a: &At,
    on: bool,
    file_seconds: Option<f64>,
) -> Res<Option<LoopSwitch>> {
    let Some(p) = placed(fs, a)? else { return Ok(None) };
    if !on {
        if p.loops <= 1.0 {
            return Ok(Some(LoopSwitch {
                loops: 1.0,
                changed: false,
            }));
        }
        write(fs, a, &[("loop", None)], "stop an event looping")?;
        return Ok(Some(LoopSwitch {
            loops: 1.0,
            changed: true,
        }));
    }
    if p.loops > 1.0 {
        return Ok(Some(LoopSwitch {
            loops: p.loops,
            changed: false,
        }));
    }
    let mut fields: Vec<(&str, Option<Value>)> = vec![("loop", Some(Value::Num(FIRST_LOOP)))];
    // A repeat needs the window's END, and an audio file states none until someone writes it.
    if p.to.is_none() {
        let secs = file_seconds.filter(|s| *s > 0.0);
        let Some(secs) = secs else {
            return fail("its file's length is not known, so there is no end for the loop to repeat from");
        };
        // Seconds to the precision every window writer keeps, and DOWN, so a file's length is never overshot.
        fields.push(("to", Some(Value::Num((secs * 10000.0).floor() / 10000.0))));
    }
    write(fs, a, &fields, "loop an event")?;
    Ok(Some(LoopSwitch {
        loops: FIRST_LOOP,
        changed: true,
    }))
}

/// What moving the loop points left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LoopPoints {
    pub kind: MaterialKind,
    pub from: f64,
    pub to: f64,
    pub loops: f64,
}

/// Moves the event's loop points (its window) to `from`/`to` in the material's unit, clamped to the material.
///
/// # Errors
/// A lane with no file, points that do not run forwards inside the material, or a write's.
pub fn set_track_placement_loop_points(fs: &mut dyn Fs, a: &At, from: f64, to: f64) -> Res<Option<LoopPoints>> {
    let Some(p) = placed(fs, a)? else { return Ok(None) };
    // The event can only narrow its material: past the material's edges, a point stops on them.
    let f = p.mat_from.max(from);
    let t = p.mat_to.map_or(to, |m| m.min(to));
    if !above(t - f, EDGE_EPS) {
        return fail("the loop points have to run forwards, inside the material");
    }
    let beats = |u: f64| {
        if p.kind == MaterialKind::Sequence {
            u
        } else {
            u * a.tempo / 60.0
        }
    };
    // A looped event keeps its length on the lane: the passes are what gives, never below a loop.
    let mut loops = p.loops;
    if let Some(pt) = p.to.filter(|_| p.loops > 1.0) {
        let extent = beats(pt - p.from) * p.loops;
        loops = FIRST_LOOP.max(js_round(extent / beats(t - f)));
    }
    let fmt = |u: f64| -> Res<Value> {
        if p.kind == MaterialKind::Sequence {
            Ok(Value::Str(to_position(u, a.num, a.den)?))
        } else {
            Ok(Value::Num(js_round(u * 10000.0) / 10000.0))
        }
    };
    let from_field = if f > p.mat_from + EDGE_EPS { Some(fmt(f)?) } else { None };
    // Cleared on the material's own end, but kept when the material states none: a loop needs one.
    let to_field = if p.mat_to.is_some_and(|m| t >= m - EDGE_EPS) {
        None
    } else {
        Some(fmt(t)?)
    };
    let loop_field = (loops > 1.0).then_some(Value::Num(loops));
    write(
        fs,
        a,
        &[("from", from_field), ("to", to_field), ("loop", loop_field)],
        "move an event's loop points",
    )?;
    Ok(Some(LoopPoints {
        kind: p.kind,
        from: f,
        to: t,
        loops,
    }))
}
