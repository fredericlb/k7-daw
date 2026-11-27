// picture_file.rs — read and edit a PICTURE lane's `events:` list, each shot `{ref, start, length, from}` in decimal seconds.
// A reader would get wrong that a refused edit is `Ok(false)` and writes nothing, as is an unreadable buffer.

use crate::picture_animate::{animate_path, animate_value, parse_animate, split_animate};
use k7x::edit::{PathSeg, Scope, Value, WriteOptions};
use crate::error::Res;
use crate::js_text::js_to_number;
use crate::start_time::js_round;

/// A placement of a picture lane's `events:`; absent fields read as the file's defaults.
#[derive(Clone, Debug, PartialEq)]
pub struct Shot {
    /// The `ref:` value, EMPTY on an ACTIVATION.
    pub ref_: String,
    pub start: f64,
    pub length: f64,
    pub source_from: f64,
    pub speed: Option<f64>,
    pub silent: bool,
}

pub fn key(k: &str) -> PathSeg {
    PathSeg::Key(k.to_owned())
}

fn path(parts: &[PathSeg]) -> Vec<PathSeg> {
    parts.to_vec()
}

fn num(v: Option<&Value>, dflt: f64) -> f64 {
    let n = js_to_number(v);
    if n.is_finite() { n } else { dflt }
}

/// Rounds to milliseconds: a drag's float noise does not belong in the file.
pub fn ms(n: f64) -> f64 {
    js_round(n * 1000.0) / 1000.0
}

fn data_of(sc: &Scope) -> Option<Value> {
    sc.to_js().ok().flatten().filter(|v| matches!(v, Value::Map(_)))
}

pub fn events_of(sc: &Scope) -> Option<Vec<Value>> {
    match data_of(sc)?.get("events") {
        Some(Value::List(l)) => Some(l.clone()),
        _ => None,
    }
}

pub fn has_index(events: &Option<Vec<Value>>, index: usize) -> bool {
    events.as_ref().is_some_and(|e| index < e.len())
}

/// The placements a lane declares, in file order, the ref-less ones included: the indices are `events:`'s.
pub fn list_shots(sc: &Scope) -> Vec<Shot> {
    events_of(sc)
        .unwrap_or_default()
        .iter()
        .map(|e| Shot {
            ref_: match e.get("ref") {
                Some(Value::Str(s)) => s.clone(),
                _ => String::new(),
            },
            start: num(e.get("start"), 0.0),
            length: num(e.get("length"), 0.0),
            source_from: num(e.get("from"), 0.0),
            speed: match e.get("speed") {
                None | Some(Value::Null) => None,
                s => Some(num(s, 1.0)),
            },
            silent: matches!(e.get("silent"), Some(Value::Bool(true))),
        })
        .collect()
}

/// Sets the lane's own rect, `frame([x, y, w, h])` in viewport px: all four, always.
pub fn set_lane_rect(sc: &mut Scope, rect: [f64; 4]) -> Res<bool> {
    if data_of(sc).is_none() {
        return Ok(true);
    }
    let v = Value::List(rect.iter().map(|n| Value::Num(js_round(*n))).collect());
    sc.set_in(&[key("frame")], &v, WriteOptions { inline: true })?;
    Ok(true)
}

/// Sets one numeric field of the placement at `index`, rounded to the millisecond; false with none there.
pub fn set_field_at(sc: &mut Scope, index: usize, field: &str, value: f64) -> Res<bool> {
    if !has_index(&events_of(sc), index) {
        return Ok(false);
    }
    let at = path(&[key("events"), PathSeg::Index(index), key(field)]);
    sc.set_in(&at, &Value::Num(ms(value)), WriteOptions::default())?;
    Ok(true)
}

/// Sets the `speed:`, signed, or drops the key at 1, which is what absent already means; false with none there.
pub fn set_speed_at(sc: &mut Scope, index: usize, speed: f64) -> Res<bool> {
    if !has_index(&events_of(sc), index) {
        return Ok(false);
    }
    let at = path(&[key("events"), PathSeg::Index(index), key("speed")]);
    if (speed - 1.0).abs() < 1e-9 {
        sc.delete_in(&at, false)?;
    } else {
        let v = js_round(speed * 10000.0) / 10000.0;
        sc.set_in(&at, &Value::Num(v), WriteOptions::default())?;
    }
    Ok(true)
}

/// Silences, or lets be heard again, the sound of the placement at `index`: a heard shot is the default, so the key is removed.
pub fn set_silent_at(sc: &mut Scope, index: usize, silent: bool) -> Res<bool> {
    if !has_index(&events_of(sc), index) {
        return Ok(false);
    }
    let at = path(&[key("events"), PathSeg::Index(index), key("silent")]);
    if silent {
        sc.set_in(&at, &Value::Bool(true), WriteOptions::default())?;
    } else {
        sc.delete_in(&at, false)?;
    }
    Ok(true)
}

/// Drops the placement at `index`; false when there is none there.
pub fn remove_at(sc: &mut Scope, index: usize) -> Res<bool> {
    if !has_index(&events_of(sc), index) {
        return Ok(false);
    }
    sc.delete_in(&[key("events"), PathSeg::Index(index)], false)?;
    Ok(true)
}

/// Appends a placement, creating `events:` when the lane has none; false when `events:` holds something else.
pub fn add_shot(sc: &mut Scope, ref_: &str, start: f64, length: f64, source_from: f64) -> Res<bool> {
    let Some(data) = data_of(sc) else {
        return Ok(false);
    };
    let mut entry = vec![
        ("ref".to_owned(), Value::Str(ref_.to_owned())),
        ("start".to_owned(), Value::Num(ms(start))),
        ("length".to_owned(), Value::Num(ms(length))),
    ];
    if source_from != 0.0 {
        entry.push(("from".to_owned(), Value::Num(ms(source_from))));
    }
    let entry = Value::Map(entry);
    match data.get("events") {
        Some(Value::List(_)) => {
            sc.add_in(&[key("events")], &entry, WriteOptions { inline: true })?;
            Ok(true)
        }
        None | Some(Value::Null) => {
            sc.set_in(&[key("events")], &Value::List(vec![entry]), WriteOptions::default())?;
            Ok(true)
        }
        Some(_) => Ok(false),
    }
}

/// Cuts the placement at `index` in two at `cut` seconds, the tail inserted RIGHT AFTER it; false when the cut is not inside it.
pub fn split_at(sc: &mut Scope, index: usize, cut: f64) -> Res<bool> {
    let Some(events) = events_of(sc) else {
        return Ok(false);
    };
    let Some(shot) = events.get(index) else {
        return Ok(false);
    };
    let start = num(shot.get("start"), 0.0);
    let length = num(shot.get("length"), 0.0);
    let next = events
        .get(index + 1)
        .map_or(f64::INFINITY, |n| num(n.get("start"), 0.0));
    let end = if length > 0.0 { start + length } else { next };
    if !(ms(cut) > start && ms(cut) < end) {
        return Ok(false);
    }
    let head = ms(cut) - start;
    let source = sc
        .get_source(&[key("events"), PathSeg::Index(index)])?
        .unwrap_or_else(|| "{}".to_owned());
    sc.insert_source_in(&[key("events")], index + 1, &source)?;
    let at = |i: usize, f: &str| path(&[key("events"), PathSeg::Index(i), key(f)]);
    let plain = WriteOptions::default();
    sc.set_in(&at(index, "length"), &Value::Num(ms(head)), plain)?;
    sc.delete_in(&at(index, "fadeOut"), false)?;
    sc.set_in(&at(index + 1, "start"), &Value::Num(ms(cut)), plain)?;
    let tail_len = if length > 0.0 { ms(length - head) } else { 0.0 };
    sc.set_in(&at(index + 1, "length"), &Value::Num(tail_len), plain)?;
    sc.delete_in(&at(index + 1, "fadeIn"), false)?;
    if let Some(animate) = parse_animate(shot.get("animate")) {
        let (h, t) = split_animate(&animate, head, length);
        let inline = WriteOptions { inline: true };
        sc.set_in(&animate_path(index), &animate_value(&h), inline)?;
        sc.set_in(&animate_path(index + 1), &animate_value(&t), inline)?;
    }
    if matches!(shot.get("ref"), Some(Value::Str(_))) {
        let from = ms(num(shot.get("from"), 0.0) + head * num(shot.get("speed"), 1.0));
        sc.set_in(&at(index + 1, "from"), &Value::Num(from), plain)?;
    }
    Ok(true)
}
