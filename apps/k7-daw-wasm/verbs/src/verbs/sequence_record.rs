// sequence_record.rs — new sequences placed on lanes: `sequence empty` over a period, `sequence record` around a MIDI take.
// After verbs/sequenceEmpty.ts and sequenceRecord.ts; each is one command, so the lot is one undo.
// A reader would get wrong the units: `empty`'s `to` is EXCLUSIVE, and a take's notes arrive in SONG beats, stored clip-local.

use super::sequence_file::{NoteIn, append_notes, set_value};
use super::sequence_notes::{json_note, stated_velocity};
use super::shared::{Done, join, meter, position, split_words};
use crate::host::Host;
use k7x::edit::{PathSeg, Value, js_number};
use serde_json::Value as Json;
use song::doc::ProjectDoc;
use song_edit::chain_item_edit::{lane_events_at, lane_folder};
use song_edit::error::{Res, fail, io_error};
use song_edit::js_text::{is_js_space, js_trim, str_to_number};
use song_edit::track_mix::add_track_placement;
use song_edit::workspace_doc::{EditOpts, ws_edit};
use song_edit::workspace_file::{ws_exists, ws_remove};
use song_schema::beats_to_position_on;

/// The meter, 4/4 where the doc states none.
fn meter_or_common(p: &ProjectDoc) -> (f64, f64) {
    let (n, d) = meter(p);
    (if n == 0.0 { 4.0 } else { n }, if d == 0.0 { 4.0 } else { d })
}

fn tc(beats: f64, num: f64, den: f64) -> Res<String> {
    beats_to_position_on(beats, num, den).or_else(|e| fail(e.to_string()))
}

/// `<base>-<n>` for the first `n` whose file is free; a lane id a filename may not hold is `fallback`.
fn free_sequence(h: &Host, lane: &str, suffix: &str, fallback: &str) -> String {
    let ok = !lane.is_empty() && lane.bytes().all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b));
    let base = if ok {
        format!("{lane}-{suffix}")
    } else {
        fallback.to_owned()
    };
    let mut n = 1;
    while ws_exists(&*h.fs, &join(&h.song_path, &format!("sequences/{base}-{n}.k7x"))) {
        n += 1;
    }
    format!("{base}-{n}")
}

fn key(k: &str) -> Vec<PathSeg> {
    vec![PathSeg::Key(k.to_owned())]
}

fn make_dir(h: &mut Host) -> Res<()> {
    let dir = join(&h.song_path, "sequences");
    h.fs.mkdir_all(&dir).map_err(|e| io_error(&e, "mkdir", &dir))
}

/// An empty sequence `to - from` beats long on each lane, placed at `from`; a failing lane takes the new files back.
fn write_empty(h: &mut Host, lanes: &[&str], from: f64, to: f64, (num, den): (f64, f64)) -> Res<Vec<String>> {
    let mut missing = Vec::new();
    for l in lanes {
        if lane_events_at(&*h.fs, &h.song_path, l)?.is_none() {
            missing.push(format!("\"{l}\""));
        }
    }
    if !missing.is_empty() {
        return fail(format!(
            "no file in {}/ for {}",
            lane_folder(&*h.fs, &h.song_path),
            missing.join(", ")
        ));
    }
    let (at, end) = (tc(from, num, den)?, tc(to - from, num, den)?);
    make_dir(h)?;
    let (mut made, mut files) = (Vec::new(), Vec::new());
    let dir = h.song_path.clone();
    let mut run = || -> Res<()> {
        for lane in lanes {
            let id = free_sequence(h, lane, "seq", "seq");
            let file = join(&dir, &format!("sequences/{id}.k7x"));
            let opts = EditOpts {
                create: true,
                raw: false,
            };
            ws_edit(h.fs, &file, opts, |doc| {
                set_value(doc, &key("from"), &Value::Str("1:1".to_owned()))?;
                set_value(doc, &key("to"), &Value::Str(end.clone()))?;
                // Empty, not absent: a sequence without its `notes` list does not load.
                set_value(doc, &key("notes"), &Value::List(Vec::new()))
            })?;
            files.push(file);
            made.push(format!("+ sequence {id} on {lane} at {at}"));
            add_track_placement(h.fs, &dir, lane, &id, &at)?;
        }
        Ok(())
    };
    if let Err(e) = run() {
        for f in &files {
            let _ = ws_remove(h.fs, f);
        }
        return Err(e);
    }
    Ok(made)
}

/// `sequence empty <track>[,<track>…] <from> <to>` — every lane checked before anything is written.
pub fn sequence_empty(a: &str, h: &mut Host) -> Done {
    let w = split_words(a);
    if w.len() < 3 {
        h.refuse(["usage: sequence empty <track>[,<track>…] <from> <to>".to_owned()]);
        return Ok(());
    }
    let (list, from_tc, to_tc) = (w[0], w[1], w[2]);
    let p = h.project()?;
    let sig = meter_or_common(&p);
    let (from, to) = match position(from_tc, &p).and_then(|f| position(to_tc, &p).map(|t| (f, t))) {
        Ok(ft) => ft,
        Err(m) => {
            h.refuse([format!("sequence empty: {m}")]);
            return Ok(());
        }
    };
    if to.is_nan() || from.is_nan() || to <= from {
        h.refuse([format!("sequence empty: {to_tc} is not after {from_tc}")]);
        return Ok(());
    }
    let lanes: Vec<&str> = list.split(',').filter(|l| !l.is_empty()).collect();
    let unknown: Vec<String> = lanes
        .iter()
        .filter(|l| !p.tracks.iter().any(|t| t.id == **l || t.name == **l))
        .map(|l| format!("\"{l}\""))
        .collect();
    if !unknown.is_empty() {
        h.refuse([format!("sequence empty: no track {}", unknown.join(", "))]);
        return Ok(());
    }
    match write_empty(h, &lanes, from, to, sig) {
        Ok(made) => {
            h.log(made);
            h.reload();
        }
        Err(e) => h.refuse([format!("sequence empty: {e}")]),
    }
    Ok(())
}

/// `toFixed(places)` read back as a number.
fn fixed(v: f64, places: usize) -> f64 {
    str_to_number(&format!("{v:.places$}"))
}

/// The bar the first note falls in, and how many whole bars reach past the last one's end.
fn record_window(notes: &[NoteIn], num: f64, den: f64) -> (f64, f64) {
    let bar = num * 4.0 / den;
    let first = notes.iter().map(|n| n.t).fold(f64::INFINITY, f64::min).max(0.0);
    let from = (first / bar).floor() * bar;
    let end = notes.iter().map(|n| n.t + n.d).fold(f64::NEG_INFINITY, f64::max);
    (from, ((end - from) / bar).ceil().max(1.0))
}

/// The take as `sequences/<track>-rec-<n>.k7x`, placed; its id, where it landed and how many notes.
fn write_recorded(h: &mut Host, track: &str, raw: &[Json], (num, den): (f64, f64)) -> Res<(String, String, usize)> {
    let notes: Vec<NoteIn> = raw.iter().filter_map(json_note).collect();
    if notes.is_empty() {
        return fail("no valid notes to record");
    }
    let (from, bars) = record_window(&notes, num, den);
    let id = free_sequence(h, track, "rec", "rec");
    // Rounded as recorded sequences already are: a microsecond is below anything heard.
    let local: Vec<NoteIn> = notes
        .iter()
        .map(|x| {
            NoteIn::new(
                fixed((x.t - from).max(0.0), 6),
                fixed(x.d, 6),
                x.k,
                stated_velocity(x.v).map(|v| fixed(v, 4)),
            )
        })
        .collect();
    let file = join(&h.song_path, &format!("sequences/{id}.k7x"));
    make_dir(h)?;
    let opts = EditOpts {
        create: true,
        raw: false,
    };
    ws_edit(h.fs, &file, opts, |doc| {
        set_value(doc, &key("from"), &Value::Str("1:1".to_owned()))?;
        set_value(doc, &key("to"), &Value::Str(format!("{}:1", js_number(bars + 1.0))))?;
        append_notes(doc, &local)
    })?;
    let at = tc(from, num, den)?;
    let dir = h.song_path.clone();
    if let Err(e) = add_track_placement(h.fs, &dir, track, &id, &at) {
        // A sequence no lane places is a file nobody asked for.
        let _ = ws_remove(h.fs, &file);
        return Err(e);
    }
    Ok((id, at, local.len()))
}

/// `sequence record <track> <notesJSON>` — a MIDI take on a lane: one new sequence in whole bars, and its placement.
pub fn sequence_record(a: &str, h: &mut Host) -> Done {
    let s = js_trim(a);
    let Some((name, rest)) = s.find(is_js_space).map(|i| (&s[..i], js_trim(&s[i..]))) else {
        h.refuse(["usage: sequence record <track> <notesJSON>".to_owned()]);
        return Ok(());
    };
    let p = h.project()?;
    let Some(track) = p
        .tracks
        .iter()
        .find(|t| t.id == name || t.name == name)
        .map(|t| t.id.clone())
    else {
        h.refuse([format!("no track \"{name}\"")]);
        return Ok(());
    };
    let raw: Json = match serde_json::from_str(rest) {
        Ok(v) => v,
        Err(e) => {
            h.refuse([format!("bad notes JSON: {e}")]);
            return Ok(());
        }
    };
    let Some(list) = raw.as_array() else {
        h.refuse(["notes payload must be a JSON array of {t,d,k,v?}".to_owned()]);
        return Ok(());
    };
    match write_recorded(h, &track, list, meter_or_common(&p)) {
        Ok((id, at, count)) => {
            h.log([format!("recorded {count} note(s) → {id}, on {track} at {at}")]);
            h.reload();
        }
        Err(e) => h.refuse([format!("sequence record: {e}")]),
    }
    Ok(())
}
