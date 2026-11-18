// take_edit.rs — an event's takes on disk: a comp span picked, a take dropped or added, a MIDI take written as a sequence.
// A reader would get wrong how an event is named: by where it STARTS, as every event verb addresses one, and refused when none is there.

use crate::error::{Res, fail};
use crate::fs::Fs;
use crate::material_notes::Note;
use crate::paths::join;
use crate::song_tempo::song_tempo;
use crate::start_time::to_position;
use crate::take_file::{Span, add_take_at, remove_take_at, set_comp_at, set_crossfade_at};
use crate::track_edit::edit_track_struct;
use crate::track_file::find_placement_at;
use crate::workspace_doc::ws_write_data;
use crate::workspace_file::ws_exists;
use k7x::edit::{Scope, StringifyOptions, Value};

/// An event's address on a lane: the beat it starts on, and the project's meter.
#[derive(Clone, Copy)]
pub struct EventAt<'a> {
    pub dir: &'a str,
    pub track: &'a str,
    pub event_beats: f64,
    pub num: f64,
    pub den: f64,
}

/// One edit on the event of `track` starting at `event_beats`, refused when there is none there.
fn at_event(
    fs: &mut dyn Fs,
    e: &EventAt,
    what: &str,
    edit: impl FnOnce(&mut Scope, usize, f64) -> Res<()>,
) -> Res<String> {
    let tempo = song_tempo(fs, e.dir)?;
    edit_track_struct(fs, e.dir, e.track, what, |doc| {
        let Some(i) = find_placement_at(doc, e.event_beats, e.num, e.den, tempo)? else {
            let at = to_position(e.event_beats, e.num, e.den)?;
            return fail(format!("no event at {at} on {}", e.track));
        };
        edit(doc, i, tempo)
    })
}

/// Plays `span.take` over `span.from..span.to` (song positions); the event's own `ref` puts its material back.
///
/// # Errors
/// No event there, a take the event lacks, or a write's.
pub fn set_event_comp(fs: &mut dyn Fs, e: &EventAt, span: &Span) -> Res<String> {
    at_event(fs, e, "comp a take", |doc, i, tempo| {
        set_comp_at(doc, i, span, e.num, e.den, tempo)
    })
}

/// Sets the crossfade at the comp seam `seam` (a song position), `length` a duration; None removes it.
///
/// # Errors
/// No event there, or a write's.
pub fn set_event_crossfade(fs: &mut dyn Fs, e: &EventAt, seam: &str, length: Option<&str>) -> Res<String> {
    at_event(fs, e, "set a crossfade", |doc, i, _| {
        set_crossfade_at(doc, i, seam, length, e.num, e.den)
    })
}

/// Drops take `ref_` and every span it played; its material stays on disk.
///
/// # Errors
/// No event there, a take the event lacks, or a write's.
pub fn remove_event_take(fs: &mut dyn Fs, e: &EventAt, ref_: &str) -> Res<String> {
    at_event(fs, e, "remove a take", |doc, i, _| {
        remove_take_at(doc, i, ref_, e.num, e.den)
    })
}

/// Adds take `take` (ref, start) heard over `span` (None: kept but not heard).
///
/// # Errors
/// No event there, or what `set_comp_at` fails with.
pub fn add_event_take(fs: &mut dyn Fs, e: &EventAt, take: (&str, &str), span: Option<(&str, &str)>) -> Res<String> {
    at_event(fs, e, "add a take", |doc, i, tempo| {
        add_take_at(doc, i, take, span, e.num, e.den, tempo)
    })
}

/// A recorded MIDI pass as `sequences/<id>.k7x`, notes in beats from the take's own start, its window the whole pass.
///
/// # Errors
/// A sequence that already exists, or a write's.
pub fn write_take_sequence(
    fs: &mut dyn Fs,
    dir: &str,
    id: &str,
    notes: &[Note],
    length_beats: f64,
    meter: (f64, f64),
) -> Res<String> {
    let rel = format!("sequences/{id}.k7x");
    if ws_exists(fs, &join(&[dir, &rel])) {
        return fail(format!("sequence '{id}' already exists ({rel})"));
    }
    fs.mkdir_all(&join(&[dir, "sequences"]))
        .map_err(|e| crate::error::io_error(&e, "mkdir", dir))?;
    let to = to_position(length_beats.max(0.0), meter.0, meter.1)?;
    let rows = notes.iter().map(|n| {
        let mut row = vec![
            ("t".to_owned(), Value::Num(n.t)),
            ("d".to_owned(), Value::Num(n.d)),
            ("k".to_owned(), Value::Num(n.k)),
        ];
        row.extend(n.v.clone().map(|v| ("v".to_owned(), v)));
        Value::Map(row)
    });
    let data = Value::Map(vec![
        ("from".to_owned(), Value::Str("1:1".to_owned())),
        ("to".to_owned(), Value::Str(to)),
        ("notes".to_owned(), Value::List(rows.collect())),
    ]);
    ws_write_data(
        fs,
        &join(&[dir, &rel]),
        &data,
        &StringifyOptions {
            header: &[],
            inline: None,
        },
    )?;
    Ok(rel)
}
