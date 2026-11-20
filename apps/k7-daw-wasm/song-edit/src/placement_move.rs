// placement_move.rs — a placement's start rewritten in the unit it is written in, and its takes, comp and crossfades carried with it.
// A reader would get wrong that a number start is SECONDS: it stays seconds on a move, and a caller's own timecode is kept only for a musical one.

use crate::error::Res;
use crate::start_time::{js_string, position, start_as, to_position};
use crate::take_file::{placement_of, write_list};
use crate::track_file::{events_to_edit, key, list_at};
use k7x::edit::{PathSeg, Scope, Value, WriteOptions};

fn is_record(v: &Value) -> bool {
    matches!(v, Value::Map(_))
}

/// Rewrites the `start` of the placement at `index`, in the unit it is written in; `spelled` is the caller's timecode, kept for a musical start.
///
/// # Errors
/// A beat out of range for a timecode, or the write's.
pub fn move_placement_at(
    doc: &mut Scope,
    index: usize,
    to_beats: f64,
    num: f64,
    den: f64,
    tempo: f64,
    spelled: Option<&str>,
) -> Res<()> {
    let path = events_to_edit(doc)?;
    let was = list_at(doc, &path)?.unwrap_or_default().into_iter().nth(index);
    let like = was
        .as_ref()
        .filter(|w| is_record(w))
        .and_then(|w| w.get("start"))
        .cloned();
    let to = match (&like, spelled) {
        (l, Some(s)) if !matches!(l, Some(Value::Num(_))) => Value::Str(s.to_owned()),
        (l, _) => start_as(l.as_ref(), to_beats, num, den, tempo)?,
    };
    let mut at = path;
    at.extend([PathSeg::Index(index), key("start")]);
    Ok(doc.set_in(&at, &to, WriteOptions::default())?)
}

fn shifted(text: Option<&Value>, delta: f64, num: f64, den: f64) -> Res<Value> {
    let tc = text.map_or_else(|| "undefined".to_owned(), js_string);
    Ok(Value::Str(to_position(position(&tc, num, den)? + delta, num, den)?))
}

fn entries(v: &Value) -> Vec<(String, Value)> {
    match v {
        Value::Map(m) => m.clone(),
        _ => vec![],
    }
}

fn with(v: &Value, k: &str, x: Value) -> Value {
    let mut out = entries(v);
    match out.iter_mut().find(|(ek, _)| ek == k) {
        Some(slot) => slot.1 = x,
        None => out.push((k.to_owned(), x)),
    }
    Value::object(out)
}

/// A placement's plain values with its takes, comp and crossfades moved by `delta` beats.
///
/// # Errors
/// A position that does not read, or a beat out of range.
pub fn shift_takes(p: &Value, delta: f64, num: f64, den: f64) -> Res<Value> {
    let mut out = p.clone();
    if let Some(Value::List(takes)) = p.get("takes") {
        let mut moved = vec![];
        for t in takes {
            moved.push(with(t, "start", shifted(t.get("start"), delta, num, den)?));
        }
        out = with(&out, "takes", Value::List(moved));
    }
    if let Some(Value::List(comp)) = p.get("comp") {
        let mut moved = vec![];
        for c in comp {
            let from = shifted(c.get("from"), delta, num, den)?;
            let to = shifted(c.get("to"), delta, num, den)?;
            moved.push(with(&with(c, "from", from), "to", to));
        }
        out = with(&out, "comp", Value::List(moved));
    }
    if let Some(Value::List(xs)) = p.get("crossfades") {
        let mut moved = vec![];
        for x in xs {
            moved.push(with(x, "at", shifted(x.get("at"), delta, num, den)?));
        }
        out = with(&out, "crossfades", Value::List(moved));
    }
    Ok(out)
}

fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Num(n)) => *n != 0.0 && !n.is_nan(),
        Some(Value::Str(s)) => !s.is_empty(),
        Some(_) => true,
    }
}

/// The placement at `index` moved by `delta` beats keeps its takes where they were played: shift them with it.
///
/// # Errors
/// A position that does not read, or the write's.
pub fn shift_takes_at(doc: &mut Scope, index: usize, delta: f64, num: f64, den: f64) -> Res<()> {
    let mut at = events_to_edit(doc)?;
    at.push(PathSeg::Index(index));
    let p = placement_of(doc, &at)?;
    if !truthy(p.get("takes")) && !truthy(p.get("comp")) && !truthy(p.get("crossfades")) {
        return Ok(());
    }
    let moved = shift_takes(&p, delta, num, den)?;
    for k in ["takes", "comp", "crossfades"] {
        if let Some(Value::List(l)) = moved.get(k) {
            write_list(doc, &at, k, l.clone())?;
        }
    }
    Ok(())
}
