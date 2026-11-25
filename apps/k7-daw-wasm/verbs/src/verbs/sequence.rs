// sequence.rs — MIDI sequences and their single notes: new, list, add, time range, move, split, remove, double; after verbs/sequence.ts.
// A reader would get wrong a note's address: its KEY and its START in beats, which is how every one of these finds it.

use super::sequence_file::{
    NoteIn, append_notes, double_sequence, move_note, remove_note, set_note_time_range, split_note,
};
use super::shared::{Done, join, meter, split_words, unique_id};
use crate::catalogue::parse_midi_note;
use crate::host::Host;
use k7x::edit::{Document, StringifyOptions, Value, js_number};
use song::clips::ClipKind;
use song_edit::Res;
use song_edit::js_text::{js_string, str_to_number};
use song_edit::sequence_loops::halve_loops_of;
use song_edit::track_file::KeptWhy;
use song_edit::workspace_doc::{EditOpts, ws_edit, ws_write_data};
use song_edit::workspace_file::ws_exists;
use song_schema::round_beat;

/// `fmtBeat`: a beat at six decimals at most, as the roll writes it.
pub fn fmt_beat(v: f64) -> String {
    js_number(round_beat(v))
}

/// A note argument as a MIDI key, or None with the refusal said.
pub fn note_arg(h: &mut Host, s: &str) -> Option<f64> {
    let k = parse_midi_note(s);
    if k.is_none() {
        h.refuse([format!("bad note \"{s}\" — use a name (C4, F#3) or a number 0..127")]);
    }
    k.map(|k| k as f64)
}

/// The file `sequences/<id>.k7x`, or None with the refusal said.
pub fn sequence_file(h: &mut Host, id: &str) -> Option<String> {
    let file = join(&h.song_path, &format!("sequences/{id}.k7x"));
    if ws_exists(&*h.fs, &file) {
        Some(file)
    } else {
        h.refuse([format!("no sequence file: sequences/{id}.k7x")]);
        None
    }
}

/// Runs a note edit over the sequence file; a failure is refused under `verb`, a success said by `said`.
pub fn edit_or_refuse(
    h: &mut Host,
    file: &str,
    verb: &str,
    edit: impl FnOnce(&mut Document) -> Res<()>,
    said: String,
) -> Done {
    match ws_edit(h.fs, file, EditOpts::default(), edit) {
        Ok(_) => {
            h.log([said]);
            h.reload();
        }
        Err(e) => h.refuse([format!("{verb}: {e}")]),
    }
    Ok(())
}

fn numbers<const N: usize>(words: [&str; N]) -> Option<[f64; N]> {
    let n = words.map(str_to_number);
    n.iter().all(|x| x.is_finite()).then_some(n)
}

/// `createSequenceFile`: four empty bars, its name checked.
fn create_sequence_file(h: &mut Host, id: &str) -> Res<String> {
    if id.is_empty() || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b)) {
        return song_edit::error::fail(format!(
            "invalid sequence name '{id}' — letters, digits, '.', '_' and '-' only"
        ));
    }
    let rel = format!("sequences/{id}.k7x");
    if ws_exists(&*h.fs, &join(&h.song_path, &rel)) {
        return song_edit::error::fail(format!("sequence '{id}' already exists ({rel})"));
    }
    let dir = join(&h.song_path, "sequences");
    h.fs.mkdir_all(&dir)
        .map_err(|e| song_edit::error::io_error(&e, "mkdir", &dir))?;
    let data = Value::Map(vec![
        ("from".to_owned(), Value::Str("1:1".to_owned())),
        ("to".to_owned(), Value::Str("5:1".to_owned())),
        ("notes".to_owned(), Value::List(Vec::new())),
    ]);
    ws_write_data(h.fs, &join(&h.song_path, &rel), &data, &StringifyOptions::default())?;
    Ok(rel)
}

/// `sequence new [name]` — four empty bars to write into, placed later with `event add`.
pub fn sequence_new(a: &str, h: &mut Host) -> Done {
    let name = split_words(a).first().copied().unwrap_or("");
    let id = if name.is_empty() {
        unique_id("seq", &h.project()?)
    } else {
        name.to_owned()
    };
    match create_sequence_file(h, &id) {
        Ok(rel) => {
            h.reload();
            h.log([format!("+ sequence {id} ({rel})")]);
        }
        Err(e) => h.refuse([format!("sequence new: {e}")]),
    }
    Ok(())
}

/// `sequence list` — every MIDI event of the song's lanes, with its notes and length.
pub fn sequence_list(_a: &str, h: &mut Host) -> Done {
    let p = h.project()?;
    let lines: Vec<String> = p
        .tracks
        .iter()
        .flat_map(|t| t.clips.iter().map(move |c| (t, c)))
        .filter(|(_, c)| c.kind == ClipKind::Midi)
        .map(|(t, c)| {
            let name = if !c.name.is_empty() {
                c.name.clone()
            } else if !c.id.is_empty() {
                c.id.clone()
            } else {
                format!("{}:seq", t.id)
            };
            format!(
                "  {name}  ({}, {} notes, {} beats)",
                t.id,
                c.notes.len(),
                js_number(c.length)
            )
        })
        .collect();
    if lines.is_empty() {
        h.log(["no sequences — add one with: sequence new".to_owned()]);
    } else {
        h.log(lines);
    }
    Ok(())
}

/// `sequence note new <sequence> <note> <start> [end]` — one note; an onset clamps to 0, a bad end makes one beat.
pub fn add_note(a: &str, h: &mut Host) -> Done {
    let w = split_words(a);
    if w.len() < 3 {
        h.refuse(["usage: sequence note new <sequence> <note> <start> [end]".to_owned()]);
        return Ok(());
    }
    let (seq, note) = (w[0], w[1]);
    let Some(k) = note_arg(h, note) else { return Ok(()) };
    let start = str_to_number(w[2]);
    // `Math.max(0, NaN)` is NaN, which the check below refuses.
    let t = if start.is_nan() { start } else { start.max(0.0) };
    let end = w.get(3).map_or(t + 1.0, |e| str_to_number(e));
    if !t.is_finite() || !end.is_finite() {
        h.refuse(["start/end must be numbers (beats)".to_owned()]);
        return Ok(());
    }
    let d = if end > t { end - t } else { 1.0 };
    let Some(file) = sequence_file(h, seq) else {
        return Ok(());
    };
    ws_edit(h.fs, &file, EditOpts::default(), |doc| {
        append_notes(doc, &[NoteIn::new(t, d, k, None)])
    })?;
    h.log([format!(
        "added {note} (k{}) {}..{} → {seq}",
        js_number(k),
        fmt_beat(t),
        fmt_beat(end)
    )]);
    h.reload();
    Ok(())
}

/// `sequence note timerange <sequence> <note> <start> <newStart> <newEnd>` — move a note, stretch it, or both.
pub fn note_time_range(a: &str, h: &mut Host) -> Done {
    let w = split_words(a);
    if w.len() < 5 {
        h.refuse(["usage: sequence note timerange <sequence> <note> <start> <newStart> <newEnd>".to_owned()]);
        return Ok(());
    }
    let (seq, note) = (w[0], w[1]);
    let Some(k) = note_arg(h, note) else { return Ok(()) };
    let Some([t, nt, ne]) = numbers([w[2], w[3], w[4]]) else {
        h.refuse(["start, newStart and newEnd must be numbers (beats)".to_owned()]);
        return Ok(());
    };
    if ne <= nt {
        h.refuse([format!(
            "a note has to end after it starts — {} → {} is nothing",
            js_number(nt),
            js_number(ne)
        )]);
        return Ok(());
    }
    let Some(file) = sequence_file(h, seq) else {
        return Ok(());
    };
    let said = format!(
        "{seq}: {note} (k{}) {} → {}, length {}",
        js_number(k),
        fmt_beat(t),
        fmt_beat(nt),
        fmt_beat(ne - nt)
    );
    edit_or_refuse(
        h,
        &file,
        "sequence note timerange",
        |doc| set_note_time_range(doc, k, t, nt, ne - nt),
        said,
    )
}

/// `sequence note move <sequence> <note> <start> <newNote> <newStart>` — length and velocity ride along.
pub fn move_one(a: &str, h: &mut Host) -> Done {
    let w = split_words(a);
    if w.len() < 5 {
        h.refuse(["usage: sequence note move <sequence> <note> <start> <newNote> <newStart>".to_owned()]);
        return Ok(());
    }
    let (seq, note, to) = (w[0], w[1], w[3]);
    let (k, nk) = (parse_midi_note(note), parse_midi_note(to));
    let (Some(k), Some(nk)) = (k, nk) else {
        let bad = if k.is_none() { note } else { to };
        h.refuse([format!("bad note \"{bad}\" — use a name (C4, F#3) or a number 0..127")]);
        return Ok(());
    };
    let (k, nk) = (k as f64, nk as f64);
    let Some([t, nt]) = numbers([w[2], w[4]]) else {
        h.refuse(["start and newStart (beats) must be numbers".to_owned()]);
        return Ok(());
    };
    let Some(file) = sequence_file(h, seq) else {
        return Ok(());
    };
    let said = format!(
        "{seq}: {note} (k{}) {} → {to} (k{}) {}",
        js_number(k),
        fmt_beat(t),
        js_number(nk),
        fmt_beat(nt)
    );
    edit_or_refuse(h, &file, "sequence note move", |doc| move_note(doc, k, t, nk, nt), said)
}

/// `sequence note split <sequence> <note> <start> <at>` — one note cut in two.
pub fn split_one(a: &str, h: &mut Host) -> Done {
    let w = split_words(a);
    if w.len() < 4 {
        h.refuse(["usage: sequence note split <sequence> <note> <start> <at>".to_owned()]);
        return Ok(());
    }
    let (seq, note) = (w[0], w[1]);
    let Some(k) = note_arg(h, note) else { return Ok(()) };
    let Some([t, cut]) = numbers([w[2], w[3]]) else {
        h.refuse(["start and at must be numbers (beats)".to_owned()]);
        return Ok(());
    };
    let Some(file) = sequence_file(h, seq) else {
        return Ok(());
    };
    let said = format!(
        "{seq}: {note} (k{}) split at {} — two notes now",
        js_number(k),
        fmt_beat(cut)
    );
    edit_or_refuse(h, &file, "sequence note split", |doc| split_note(doc, k, t, cut), said)
}

/// `sequence note remove <sequence> <note> <start>`.
pub fn remove_one(a: &str, h: &mut Host) -> Done {
    let w = split_words(a);
    if w.len() < 3 {
        h.refuse(["usage: sequence note remove <sequence> <note> <start>".to_owned()]);
        return Ok(());
    }
    let (seq, note) = (w[0], w[1]);
    let Some(k) = note_arg(h, note) else { return Ok(()) };
    let Some([t]) = numbers([w[2]]) else {
        h.refuse(["start (beats) must be a number".to_owned()]);
        return Ok(());
    };
    let Some(file) = sequence_file(h, seq) else {
        return Ok(());
    };
    let said = format!("{seq}: {note} (k{}) at {} removed", js_number(k), fmt_beat(t));
    edit_or_refuse(h, &file, "sequence note remove", |doc| remove_note(doc, k, t), said)
}

/// `sequence double <sequence>` — twice as long, playing twice, and every looped placement half as many passes.
pub fn sequence_double(a: &str, h: &mut Host) -> Done {
    let Some(seq) = split_words(a).first().copied() else {
        h.refuse(["usage: sequence double <sequence>".to_owned()]);
        return Ok(());
    };
    let Some(file) = sequence_file(h, seq) else {
        return Ok(());
    };
    // 4/4 when the doc states none: the bar is what a window rounds to.
    let (n, d) = meter(&h.project()?);
    let (num, den) = (if n == 0.0 { 4.0 } else { n }, if d == 0.0 { 4.0 } else { d });
    if let Err(e) = ws_edit(h.fs, &file, EditOpts::default(), |doc| double_sequence(doc, num, den)) {
        h.refuse([format!("sequence double: {e}")]);
        return Ok(());
    }
    let dir = h.song_path.clone();
    let loops = halve_loops_of(h.fs, &dir, seq, None)?;
    let mut lines = vec![format!("{seq}: doubled — twice as long, its notes played twice")];
    if loops.halved > 0 {
        lines.push(format!("{} placement(s) now loop half as many times", loops.halved));
    }
    lines.extend(loops.kept.iter().map(|(k, lane)| {
        let why = match k.why {
            KeptWhy::Odd => "an odd number of passes cannot be halved",
            KeptWhy::Windowed => "it states a window of its own, which did not grow",
        };
        format!(
            "{lane} at {} still loops {}× and is now twice as long — {why}",
            js_string(&k.start),
            js_number(k.loop_)
        )
    }));
    h.log(lines);
    h.reload();
    Ok(())
}
