// curve.rs — the curve files: `curve new`, `curve set` (one write of the whole file) and a drawn curve's points; after curve.ts, curvePoint.ts.
// A reader would get wrong how a point is addressed: by its TIME, matched within 1e-6 beats, never by its index in the list.

use super::shared::{Done, join, meter, position, split_words};
use crate::host::Host;
use k7x::PathSeg;
use k7x::edit::{Document, StringifyOptions, Value, WriteOptions, js_number, stringify};
use serde_json::Value as Json;
use song::curves::decode_curve;
use song_edit::Res;
use song_edit::error::fail;
use song_edit::start_time::{js_string, to_position};
use song_edit::workspace_doc::{EditOpts, ws_data, ws_edit, ws_write_text};
use song_edit::workspace_file::ws_exists;

/// The fields of each kind, in the order a file lists them: what `curve set` keeps and nothing else.
fn curve_fields(kind: &str) -> &'static [&'static str] {
    match kind {
        "lfo" => &[
            "type",
            "shape",
            "frequency",
            "phase",
            "duration",
            "amplitude",
            "polarity",
            "cut",
            "conditions",
        ],
        "custom" => &["type", "amplitude", "polarity", "cut", "points", "conditions"],
        "onoff" => &["type", "points", "conditions"],
        _ => &[
            "type",
            "attack",
            "decay",
            "sustain",
            "release",
            "level",
            "amplitude",
            "polarity",
            "cut",
            "conditions",
        ],
    }
}

pub fn curve_rel(id: &str) -> String {
    format!("curves/{id}.k7x")
}

/// The filename IS the id, so it has to be one a path and a reference can both carry.
pub fn bad_name(id: &str) -> Option<String> {
    let ok = !id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
    (!ok).then(|| format!("invalid curve name '{id}' — letters, digits, '_' and '-' only"))
}

/// Makes `curves/` before a file lands in it; a folder that is already there is no failure.
pub fn curves_dir(h: &mut Host) {
    let dir = join(&h.song_path, "curves");
    let _ = h.fs.mkdir_all(&dir);
}

/// `curve new <name>` — scaffolds `curves/<name>.k7x` as a sine LFO, refusing to overwrite.
pub fn curve_create(a: &str, h: &mut Host) -> Done {
    let id = a.trim();
    if id.is_empty() {
        h.refuse(["usage: curve new <name>  (e.g. curve new sweep)".to_owned()]);
        return Ok(());
    }
    if let Some(why) = bad_name(id) {
        h.refuse([format!("curve new: {why}")]);
        return Ok(());
    }
    let rel = curve_rel(id);
    let path = join(&h.song_path, &rel);
    if ws_exists(&*h.fs, &path) {
        h.refuse([format!("curve new: curve '{id}' already exists ({rel})")]);
        return Ok(());
    }
    curves_dir(h);
    let text = [
        "type(\"lfo\")",
        "shape(\"sine\") // sine | triangle | saw | square",
        "frequency(1) // cycles per bar",
        "polarity(\"unipolar\") // unipolar 0..1 | bipolar -1..1",
        "phase(0) // 0..1 of a cycle",
        "",
    ]
    .join("\n");
    if let Err(e) = ws_write_text(h.fs, &path, &text) {
        h.refuse([format!("curve new: {e}")]);
        return Ok(());
    }
    h.reload();
    h.log([format!("created {rel}")]);
    Ok(())
}

const SET_USAGE: &str = "usage: curve set <name> <json> (e.g. curve set wobble {\"type\":\"lfo\",\"shape\":\"sine\",\"frequency\":1,\"phase\":0,\"amplitude\":1})";

/// A JSON value as k7x data.
fn k7x_of(j: &Json) -> Value {
    match j {
        Json::Null => Value::Null,
        Json::Bool(b) => Value::Bool(*b),
        Json::Number(n) => Value::Num(n.as_f64().unwrap_or(f64::NAN)),
        Json::String(s) => Value::Str(s.clone()),
        Json::Array(a) => Value::List(a.iter().map(k7x_of).collect()),
        Json::Object(o) => Value::object(o.iter().map(|(k, v)| (k.clone(), k7x_of(v))).collect()),
    }
}

/// A curve's points written one a line, so a moved point is a one-line diff.
fn point_lines(path: &[PathSeg], _: &Value) -> bool {
    path.len() == 2 && matches!(&path[0], PathSeg::Key(k) if k == "points")
}

/// The curve file `raw` makes, checked as the compile reads it; the refusal is the decode's first finding.
fn curve_text(id: &str, raw: &serde_json::Map<String, Json>) -> Result<String, String> {
    let whole = k7x_of(&Json::Object(raw.clone()));
    let text = stringify(
        &whole,
        &StringifyOptions {
            header: &[],
            inline: None,
        },
    )
    .map_err(|e| e.to_string())?;
    decode_curve(&curve_rel(id), &text).map_err(|f| format!("curve '{id}': {}", f.message))?;
    let kind = raw.get("type").and_then(Json::as_str).unwrap_or("");
    let ordered = Value::object(
        curve_fields(kind)
            .iter()
            .filter_map(|k| raw.get(*k).map(|v| ((*k).to_owned(), k7x_of(v))))
            .collect(),
    );
    stringify(
        &ordered,
        &StringifyOptions {
            header: &[],
            inline: Some(&point_lines),
        },
    )
    .map_err(|e| e.to_string())
}

/// `curve set <name> <json>` — a curve written WHOLE, created or replaced: the curve editor's save.
pub fn curve_set(a: &str, h: &mut Host) -> Done {
    let t = a.trim();
    let Some((id, json)) = t.split_once(|c: char| c.is_whitespace()) else {
        h.refuse([SET_USAGE.to_owned()]);
        return Ok(());
    };
    let Ok(Json::Object(raw)) = serde_json::from_str::<Json>(json.trim()) else {
        h.refuse([SET_USAGE.to_owned()]);
        return Ok(());
    };
    let text = match bad_name(id).map_or_else(|| curve_text(id, &raw), Err) {
        Ok(t) => t,
        Err(why) => {
            h.refuse([format!("curve set: {why}")]);
            return Ok(());
        }
    };
    let rel = curve_rel(id);
    let path = join(&h.song_path, &rel);
    let created = !ws_exists(&*h.fs, &path);
    curves_dir(h);
    if let Err(e) = ws_write_text(h.fs, &path, &text) {
        h.refuse([format!("curve set: {e}")]);
        return Ok(());
    }
    h.reload();
    h.log([format!("{} {rel}", if created { "created" } else { "wrote" })]);
    Ok(())
}

/// One point as the file holds it: its beat and its value.
pub struct PointAt {
    pub beat: f64,
    pub value: f64,
}

fn points_of(raw: &Value) -> Vec<Value> {
    match raw.get("points") {
        Some(Value::List(l)) => l.clone(),
        _ => Vec::new(),
    }
}

/// One point's beat, or None when its time does not read: one unreadable point, never a broken file.
fn beat_of(point: &Value, num: f64, den: f64) -> Option<f64> {
    if !matches!(point, Value::Map(_)) {
        return None;
    }
    let time = point.get("time").map_or_else(String::new, js_string);
    song_edit::start_time::position(&time, num, den).ok()
}

/// The points a curve's data declares, in file order, unreadable entries skipped.
pub fn curve_points(raw: &Value, num: f64, den: f64) -> Vec<PointAt> {
    points_of(raw)
        .iter()
        .filter_map(|p| {
            let beat = beat_of(p, num, den)?;
            let value = match p.get("value") {
                Some(Value::Num(n)) => *n,
                _ => 0.0,
            };
            Some(PointAt { beat, value })
        })
        .collect()
}

fn index_at(raw: &Value, beat: f64, num: f64, den: f64) -> Option<usize> {
    points_of(raw)
        .iter()
        .position(|p| beat_of(p, num, den).is_some_and(|b| (b - beat).abs() < 1e-6))
}

fn keys(path: &[&str], i: Option<usize>) -> Vec<PathSeg> {
    let mut out = vec![PathSeg::Key(path[0].to_owned())];
    out.extend(i.map(PathSeg::Index));
    out.extend(path[1..].iter().map(|k| PathSeg::Key((*k).to_owned())));
    out
}

fn is_drawn(raw: &Value) -> Option<&str> {
    match raw.get("type") {
        Some(Value::Str(t)) if t == "custom" || t == "onoff" => Some(t.as_str()),
        _ => None,
    }
}

/// Puts a point at `beat`, in time order and one value per moment.
fn set_point(doc: &mut Document, beat: f64, value: f64, num: f64, den: f64) -> Res<()> {
    let raw = doc.to_js()?;
    let Some(kind) = is_drawn(&raw) else {
        return fail("not a drawn curve — only a `type: custom` or `onoff` one has points to edit");
    };
    // An on/off curve holds 0 or 1 and nothing between.
    let v = if kind == "onoff" {
        f64::from(u8::from(value >= 0.5))
    } else {
        value.clamp(0.0, 1.0)
    };
    if let Some(at) = index_at(&raw, beat, num, den) {
        return Ok(doc.set_in(
            &keys(&["points", "value"], Some(at)),
            &Value::Num(v),
            WriteOptions::default(),
        )?);
    }
    let point = Value::object(vec![
        ("time".to_owned(), Value::Str(to_position(beat, num, den)?)),
        ("value".to_owned(), Value::Num(v)),
    ]);
    if !matches!(raw.get("points"), Some(Value::List(_))) {
        return Ok(doc.set_in(
            &keys(&["points"], None),
            &Value::List(vec![point]),
            WriteOptions::default(),
        )?);
    }
    let points = points_of(&raw);
    let before = points
        .iter()
        .position(|p| beat_of(p, num, den).is_some_and(|b| b > beat))
        .unwrap_or(points.len());
    Ok(doc.insert_in(&keys(&["points"], None), before, &point, WriteOptions::default())?)
}

/// Takes the point at `beat` out; false when there is none.
fn remove_point(doc: &mut Document, beat: f64, num: f64, den: f64) -> Res<bool> {
    let raw = doc.to_js()?;
    let Some(at) = index_at(&raw, beat, num, den) else {
        return Ok(false);
    };
    Ok(doc.delete_in(&keys(&["points"], Some(at)), false)?)
}

/// Runs a point edit over one curve file; an edit answering false is `nothing to edit`.
fn edit_curve(h: &mut Host, id: &str, edit: impl FnOnce(&mut Document) -> Res<bool>) -> Res<String> {
    let rel = curve_rel(id);
    let path = join(&h.song_path, &rel);
    if !ws_exists(&*h.fs, &path) {
        return fail(format!("no curve '{id}' ({rel})"));
    }
    ws_edit(h.fs, &path, EditOpts::default(), |doc| {
        if edit(doc)? {
            Ok(())
        } else {
            fail(format!("nothing to edit in curve '{id}'"))
        }
    })?;
    Ok(rel)
}

/// `curve point add|move|remove <curve> …` — addressed by time, a value 0..1 of the curve.
fn point_command(a: &str, h: &mut Host, mode: &str) -> Done {
    let verb = format!("curve point {mode}");
    let w = split_words(a);
    if w.len() < if mode == "remove" { 2 } else { 3 } {
        h.refuse([match mode {
            "add" => format!("usage: {verb} <curve> <time> <value>   (value 0..1)"),
            "move" => format!("usage: {verb} <curve> <time> <newTime> [value]"),
            _ => format!("usage: {verb} <curve> <time>"),
        }]);
        return Ok(());
    }
    let p = h.project()?;
    let (num, den) = meter(&p);
    let (id, at_tc) = (w[0], w[1]);
    let at = match position(at_tc, &p) {
        Ok(b) => b,
        Err(m) => {
            h.refuse([format!("bad timecode: {m}")]);
            return Ok(());
        }
    };
    let unit = |raw: Option<&str>| {
        raw.map(song_edit::js_text::str_to_number)
            .filter(|v| v.is_finite())
            .map(|v| v.clamp(0.0, 1.0))
    };
    let file = join(&h.song_path, &curve_rel(id));
    let points = match ws_data(&*h.fs, &file) {
        Ok(raw) => curve_points(&raw, num, den),
        Err(e) => {
            h.refuse([format!("{verb}: {e}")]);
            return Ok(());
        }
    };
    let done = match mode {
        "add" => {
            let Some(v) = unit(w.get(2).copied()) else {
                h.refuse([format!("'{}' is not a value between 0 and 1", w[2])]);
                return Ok(());
            };
            let had = points.iter().any(|pt| (pt.beat - at).abs() < 1e-6);
            edit_curve(h, id, |doc| set_point(doc, at, v, num, den).map(|()| true)).map(|rel| {
                let what = if had { "set the point" } else { "added a point" };
                format!("{id}: {what} at {at_tc} to {} ({rel})", js_number(v))
            })
        }
        "move" => {
            let to = match position(w[2], &p) {
                Ok(b) => b,
                Err(m) => {
                    h.refuse([format!("bad timecode: {m}")]);
                    return Ok(());
                }
            };
            let value = unit(w.get(3).copied());
            edit_curve(h, id, |doc| {
                let raw = doc.to_js()?;
                let Some(here) = curve_points(&raw, num, den)
                    .into_iter()
                    .find(|pt| (pt.beat - at).abs() < 1e-6)
                else {
                    return Ok(false);
                };
                if !remove_point(doc, at, num, den)? {
                    return Ok(false);
                }
                set_point(doc, to, value.unwrap_or(here.value), num, den)?;
                Ok(true)
            })
            .map(|rel| format!("{id}: the point at {at_tc} is now at {} ({rel})", w[2]))
        }
        _ => {
            // The LAST point is not removable: a curve file with none does not load.
            if points.len() <= 1 {
                h.refuse([
                    format!("'{id}' has one point left, and a curve with none does not load"),
                    "  to stop automating, use: track automation remove <track> <param>".to_owned(),
                ]);
                return Ok(());
            }
            edit_curve(h, id, |doc| remove_point(doc, at, num, den))
                .map(|rel| format!("{id}: removed the point at {at_tc} ({rel})"))
        }
    };
    match done {
        Ok(line) => {
            h.reload();
            h.log([line]);
        }
        Err(e) => h.refuse([format!("{verb}: {e}")]),
    }
    Ok(())
}

pub fn point_add(a: &str, h: &mut Host) -> Done {
    point_command(a, h, "add")
}

pub fn point_move(a: &str, h: &mut Host) -> Done {
    point_command(a, h, "move")
}

pub fn point_remove(a: &str, h: &mut Host) -> Done {
    point_command(a, h, "remove")
}
