// take_file.rs — writing an event's TAKES and its COMP into a lane file, after song-schema's takeFile.ts over a `Scope`.
// A reader would get wrong the unit: comp spans are positions in SONG time, read to beats for the arithmetic and written back.

use crate::error::{Res, fail};
use crate::start_time::{js_string, position, start_beats, to_position};
use crate::takes::{CompSpan, set_comp_span};
use crate::track_file::{events_to_edit, key};
use k7x::edit::{PathSeg, Scope, Value, WriteOptions};

/// A map's field as `String(v)` spells it, `undefined` for an absent one.
fn text_of(v: &Value, k: &str) -> String {
    match v.get(k) {
        None => "undefined".to_owned(),
        Some(x) => js_string(x),
    }
}

pub(crate) fn placement_of(doc: &Scope, at: &[PathSeg]) -> Res<Value> {
    Ok(match doc.get_in(at)? {
        Some(p @ Value::Map(_)) => p,
        _ => Value::Map(vec![]),
    })
}

fn under(at: &[PathSeg], k: &str) -> Vec<PathSeg> {
    let mut p = at.to_vec();
    p.push(key(k));
    p
}

struct Takes {
    ref_: String,
    start: Option<Value>,
    takes: Vec<Value>,
    comp: Vec<CompSpan>,
}

fn list_of(p: &Value, k: &str) -> Vec<Value> {
    match p.get(k) {
        Some(Value::List(l)) => l.clone(),
        _ => vec![],
    }
}

/// The take entries and comp spans of the placement at `at`, the comp in BEATS.
fn takes_of(doc: &Scope, at: &[PathSeg], num: f64, den: f64) -> Res<Takes> {
    let p = placement_of(doc, at)?;
    let mut comp = vec![];
    for s in list_of(&p, "comp") {
        comp.push(CompSpan {
            take: text_of(&s, "take"),
            from: position(&text_of(&s, "from"), num, den)?,
            to: position(&text_of(&s, "to"), num, den)?,
        });
    }
    Ok(Takes {
        ref_: p
            .get("ref")
            .filter(|v| !matches!(v, Value::Null))
            .map_or(String::new(), js_string),
        start: p.get("start").cloned(),
        takes: list_of(&p, "takes"),
        comp,
    })
}

fn has_ref(takes: &[Value], r: &str) -> bool {
    takes
        .iter()
        .any(|t| matches!(t.get("ref"), Some(Value::Str(s)) if s == r))
}

/// Writes a list back with the fewest edits, or drops the key when it is empty.
pub(crate) fn write_list(doc: &mut Scope, at: &[PathSeg], k: &str, list: Vec<Value>) -> Res<()> {
    let path = under(at, k);
    if list.is_empty() {
        doc.delete_in(&path, false)?;
    } else if matches!(doc.get_in(&path)?, Some(Value::List(_))) {
        doc.apply_in(&path, &Value::List(list))?;
    } else {
        doc.set_in(&path, &Value::List(list), WriteOptions::default())?;
    }
    Ok(())
}

fn write_comp(doc: &mut Scope, at: &[PathSeg], comp: &[CompSpan], num: f64, den: f64) -> Res<()> {
    let mut spans = vec![];
    for s in comp {
        spans.push(Value::object(vec![
            ("take".to_owned(), Value::Str(s.take.clone())),
            ("from".to_owned(), Value::Str(to_position(s.from, num, den)?)),
            ("to".to_owned(), Value::Str(to_position(s.to, num, den)?)),
        ]));
    }
    write_list(doc, at, "comp", spans)
}

fn event_path(doc: &mut Scope, index: usize) -> Res<Vec<PathSeg>> {
    let mut at = events_to_edit(doc)?;
    at.push(PathSeg::Index(index));
    Ok(at)
}

/// A span to lay over a comp, its edges song positions.
pub struct Span<'a> {
    pub take: &'a str,
    pub from: &'a str,
    pub to: &'a str,
}

/// Lays one span over the comp of the placement at `index`; `take` its own `ref` puts the event's material back.
///
/// # Errors
/// A take the event does not have, a span that ends before it starts, a time that does not read, or the write's.
pub fn set_comp_at(doc: &mut Scope, index: usize, span: &Span, num: f64, den: f64, tempo: f64) -> Res<()> {
    let at = event_path(doc, index)?;
    let t = takes_of(doc, &at, num, den)?;
    let start = start_beats(t.start.as_ref(), num, den, tempo, "1")?;
    if span.take != t.ref_ && !has_ref(&t.takes, span.take) {
        return fail(format!("no take \"{}\" on this event", span.take));
    }
    // Never before the event's own start: a comped event begins where its placement does.
    let from = start.max(position(span.from, num, den)?);
    let to = position(span.to, num, den)?;
    if to <= from {
        return fail("a comp span must end after it starts");
    }
    let next = set_comp_span(
        &t.comp,
        &CompSpan {
            take: span.take.to_owned(),
            from,
            to,
        },
        &t.ref_,
    );
    write_comp(doc, &at, &next, num, den)
}

/// Adds a take to the placement at `index` and comps it in over `span`; None adds it unheard.
///
/// # Errors
/// What `set_comp_at` fails with, or the write's.
pub fn add_take_at(
    doc: &mut Scope,
    index: usize,
    take: (&str, &str),
    span: Option<(&str, &str)>,
    num: f64,
    den: f64,
    tempo: f64,
) -> Res<()> {
    let at = event_path(doc, index)?;
    let path = under(&at, "takes");
    let entry = Value::object(vec![
        ("ref".to_owned(), Value::Str(take.0.to_owned())),
        ("start".to_owned(), Value::Str(take.1.to_owned())),
    ]);
    if matches!(doc.get_in(&path)?, Some(Value::List(_))) {
        doc.add_in(&path, &entry, WriteOptions { inline: true })?;
    } else {
        doc.set_in(&path, &Value::List(vec![entry]), WriteOptions::default())?;
    }
    if let Some((from, to)) = span {
        set_comp_at(doc, index, &Span { take: take.0, from, to }, num, den, tempo)?;
    }
    Ok(())
}

/// Drops a take from the placement at `index`, and every span of the comp it played.
///
/// # Errors
/// A take the event does not have, or the write's.
pub fn remove_take_at(doc: &mut Scope, index: usize, ref_: &str, num: f64, den: f64) -> Res<()> {
    let at = event_path(doc, index)?;
    let t = takes_of(doc, &at, num, den)?;
    if !has_ref(&t.takes, ref_) {
        return fail(format!("no take \"{ref_}\" on this event"));
    }
    let kept = t
        .takes
        .into_iter()
        .filter(|x| !matches!(x.get("ref"), Some(Value::Str(s)) if s == ref_))
        .collect();
    write_list(doc, &at, "takes", kept)?;
    let comp: Vec<CompSpan> = t.comp.into_iter().filter(|s| s.take != ref_).collect();
    write_comp(doc, &at, &comp, num, den)
}

/// Sets the crossfade at the seam `seam` (a song position) of the placement at `index`; a None length removes it.
///
/// # Errors
/// A time that does not read, or the write's.
pub fn set_crossfade_at(
    doc: &mut Scope,
    index: usize,
    seam: &str,
    length: Option<&str>,
    num: f64,
    den: f64,
) -> Res<()> {
    let path = event_path(doc, index)?;
    let p = placement_of(doc, &path)?;
    let beat = position(seam, num, den)?;
    let mut list: Vec<(f64, Value)> = vec![];
    for x in list_of(&p, "crossfades") {
        let b = position(&text_of(&x, "at"), num, den)?;
        if (b - beat).abs() > 1e-6 {
            list.push((b, x));
        }
    }
    if let Some(len) = length {
        let entry = Value::object(vec![
            ("at".to_owned(), Value::Str(seam.to_owned())),
            ("length".to_owned(), Value::Str(len.to_owned())),
        ]);
        list.push((beat, entry));
    }
    list.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    write_list(doc, &path, "crossfades", list.into_iter().map(|(_, v)| v).collect())
}
