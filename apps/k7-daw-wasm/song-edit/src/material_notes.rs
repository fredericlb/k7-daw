// material_notes.rs — what cutting and merging MATERIAL share: a sequence's notes, a placement's window, a new sequence written.
// A reader would get wrong whose window wins: the MOST restrictive edge of the material and the placement, as the compiler reads it.

use crate::error::Res;
use crate::fs::Fs;
use crate::material_file::material_window_of;
use crate::module_io::entity_file_for;
use crate::paths::join;
use crate::start_time::{js_string, js_to_number, position, to_fixed6, to_position};
use crate::workspace_doc::{ws_data, ws_write_data};
use k7x::edit::{StringifyOptions, Value};
use std::collections::HashSet;

/// Two placements at the same beat are the same placement, so this is what "the same" means.
pub const EDGE_EPS: f64 = 1e-6;

/// A note as a sequence file holds it.
#[derive(Clone, Debug, PartialEq)]
pub struct Note {
    pub t: f64,
    pub d: f64,
    pub k: f64,
    pub v: Option<Value>,
}

/// One material this wrote: what it is called, where it landed, and how much is in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WrittenMaterial {
    pub ref_: String,
    pub file: String,
    pub notes: usize,
}

/// The window a placement of a sequence actually plays, in the material's own beats.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Window {
    pub from: f64,
    pub to: Option<f64>,
}

/// A free name under `sequences/`: the number keeps climbing, so nothing is overwritten.
pub fn free_sequence_name(fs: &dyn Fs, dir: &str, base: &str, taken: &mut HashSet<String>) -> String {
    let mut n = 1;
    loop {
        let name = format!("{base}-{n}");
        if !taken.contains(&name) && entity_file_for(fs, &join(&[dir, "sequences"]), "sequences", &name).is_none() {
            taken.insert(name.clone());
            return name;
        }
        n += 1;
    }
}

fn finite(v: Option<&Value>) -> Option<f64> {
    match v {
        Some(Value::Num(n)) if n.is_finite() => Some(*n),
        _ => None,
    }
}

/// The notes a sequence file holds, its short form expanded; anything unreadable is no notes at all.
#[must_use]
pub fn notes_of(fs: &dyn Fs, file: &str) -> Vec<Note> {
    let Ok(raw) = ws_data(fs, file) else { return vec![] };
    let Some(Value::List(notes)) = raw.get("notes") else {
        return vec![];
    };
    notes
        .iter()
        .filter_map(|n| {
            Some(Note {
                t: finite(n.get("t"))?,
                d: finite(n.get("d"))?,
                k: finite(n.get("k"))?,
                v: n.get("v").cloned(),
            })
        })
        .collect()
}

/// The notes sounding inside `[from, to)`, rebased to beat 0; a straddling note is TRUNCATED.
#[must_use]
pub fn notes_in(notes: &[Note], from: f64, to: f64) -> Vec<Note> {
    let mut out = vec![];
    for n in notes {
        let start = n.t.max(from);
        let end = (n.t + n.d).min(to);
        if end - start <= EDGE_EPS {
            continue;
        }
        out.push(Note {
            t: start - from,
            d: end - start,
            ..n.clone()
        });
    }
    sort_notes(&mut out);
    out
}

pub fn sort_notes(notes: &mut [Note]) {
    notes.sort_by(|a, b| a.t.total_cmp(&b.t).then(a.k.total_cmp(&b.k)));
}

/// What a written sequence declares: its edges, and the verb that wrote it.
pub struct Edges<'a> {
    pub from: Option<f64>,
    pub to: f64,
    pub verb: &'a str,
}

/// Writes `sequences/<ref>.k7x`; which edges it declares differs by verb (docs/materialCut.md).
///
/// # Errors
/// A position out of range, or the write's.
pub fn write_sequence(
    fs: &mut dyn Fs,
    dir: &str,
    ref_: &str,
    notes: &[Note],
    edges: Option<&Edges>,
    meter: (f64, f64),
) -> Res<WrittenMaterial> {
    let rel = format!("sequences/{ref_}.k7x");
    let mut data: Vec<(String, Value)> = vec![];
    if let Some(from) = edges.and_then(|e| e.from) {
        data.push(("from".to_owned(), Value::Str(to_position(from, meter.0, meter.1)?)));
    }
    if let Some(e) = edges.filter(|e| e.to > EDGE_EPS) {
        data.push(("to".to_owned(), Value::Str(to_position(e.to, meter.0, meter.1)?)));
    }
    let rows = notes.iter().map(|n| {
        let mut row = vec![
            ("t".to_owned(), Value::Num(to_fixed6(n.t))),
            ("d".to_owned(), Value::Num(to_fixed6(n.d))),
            ("k".to_owned(), Value::Num(n.k)),
        ];
        row.extend(n.v.clone().map(|v| ("v".to_owned(), v)));
        Value::Map(row)
    });
    data.push(("notes".to_owned(), Value::List(rows.collect())));
    let header = [format!(
        "Written by `{}`.",
        edges.map_or("selection consolidate", |e| e.verb)
    )];
    ws_write_data(
        fs,
        &join(&[dir, &rel]),
        &Value::Map(data),
        &StringifyOptions {
            header: &header,
            inline: None,
        },
    )?;
    Ok(WrittenMaterial {
        ref_: ref_.to_owned(),
        file: rel,
        notes: notes.len(),
    })
}

/// The window a placement of `ref_` actually plays, in the material's own beats.
///
/// # Errors
/// A material file that does not read, or an edge that is not a position.
pub fn window_of(fs: &dyn Fs, dir: &str, ref_: &str, placement: &Value, meter: (f64, f64)) -> Res<Window> {
    let file = entity_file_for(fs, &join(&[dir, "sequences"]), "sequences", ref_);
    let pos =
        |v: Option<&Value>| -> Res<Option<f64>> { v.map(|v| position(&js_string(v), meter.0, meter.1)).transpose() };
    let mat = match &file {
        Some(f) => material_window_of(fs, &f.path)?,
        None => Default::default(),
    };
    let own = |k: &str| placement.get(k).filter(|v| matches!(v, Value::Num(_) | Value::Str(_)));
    let m_from = pos(mat.from.as_ref())?.unwrap_or(0.0);
    let m_to = pos(mat.to.as_ref())?;
    let p_from = pos(own("from"))?;
    let p_to = pos(own("to"))?;
    // The most restrictive edge wins, the same rule the compiler applies.
    let from = m_from.max(p_from.unwrap_or(0.0));
    let to = match (m_to, p_to) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    };
    Ok(Window { from, to })
}

/// `String(p.ref ?? "")` of a placement.
#[must_use]
pub fn ref_of(p: &Value) -> String {
    p.get("ref")
        .filter(|v| !matches!(v, Value::Null))
        .map_or(String::new(), js_string)
}

/// `Math.max(1, Math.floor(Number(p.loop ?? 1)) || 1)`: the passes a placement plays.
#[must_use]
pub fn loops_of(p: &Value) -> f64 {
    let n = p
        .get("loop")
        .filter(|v| !matches!(v, Value::Null))
        .map_or(1.0, js_to_number)
        .floor();
    let n = if n.is_nan() || n == 0.0 { 1.0 } else { n };
    n.max(1.0)
}
