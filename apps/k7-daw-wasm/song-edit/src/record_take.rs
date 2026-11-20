// record_take.rs — a finished take renamed, declared as a clip and placed on its lane or added to its armed event (3j O1).
// A reader would get wrong that the clock and the tempo are arguments: the server reads them from its compile and its own local time.

use crate::error::{Res, fail, io_error};
use crate::fs::Fs;
use crate::paths::{basename, dirname, extname, join};
use crate::take_edit::{EventAt, add_event_take};
use crate::track_edit::edit_track_struct;
use crate::track_file::add_placement;
use crate::workspace_doc::ws_write_data;
use crate::workspace_file::ws_exists;
use k7x::edit::{StringifyOptions, Value};
use song_schema::beats_to_position_on;

/// A finished take as the engine reports it; the position is in seconds, as the engine has no tempo map.
#[derive(Clone, Debug, PartialEq)]
pub struct RecordedTake {
    pub track: String,
    pub file: String,
    pub start: f64,
    pub length: f64,
}

/// The project's tempo and metre, read from its compile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Meter {
    pub tempo: f64,
    pub num: f64,
    pub den: f64,
}

/// Where a take recorded in the takes view goes: the event starting at `event_beats`, punched into `punch` when set.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TakeTarget {
    pub event_beats: f64,
    pub punch: Option<(f64, Option<f64>)>,
}

struct Declared {
    id: String,
    start: String,
    start_beats: f64,
    length_beats: f64,
}

fn position(beats: f64, m: Meter) -> Res<String> {
    beats_to_position_on(beats, m.num, m.den).map_err(|e| crate::error::Error::Msg(e.to_string()))
}

/// `[^A-Za-z0-9._-]` replaced by `-` per UTF-16 unit, as the TypeScript's global replace does.
fn track_token(track: &str) -> String {
    let mut out = String::new();
    for c in track.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
            out.push(c);
        } else {
            out.extend(std::iter::repeat_n('-', c.len_utf16()));
        }
    }
    out
}

/// The take renamed to `{track}_{bar.beat}_{stamp}` and declared as a clip, its window written out in full; where it lands.
fn declare_take(fs: &mut dyn Fs, dir: &str, m: Meter, take: &RecordedTake, stamp: &str) -> Res<Declared> {
    if !fs.exists(&take.file) {
        return fail(format!("recorded file is missing: {}", take.file));
    }
    let beats = |s: f64| s * m.tempo / 60.0;
    let start = position(beats(take.start), m)?;
    let to = position(beats(take.length), m)?;
    let from = position(0.0, m)?;
    let bar_beat = start.split(':').take(2).collect::<Vec<_>>().join(".");
    let base = format!("{}_{bar_beat}_{stamp}", track_token(&take.track));
    let ext = extname(&take.file);
    let folder = dirname(&take.file);
    let taken = |id: &str| {
        fs.exists(&join(&[&folder, &format!("{id}{ext}")]))
            || ws_exists(fs, &join(&[dir, "clips", &format!("{id}.k7x")]))
    };
    // A name another take holds is never replaced: the next free sibling is used, as the engine does.
    let mut id = base.clone();
    let mut n = 1;
    while join(&[&folder, &format!("{id}{ext}")]) != take.file && taken(&id) {
        n += 1;
        id = format!("{base}_{n}");
    }
    let wav = format!("{id}{ext}");
    let dest = join(&[&folder, &wav]);
    if dest != take.file {
        fs.rename(&take.file, &dest)
            .map_err(|e| io_error(&e, "rename", &take.file))?;
    }
    // The clip is declared before it is placed: a placement naming no clip fails the compile.
    let clip = join(&[dir, "clips", &format!("{id}.k7x")]);
    if !ws_exists(fs, &clip) {
        let data = Value::Map(vec![
            ("source".to_owned(), Value::Str(format!("clips/{}", basename(&wav)))),
            ("from".to_owned(), Value::Str(from)),
            ("to".to_owned(), Value::Str(to)),
        ]);
        ws_write_data(
            fs,
            &clip,
            &data,
            &StringifyOptions {
                header: &[],
                inline: None,
            },
        )?;
    }
    Ok(Declared {
        id,
        start,
        start_beats: beats(take.start),
        length_beats: beats(take.length),
    })
}

/// Declares a finished take and places it on its lane; the clip's id.
///
/// # Errors
/// A file that is not where it says, a lane with no file, or a write's.
pub fn add_take_to_track(fs: &mut dyn Fs, dir: &str, m: Meter, take: &RecordedTake, stamp: &str) -> Res<String> {
    let d = declare_take(fs, dir, m, take, stamp)?;
    let placement = Value::Map(vec![
        ("ref".to_owned(), Value::Str(d.id.clone())),
        ("start".to_owned(), Value::Str(d.start)),
    ]);
    // The engine mutes the armed track for the roll, so only the placement is written here.
    edit_track_struct(
        fs,
        dir,
        &take.track,
        &format!("add the recorded clip \"{}\"", d.id),
        |lane| add_placement(lane, &placement),
    )?;
    Ok(d.id)
}

/// Declares a finished take and adds it to the event it was recorded over, comped in where it was played.
///
/// # Errors
/// As `add_take_to_track`, or no event at the target.
pub fn add_take_to_event(
    fs: &mut dyn Fs,
    dir: &str,
    m: Meter,
    take: &RecordedTake,
    target: &TakeTarget,
    stamp: &str,
) -> Res<String> {
    let d = declare_take(fs, dir, m, take, stamp)?;
    // Heard from where it starts to where it stops, or only inside the punch: the rest is kept, unheard.
    let from = d.start_beats.max(target.punch.map_or(d.start_beats, |p| p.0));
    let to = (d.start_beats + d.length_beats).min(target.punch.and_then(|p| p.1).unwrap_or(f64::INFINITY));
    let span = if to > from {
        Some((position(from, m)?, position(to, m)?))
    } else {
        None
    };
    let at = EventAt {
        dir,
        track: &take.track,
        event_beats: target.event_beats,
        num: m.num,
        den: m.den,
    };
    add_event_take(
        fs,
        &at,
        (&d.id, &d.start),
        span.as_ref().map(|(a, b)| (a.as_str(), b.as_str())),
    )?;
    Ok(d.id)
}
