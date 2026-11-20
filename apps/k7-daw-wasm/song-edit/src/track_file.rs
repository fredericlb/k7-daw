// track_file.rs — editing the PLACEMENTS on a lane, a lane being a node file: song-schema's trackFile.ts over a `Scope`.
// A reader would get wrong WHICH list is edited: a lane on one of its alternatives has two, the playing one and the module's own.

use crate::error::{Res, fail};
use crate::module_file::current_alternative;
use crate::start_time::{above, js_string, js_to_number, start_as, start_beats, start_tolerance};
use crate::time_splice::{Mode, SplicedList, TimeSplice, splice_list};
use k7x::ListForm;
use k7x::edit::{PathSeg, Scope, Tree, Value, WriteOptions};

pub fn key(k: &str) -> PathSeg {
    PathSeg::Key(k.to_owned())
}

fn at(path: &[PathSeg], more: &[PathSeg]) -> Vec<PathSeg> {
    [path, more].concat()
}

/// A plain value set under `k` in a map, replacing in place or appended last, as `{...m, k: v}` does.
pub fn with_field(map: &Value, k: &str, v: Option<Value>) -> Value {
    let Value::Map(entries) = map else { return map.clone() };
    let mut out: Vec<(String, Value)> = entries.clone();
    match (out.iter().position(|(ek, _)| ek == k), v) {
        (Some(i), Some(v)) => out[i].1 = v,
        (Some(i), None) => {
            out.remove(i);
        }
        (None, Some(v)) => out.push((k.to_owned(), v)),
        (None, None) => {}
    }
    Value::object(out)
}

/// The module's own arrangement, where a lane with no sets keeps its events.
fn own_events() -> Vec<PathSeg> {
    vec![key("events")]
}

fn set_events(n: usize) -> Vec<PathSeg> {
    vec![key("alternatives"), PathSeg::Index(n - 1), key("events")]
}

pub fn list_at(doc: &Scope, path: &[PathSeg]) -> Res<Option<Vec<Value>>> {
    Ok(match doc.get_in(path)? {
        Some(Value::List(l)) => Some(l),
        _ => None,
    })
}

/// Which attribute set is ON, 0 for the module's own.
fn selected_set(doc: &Scope) -> Res<usize> {
    let raw = doc.to_js()?;
    Ok(match raw {
        Some(m @ Value::Map(_)) => current_alternative(&m),
        _ => 0,
    })
}

/// The events list that is PLAYING: the selected set's own, else the module's.
///
/// # Errors
/// A document that does not read.
pub fn events_in(doc: &Scope) -> Res<Vec<PathSeg>> {
    let n = selected_set(doc)?;
    if n == 0 {
        return Ok(own_events());
    }
    let mine = set_events(n);
    Ok(if list_at(doc, &mine)?.is_some() {
        mine
    } else {
        own_events()
    })
}

/// The events list an edit lands in, forking the selected set's on its first edit.
///
/// # Errors
/// A document that does not read, or the write's.
pub fn events_to_edit(doc: &mut Scope) -> Res<Vec<PathSeg>> {
    let n = selected_set(doc)?;
    if n == 0 {
        return Ok(own_events());
    }
    let mine = set_events(n);
    if list_at(doc, &mine)?.is_some() {
        return Ok(mine);
    }
    let own = own_events();
    let form = doc.node_at(&own, |t| match t {
        Some(Tree::List { form, .. }) => Some(*form),
        _ => None,
    })?;
    let source = if form.is_some() { doc.get_source(&own)? } else { None };
    // A call's own arguments, `events(a, b)`, are a list only once bracketed.
    let inherited = match (form, source) {
        (Some(ListForm::Args), Some(s)) => Some(format!("[{s}]")),
        (_, s) => s,
    };
    match inherited.filter(|s| !s.is_empty()) {
        Some(s) => doc.set_source_in(&mine, &s)?,
        None => doc.set_in(&mine, &Value::List(vec![]), WriteOptions::default())?,
    }
    Ok(mine)
}

fn append_placement(doc: &mut Scope, path: &[PathSeg], value: &Value) -> Res<()> {
    if list_at(doc, path)?.is_some() {
        Ok(doc.add_in(path, value, WriteOptions { inline: true })?)
    } else {
        Ok(doc.set_in(path, &Value::List(vec![value.clone()]), WriteOptions::default())?)
    }
}

/// Appends a placement, on a line of its own.
///
/// # Errors
/// What the write fails with.
pub fn add_placement(doc: &mut Scope, placement: &Value) -> Res<()> {
    let path = events_to_edit(doc)?;
    append_placement(doc, &path, placement)
}

/// The index of the placement whose `start` reads as `beats`, in either unit.
///
/// # Errors
/// A document that does not read.
pub fn find_placement_at(doc: &Scope, beats: f64, num: f64, den: f64, tempo: f64) -> Res<Option<usize>> {
    let events = list_at(doc, &events_in(doc)?)?.unwrap_or_default();
    Ok(events.iter().position(|p| {
        let start = match p {
            Value::Map(_) => p.get("start"),
            _ => None,
        };
        start_beats(start, num, den, tempo, "1:1:1").is_ok_and(|b| (b - beats).abs() < start_tolerance(start))
    }))
}

/// The placement at `index` in the playing list as plain values.
///
/// # Errors
/// A document that does not read.
pub fn placement_at(doc: &Scope, index: usize) -> Res<Option<Value>> {
    let list = list_at(doc, &events_in(doc)?)?.unwrap_or_default();
    Ok(list.into_iter().nth(index).filter(|p| matches!(p, Value::Map(_))))
}

/// Every placement in the playing list; one that is not a map is an empty one.
///
/// # Errors
/// A document that does not read.
pub fn placement_values(doc: &Scope) -> Res<Vec<Value>> {
    let list = list_at(doc, &events_in(doc)?)?.unwrap_or_default();
    Ok(list
        .into_iter()
        .map(|p| {
            if matches!(p, Value::Map(_)) {
                p
            } else {
                Value::Map(vec![])
            }
        })
        .collect())
}

/// Drops the placement at `index`.
///
/// # Errors
/// What the write fails with.
pub fn remove_placement_at(doc: &mut Scope, index: usize) -> Res<()> {
    let path = events_to_edit(doc)?;
    doc.delete_in(&at(&path, &[PathSeg::Index(index)]), false)?;
    Ok(())
}

/// Sets, or with None clears, one field of the placement at `index`.
///
/// # Errors
/// What the write fails with.
pub fn set_placement_field(doc: &mut Scope, index: usize, field: &str, value: Option<Value>) -> Res<()> {
    let path = events_to_edit(doc)?;
    let target = at(&path, &[PathSeg::Index(index), key(field)]);
    match value {
        None => doc.delete_in(&target, false).map(|_| ())?,
        Some(v) => doc.set_in(&target, &v, WriteOptions::default())?,
    }
    Ok(())
}

/// Copies the placement at `index`, patches the copy and appends it: what a split writes.
///
/// # Errors
/// No placement at `index`, or the write's.
pub fn copy_placement_at(doc: &mut Scope, index: usize, patch: &[(&str, Option<Value>)]) -> Res<()> {
    let path = events_to_edit(doc)?;
    let src = list_at(doc, &path)?.unwrap_or_default().into_iter().nth(index);
    let Some(mut merged @ Value::Map(_)) = src else {
        return fail(format!("no placement at index {index}"));
    };
    for (k, v) in patch {
        merged = with_field(&merged, k, v.clone());
    }
    append_placement(doc, &path, &merged)
}

/// Every list of placements this file holds: what a change to the song's CLOCK must reach.
fn every_event_list(doc: &Scope) -> Res<Vec<Vec<PathSeg>>> {
    let mut out = vec![];
    if list_at(doc, &own_events())?.is_some() {
        out.push(own_events());
    }
    let alts = list_at(doc, &[key("alternatives")])?.unwrap_or_default();
    for i in 0..alts.len() {
        let path = vec![key("alternatives"), PathSeg::Index(i), key("events")];
        if list_at(doc, &path)?.is_some() {
            out.push(path);
        }
    }
    Ok(out)
}

/// Why a loop halving passed a placement by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeptWhy {
    Odd,
    Windowed,
}

/// A placement this lane places `ref` with, and why a loop halving passed it by.
#[derive(Clone, Debug, PartialEq)]
pub struct KeptLoop {
    pub start: Value,
    pub loop_: f64,
    pub why: KeptWhy,
}

/// Halves the `loop` of every placement of `ref` that takes the material's WHOLE window; how many it halved and what it kept.
///
/// # Errors
/// What the write fails with.
pub fn halve_sequence_loops(doc: &mut Scope, ref_: &str) -> Res<(usize, Vec<KeptLoop>)> {
    let mut kept = vec![];
    let mut halved = 0;
    for path in every_event_list(doc)? {
        let items = list_at(doc, &path)?.unwrap_or_default();
        for (i, item) in items.iter().enumerate() {
            if !matches!(item, Value::Map(_)) || item.get("ref").map_or(String::new(), js_string_nn) != ref_ {
                continue;
            }
            let loop_ = item
                .get("loop")
                .filter(|v| !matches!(v, Value::Null))
                .map_or(1.0, js_to_number);
            if loop_.fract() != 0.0 || !loop_.is_finite() || loop_ < 2.0 {
                continue;
            }
            let start = item
                .get("start")
                .filter(|v| !matches!(v, Value::Null))
                .cloned()
                .unwrap_or_else(|| Value::Str("1:1:1".to_owned()));
            let has = |k: &str| item.get(k).is_some_and(|v| !matches!(v, Value::Null));
            if has("from") || has("to") {
                kept.push(KeptLoop {
                    start,
                    loop_,
                    why: KeptWhy::Windowed,
                });
                continue;
            }
            if loop_ % 2.0 != 0.0 {
                kept.push(KeptLoop {
                    start,
                    loop_,
                    why: KeptWhy::Odd,
                });
                continue;
            }
            let target = at(&path, &[PathSeg::Index(i), key("loop")]);
            // Back to ONE pass is the default, and the default is written by not writing it.
            if loop_ == 2.0 {
                doc.delete_in(&target, false)?;
            } else {
                doc.set_in(&target, &Value::Num(loop_ / 2.0), WriteOptions::default())?;
            }
            halved += 1;
        }
    }
    Ok((halved, kept))
}

/// `String(v ?? "")`.
fn js_string_nn(v: &Value) -> String {
    if matches!(v, Value::Null) {
        String::new()
    } else {
        js_string(v)
    }
}

struct Events<'a, 'd> {
    doc: &'a mut Scope<'d>,
    path: Vec<PathSeg>,
    items: Vec<Value>,
    meter: (f64, f64, f64),
}

impl SplicedList for Events<'_, '_> {
    fn count(&self) -> usize {
        self.items.len()
    }

    fn position_of(&mut self, i: usize) -> Option<f64> {
        let item = self.items.get(i).filter(|v| matches!(v, Value::Map(_)))?;
        let (num, den, tempo) = self.meter;
        start_beats(item.get("start"), num, den, tempo, "1:1:1").ok()
    }

    fn remove(&mut self, i: usize) -> Res<()> {
        self.doc.delete_in(&at(&self.path, &[PathSeg::Index(i)]), false)?;
        Ok(())
    }

    fn shift(&mut self, i: usize, to: f64) -> Res<()> {
        let (num, den, tempo) = self.meter;
        let like = self.items.get(i).and_then(|v| v.get("start"));
        let v = start_as(like, to, num, den, tempo)?;
        let target = at(&self.path, &[PathSeg::Index(i), key("start")]);
        Ok(self.doc.set_in(&target, &v, WriteOptions::default())?)
    }
}

/// Inserts or deletes time across every placement list, an event straddling the cut going by its `start`.
///
/// # Errors
/// A start that does not write back, or the write's.
pub fn splice_time(
    doc: &mut Scope,
    at_beats: f64,
    len_beats: f64,
    mode: Mode,
    meter: (f64, f64, f64),
) -> Res<TimeSplice> {
    let mut out = TimeSplice::default();
    if !above(len_beats, 0.0) {
        return Ok(out);
    }
    for path in every_event_list(doc)? {
        let items = list_at(doc, &path)?.unwrap_or_default();
        let mut list = Events {
            doc: &mut *doc,
            path,
            items,
            meter,
        };
        splice_list(&mut list, at_beats, len_beats, mode, &mut out)?;
    }
    Ok(out)
}
