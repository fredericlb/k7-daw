// sequence_file.rs — editing a `sequences/<id>.k7x` document: one note by (key, start), a region of notes, the doubling.
// A reader would get wrong what these see: the EXPANDED notes, since `ws_edit` hands them the short form already undone.

use k7x::edit::{Document, PathSeg, Value, WriteOptions};
use song_edit::error::{Res, fail};
use song_edit::js_text::{js_string, str_to_number};
use song_edit::start_time::js_round;
use song_schema::{beats_to_position_on, parse_position_on};

/// Two beat positions closer than this are the same one.
const EPS: f64 = 1e-6;

/// A note as plain values; `repeat` and `skip` are a pad's effects, carried so a copy keeps them.
#[derive(Clone, Debug, PartialEq)]
pub struct NoteIn {
    pub t: f64,
    pub d: f64,
    pub k: f64,
    pub v: Option<f64>,
    pub repeat: Option<f64>,
    pub skip: Option<f64>,
}

impl NoteIn {
    pub fn new(t: f64, d: f64, k: f64, v: Option<f64>) -> NoteIn {
        NoteIn {
            t,
            d,
            k,
            v,
            repeat: None,
            skip: None,
        }
    }
}

/// `isPlayableNote`: finite times, a positive length, a whole MIDI key.
pub fn is_playable(n: &NoteIn) -> bool {
    n.t.is_finite() && n.d.is_finite() && n.d > 0.0 && n.k.fract() == 0.0 && (0.0..=127.0).contains(&n.k)
}

/// `Number(x ?? NaN)` over a field a file holds.
fn number(v: Option<&Value>) -> f64 {
    match v {
        None | Some(Value::Null) => f64::NAN,
        Some(Value::Num(n)) => *n,
        Some(Value::Bool(b)) => f64::from(u8::from(*b)),
        Some(Value::Str(s)) => str_to_number(s),
        Some(_) => f64::NAN,
    }
}

fn num_field(raw: &Value, key: &str) -> Option<f64> {
    match raw.get(key) {
        Some(Value::Num(n)) => Some(*n),
        _ => None,
    }
}

fn key(k: &str) -> PathSeg {
    PathSeg::Key(k.to_owned())
}

fn at(i: usize, field: &str) -> Vec<PathSeg> {
    vec![key("notes"), PathSeg::Index(i), key(field)]
}

/// The note list as written, empty when there is none.
fn notes_of(doc: &Document) -> Res<Vec<Value>> {
    Ok(match doc.get_in(&[key("notes")])? {
        Some(Value::List(l)) => l,
        _ => Vec::new(),
    })
}

fn index_in(notes: &[Value], k: f64, t: f64) -> Option<usize> {
    notes
        .iter()
        .position(|n| matches!(n, Value::Map(_)) && number(n.get("k")) == k && (number(n.get("t")) - t).abs() < EPS)
}

/// The index of the note an edit addresses; a miss is the edit's failure.
fn note_at(doc: &Document, k: f64, t: f64) -> Res<usize> {
    match index_in(&notes_of(doc)?, k, t) {
        Some(i) => Ok(i),
        None => fail(format!("no note k{} at beat {}", js_number(k), js_number(t))),
    }
}

fn js_number(x: f64) -> String {
    k7x::edit::js_number(x)
}

fn note_value(n: &NoteIn) -> Value {
    let mut m = vec![
        ("t".to_owned(), Value::Num(n.t)),
        ("d".to_owned(), Value::Num(n.d)),
        ("k".to_owned(), Value::Num(n.k)),
    ];
    m.extend(n.v.map(|v| ("v".to_owned(), Value::Num(v))));
    m.extend(
        n.repeat
            .filter(|r| *r != 0.0)
            .map(|r| ("repeat".to_owned(), Value::Num(r))),
    );
    m.extend(n.skip.filter(|s| *s != 0.0).map(|s| ("skip".to_owned(), Value::Num(s))));
    Value::Map(m)
}

fn set(doc: &mut Document, path: &[PathSeg], v: f64) -> Res<()> {
    Ok(doc.set_in(path, &Value::Num(v), WriteOptions::default())?)
}

fn delete(doc: &mut Document, path: &[PathSeg]) -> Res<()> {
    doc.delete_in(path, false)?;
    Ok(())
}

/// `appendNotes`: each note on a line of its own, the list made when absent.
pub fn append_notes(doc: &mut Document, notes: &[NoteIn]) -> Res<()> {
    if notes.is_empty() {
        return Ok(());
    }
    let values: Vec<Value> = notes.iter().map(note_value).collect();
    if !matches!(doc.get_in(&[key("notes")])?, Some(Value::List(_))) {
        return set_value(doc, &[key("notes")], &Value::List(values));
    }
    for v in &values {
        doc.add_in(&[key("notes")], v, WriteOptions { inline: true })?;
    }
    Ok(())
}

pub fn set_value(doc: &mut Document, path: &[PathSeg], v: &Value) -> Res<()> {
    Ok(doc.set_in(path, v, WriteOptions::default())?)
}

/// `setNoteTimeRange`: one note moved and/or stretched.
pub fn set_note_time_range(doc: &mut Document, k: f64, t: f64, new_t: f64, new_d: f64) -> Res<()> {
    let i = note_at(doc, k, t)?;
    if new_d.is_nan() || new_d <= 0.0 {
        return fail("a note has to be longer than nothing");
    }
    if new_t < 0.0 {
        return fail("a note cannot start before the sequence does");
    }
    set(doc, &at(i, "t"), new_t)?;
    set(doc, &at(i, "d"), new_d)
}

pub fn remove_note(doc: &mut Document, k: f64, t: f64) -> Res<()> {
    let i = note_at(doc, k, t)?;
    delete(doc, &[key("notes"), PathSeg::Index(i)])
}

/// `moveNote`: the same note on another key and/or start, refused onto a held spot.
pub fn move_note(doc: &mut Document, k: f64, t: f64, new_k: f64, new_t: f64) -> Res<()> {
    let i = note_at(doc, k, t)?;
    if new_t < 0.0 {
        return fail("a note cannot start before the sequence does");
    }
    let same = new_k == k && (new_t - t).abs() < EPS;
    if !same && index_in(&notes_of(doc)?, new_k, new_t).is_some() {
        return fail(format!(
            "k{} already has a note at beat {}",
            js_number(new_k),
            js_number(new_t)
        ));
    }
    set(doc, &at(i, "k"), new_k)?;
    set(doc, &at(i, "t"), new_t)
}

/// One note's way elsewhere: key `k` at `t` to key `nk` at `nt`.
pub struct NoteMove {
    pub k: f64,
    pub t: f64,
    pub nk: f64,
    pub nt: f64,
}

/// `moveNotes`: every index read before any write; a target is free unless a note that STAYS holds it.
pub fn move_notes(doc: &mut Document, moves: &[NoteMove]) -> Res<()> {
    if moves.is_empty() {
        return fail("no note to move");
    }
    let notes = notes_of(doc)?;
    if notes.is_empty() {
        return fail("the sequence has no notes");
    }
    let mut from = Vec::with_capacity(moves.len());
    for m in moves {
        match index_in(&notes, m.k, m.t) {
            Some(i) => from.push(i),
            None => return fail(format!("no note k{} at beat {}", js_number(m.k), js_number(m.t))),
        }
    }
    let mut unique = from.clone();
    unique.sort_unstable();
    unique.dedup();
    if unique.len() != from.len() {
        return fail("the same note is moved twice");
    }
    let mut landing: Vec<(f64, f64)> = Vec::new();
    for m in moves {
        if m.nk.fract() != 0.0 || !(0.0..=127.0).contains(&m.nk) {
            return fail(format!("k{} is not a MIDI key", js_number(m.nk)));
        }
        if !m.nt.is_finite() || m.nt < 0.0 {
            return fail("a note cannot start before the sequence does");
        }
        if index_in(&notes, m.nk, m.nt).is_some_and(|there| !from.contains(&there)) {
            return fail(format!(
                "k{} already has a note at beat {}",
                js_number(m.nk),
                js_number(m.nt)
            ));
        }
        let spot = (m.nk, js_round(m.nt / EPS));
        if landing.contains(&spot) {
            return fail(format!(
                "two notes would land on k{} at beat {}",
                js_number(m.nk),
                js_number(m.nt)
            ));
        }
        landing.push(spot);
    }
    for (m, i) in moves.iter().zip(from) {
        set(doc, &at(i, "k"), m.nk)?;
        set(doc, &at(i, "t"), m.nt)?;
    }
    Ok(())
}

/// `splitNote`: two notes at `at`, strictly inside the one; the velocity rides onto both.
pub fn split_note(doc: &mut Document, k: f64, t: f64, cut: f64) -> Res<()> {
    let i = note_at(doc, k, t)?;
    let d = number(doc.get_in(&at(i, "d"))?.as_ref());
    if !d.is_finite() || d <= 0.0 {
        return fail(format!(
            "the note k{} at beat {} has no length to split",
            js_number(k),
            js_number(t)
        ));
    }
    if cut <= t + EPS || cut >= t + d - EPS {
        return fail(format!(
            "{} is not inside the note — it runs {} to {}, and a split has to leave two",
            js_number(cut),
            js_number(t),
            js_number(t + d)
        ));
    }
    let v = match doc.get_in(&at(i, "v"))? {
        Some(Value::Num(v)) => Some(v),
        _ => None,
    };
    set(doc, &at(i, "d"), cut - t)?;
    append_notes(doc, &[NoteIn::new(cut, t + d - cut, k, v)])
}

/// `setNoteVelocity`: None CLEARS the key rather than writing `v: 1`.
pub fn set_note_velocity(doc: &mut Document, k: f64, t: f64, v: Option<f64>) -> Res<()> {
    let i = note_at(doc, k, t)?;
    match v {
        None => delete(doc, &at(i, "v")),
        Some(v) => set(doc, &at(i, "v"), v),
    }
}

/// `setNoteEffect`: a pad's `repeat` or `skip` on, or off with None, which deletes the key.
pub fn set_note_effect(doc: &mut Document, k: f64, t: f64, effect: &str, n: Option<f64>) -> Res<()> {
    let i = note_at(doc, k, t)?;
    match n {
        Some(n) if n.fract() != 0.0 || n < 2.0 => fail(format!(
            "{effect} takes a whole number of 2 or more — {} says nothing a pad without it does not",
            js_number(n)
        )),
        None => delete(doc, &at(i, effect)),
        Some(n) => set(doc, &at(i, effect), n),
    }
}

/// One entry as plain values, or None for an entry that is not a note, which is left alone.
fn read_note(raw: &Value) -> Option<NoteIn> {
    if !matches!(raw, Value::Map(_)) {
        return None;
    }
    let (t, d, k) = (number(raw.get("t")), number(raw.get("d")), number(raw.get("k")));
    if !t.is_finite() || !d.is_finite() || !k.is_finite() {
        return None;
    }
    Some(NoteIn {
        t,
        d,
        k,
        v: num_field(raw, "v"),
        repeat: num_field(raw, "repeat"),
        skip: num_field(raw, "skip"),
    })
}

/// A rectangle of the roll: two keys inclusive and a half-open beat span.
pub struct NoteRegion {
    pub lo_key: f64,
    pub hi_key: f64,
    pub from: f64,
    pub to: f64,
}

/// `mapNotesInRegion`: every note STARTING inside rewritten in one pass, None removing it; how many it reached.
pub fn map_notes_in_region(
    doc: &mut Document,
    region: &NoteRegion,
    edit: &dyn Fn(&NoteIn) -> Option<NoteIn>,
) -> Res<usize> {
    let notes = notes_of(doc)?;
    let (mut drop, mut changed) = (Vec::new(), 0);
    for (i, raw) in notes.iter().enumerate() {
        let Some(n) = read_note(raw) else { continue };
        if n.k < region.lo_key || n.k > region.hi_key || n.t < region.from - EPS || n.t >= region.to - EPS {
            continue;
        }
        changed += 1;
        let Some(next) = edit(&n) else {
            drop.push(i);
            continue;
        };
        if !is_playable(&next) {
            return fail(format!(
                "k{} at beat {} would become a note nothing can play",
                js_number(n.k),
                js_number(n.t)
            ));
        }
        for (field, was, now) in [("k", n.k, next.k), ("t", n.t, next.t), ("d", n.d, next.d)] {
            if was != now {
                set(doc, &at(i, field), now)?;
            }
        }
        if next.v != n.v {
            match next.v {
                None => delete(doc, &at(i, "v"))?,
                Some(v) => set(doc, &at(i, "v"), v)?,
            }
        }
    }
    // Back to front, so an index read before the first deletion still names its own note.
    for i in drop.into_iter().rev() {
        delete(doc, &[key("notes"), PathSeg::Index(i)])?;
    }
    Ok(changed)
}

/// A sequence's window in beats as the compiler reads it: a stated `to` after `from`, else the notes rounded up to the bar.
fn window_of(data: &Value, notes: &[NoteIn], num: f64, den: f64) -> Res<(f64, f64)> {
    let bar = num * 4.0 / den;
    let extent = (notes.iter().fold(0.0_f64, |m, n| m.max(n.t + n.d)) / bar).ceil() * bar;
    let raw = |k: &str| -> Res<Option<f64>> {
        match data.get(k) {
            None | Some(Value::Null) => Ok(None),
            Some(v) => parse_position_on(&js_string(v), num, den)
                .map(Some)
                .or_else(|e| fail(e.to_string())),
        }
    };
    let from = raw("from")?.unwrap_or(0.0);
    let to = match raw("to")? {
        Some(s) if s > from => s,
        _ => extent.max(from),
    };
    Ok((from, to))
}

/// `doubleSequence`: the window's notes copied one window later, a stated `to` moved out.
pub fn double_sequence(doc: &mut Document, num: f64, den: f64) -> Res<()> {
    let notes: Vec<NoteIn> = notes_of(doc)?.iter().filter_map(read_note).collect();
    if notes.is_empty() {
        return fail("no notes to double — this sequence is empty");
    }
    let data = doc.to_js()?;
    let (from, to) = window_of(&data, &notes, num, den)?;
    let win = to - from;
    if win.is_nan() || win <= 0.0 {
        return fail("this sequence has no length to double");
    }
    let copies: Vec<NoteIn> = notes
        .iter()
        .filter(|n| n.t >= from - EPS && n.t < to - EPS)
        .map(|n| NoteIn {
            t: n.t + win,
            ..n.clone()
        })
        .collect();
    if copies.is_empty() {
        return fail("no notes inside the window — nothing to double");
    }
    if !matches!(data.get("to"), None | Some(Value::Null)) {
        let to = beats_to_position_on(from + 2.0 * win, num, den).or_else(|e| fail(e.to_string()))?;
        set_value(doc, &[key("to")], &Value::Str(to))?;
    }
    append_notes(doc, &copies)
}
