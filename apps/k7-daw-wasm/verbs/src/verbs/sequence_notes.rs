// sequence_notes.rs — notes over a REGION (remove, transpose, quantize, velocity), a selection moved, a pad's effects,
// one note's velocity and a batch of recorded notes; after verbs/sequenceNotes.ts.
// A reader would get wrong an empty region: it is REFUSED by name, since silence would read as success.

use super::sequence::{edit_or_refuse, fmt_beat, note_arg, sequence_file};
use super::sequence_file::{
    NoteIn, NoteMove, NoteRegion, append_notes, is_playable, map_notes_in_region, move_notes, set_note_effect,
    set_note_velocity,
};
use super::shared::{Done, split_words};
use crate::catalogue::parse_midi_note;
use crate::host::Host;
use k7x::edit::js_number;
use serde_json::Value as Json;
use song_edit::js_text::{is_js_space, js_trim, str_to_number};
use song_edit::start_time::js_round;
use song_edit::workspace_doc::{EditOpts, ws_edit};

/// `sequence note repeat|skip <sequence> <note> <start> <n|off>` — a pad's two effects.
fn pad_effect(a: &str, h: &mut Host, effect: &str) -> Done {
    let w = split_words(a);
    if w.len() < 4 {
        h.refuse([format!(
            "usage: sequence note {effect} <sequence> <note> <start> <n|off>"
        )]);
        return Ok(());
    }
    let (seq, note, n_arg) = (w[0], w[1], w[3]);
    let Some(k) = note_arg(h, note) else { return Ok(()) };
    let t = str_to_number(w[2]);
    if !t.is_finite() {
        h.refuse(["start must be a number of beats".to_owned()]);
        return Ok(());
    }
    let n = (n_arg != "off").then(|| str_to_number(n_arg));
    if let Some(n) = n
        && (!n.is_finite() || n.fract() != 0.0 || n < 2.0)
    {
        h.refuse([format!(
            "{effect} takes a whole number of 2 or more, or \"off\" — not \"{n_arg}\""
        )]);
        return Ok(());
    }
    let Some(file) = sequence_file(h, seq) else {
        return Ok(());
    };
    let said = match n {
        None => format!("{effect} off"),
        Some(n) if effect == "repeat" => format!("repeated ×{}", js_number(n)),
        Some(n) => format!("silent every {} passes", js_number(n)),
    };
    let said = format!("{seq}: {note} (k{}) at {} → {said}", js_number(k), fmt_beat(t));
    edit_or_refuse(
        h,
        &file,
        &format!("sequence note {effect}"),
        |doc| set_note_effect(doc, k, t, effect, n),
        said,
    )
}

pub fn note_repeat(a: &str, h: &mut Host) -> Done {
    pad_effect(a, h, "repeat")
}

pub fn note_skip(a: &str, h: &mut Host) -> Done {
    pad_effect(a, h, "skip")
}

/// The trailing number a region verb takes, and its check.
struct Wants {
    name: &'static str,
    check: fn(f64) -> Option<&'static str>,
}

/// `sequence notes <verb> <sequence> <loNote> <hiNote> <from> <to> [value]` — one rectangle, one pass, one undo.
fn region(
    a: &str,
    h: &mut Host,
    verb: &str,
    value: Option<Wants>,
    edit: &dyn Fn(f64, &NoteIn) -> Option<NoteIn>,
) -> Done {
    let usage = format!(
        "usage: sequence notes {verb} <sequence> <loNote> <hiNote> <from> <to>{}",
        value.as_ref().map_or(String::new(), |v| format!(" <{}>", v.name))
    );
    let w = split_words(a);
    if w.len() < 5 || (value.is_some() && w.len() < 6) {
        h.refuse([usage]);
        return Ok(());
    }
    let (seq, lo_arg, hi_arg) = (w[0], w[1], w[2]);
    let (Some(lo), Some(hi)) = (parse_midi_note(lo_arg), parse_midi_note(hi_arg)) else {
        h.refuse([format!(
            "bad note in \"{lo_arg} {hi_arg}\" — use a name (C4, F#3) or a number 0..127"
        )]);
        return Ok(());
    };
    let (from, to) = (str_to_number(w[3]), str_to_number(w[4]));
    if !from.is_finite() || !to.is_finite() {
        h.refuse(["from/to must be numbers (beats)".to_owned()]);
        return Ok(());
    }
    let mut v = 0.0;
    if let Some(wants) = &value {
        v = str_to_number(w[5]);
        if !v.is_finite() {
            h.refuse([format!("{} must be a number", wants.name)]);
            return Ok(());
        }
        if let Some(bad) = (wants.check)(v) {
            h.refuse([bad.to_owned()]);
            return Ok(());
        }
    }
    let Some(file) = sequence_file(h, seq) else {
        return Ok(());
    };
    let r = NoteRegion {
        lo_key: lo.min(hi) as f64,
        hi_key: lo.max(hi) as f64,
        from,
        to,
    };
    let mut changed = 0;
    let done = ws_edit(h.fs, &file, EditOpts::default(), |doc| {
        changed = map_notes_in_region(doc, &r, &|n| edit(v, n))?;
        Ok(())
    });
    if let Err(e) = done {
        h.refuse([format!("sequence notes {verb}: {e}")]);
        return Ok(());
    }
    if changed == 0 {
        h.refuse([format!(
            "no note between {lo_arg} and {hi_arg} from beat {} to {}",
            js_number(from),
            js_number(to)
        )]);
        return Ok(());
    }
    h.log([format!("{seq}: {changed} note(s) {verb}d")]);
    h.reload();
    Ok(())
}

pub fn notes_remove(a: &str, h: &mut Host) -> Done {
    region(a, h, "remove", None, &|_, _| None)
}

/// Transposing CLAMPS onto the keyboard, as the engine's `transpose` does.
pub fn notes_transpose(a: &str, h: &mut Host) -> Done {
    let wants = Wants {
        name: "semitones",
        check: |v| (v.fract() != 0.0).then_some("semitones must be a whole number"),
    };
    region(a, h, "transpose", Some(wants), &|v, n| {
        Some(NoteIn {
            k: (n.k + v).clamp(0.0, 127.0),
            ..n.clone()
        })
    })
}

/// Quantizing moves the ONSET and keeps the length.
pub fn notes_quantize(a: &str, h: &mut Host) -> Done {
    let wants = Wants {
        name: "grid",
        check: |v| (v <= 0.0).then_some("the grid has to be more than 0 beats"),
    };
    region(a, h, "quantize", Some(wants), &|v, n| {
        Some(NoteIn {
            t: (js_round(n.t / v) * v).max(0.0),
            ..n.clone()
        })
    })
}

/// Absolute, and 1 CLEARS the field.
pub fn notes_velocity(a: &str, h: &mut Host) -> Done {
    let wants = Wants {
        name: "velocity",
        check: |v| (!(0.0..=1.0).contains(&v)).then_some("velocity is 0..1"),
    };
    region(a, h, "velocity", Some(wants), &|v, n| {
        Some(NoteIn {
            v: (v != 1.0).then(|| js_round(v * 100.0) / 100.0),
            ..n.clone()
        })
    })
}

/// The first word and the rest of a trimmed line, split at its first space.
fn head_rest(a: &str) -> Option<(&str, &str)> {
    let s = js_trim(a);
    let sp = s.find(is_js_space)?;
    Some((&s[..sp], &s[sp..]))
}

/// `sequence notes move <sequence> <[[note,start,newNote,newStart],…]>` — a dragged selection, one write.
pub fn notes_move(a: &str, h: &mut Host) -> Done {
    let usage = "usage: sequence notes move <sequence> <movesJSON [[note,start,newNote,newStart],…]>";
    let Some((seq, rest)) = head_rest(a) else {
        h.refuse([usage.to_owned()]);
        return Ok(());
    };
    let raw: Json = match serde_json::from_str(rest) {
        Ok(v) => v,
        Err(e) => {
            h.refuse([format!("bad moves JSON: {e}")]);
            return Ok(());
        }
    };
    let as_move = |m: &Json| -> Option<NoteMove> {
        let l = m.as_array().filter(|l| l.len() == 4)?;
        let n: Vec<f64> = l.iter().filter_map(Json::as_f64).collect();
        (n.len() == 4).then(|| NoteMove {
            k: n[0],
            t: n[1],
            nk: n[2],
            nt: n[3],
        })
    };
    let moves: Option<Vec<NoteMove>> = raw.as_array().map(|l| l.iter().map(as_move).collect()).unwrap_or(None);
    let Some(moves) = moves.filter(|m| !m.is_empty()) else {
        h.refuse([usage.to_owned()]);
        return Ok(());
    };
    let Some(file) = sequence_file(h, seq) else {
        return Ok(());
    };
    let said = format!("{seq}: moved {} note(s)", moves.len());
    edit_or_refuse(h, &file, "sequence notes move", |doc| move_notes(doc, &moves), said)
}

/// `sequence note velocity <sequence> <note> <start> <0..1>` — clamped, two decimals, 1 clearing the field.
pub fn note_velocity(a: &str, h: &mut Host) -> Done {
    let w = split_words(a);
    if w.len() < 4 {
        h.refuse(["usage: sequence note velocity <sequence> <note> <start> <velocity 0..1>".to_owned()]);
        return Ok(());
    }
    let (seq, note) = (w[0], w[1]);
    let Some(k) = note_arg(h, note) else { return Ok(()) };
    let (t, raw) = (str_to_number(w[2]), str_to_number(w[3]));
    if !t.is_finite() || !raw.is_finite() {
        h.refuse(["start (beats) and velocity (0..1) must be numbers".to_owned()]);
        return Ok(());
    }
    let v = js_round(raw.clamp(0.0, 1.0) * 100.0) / 100.0;
    let Some(file) = sequence_file(h, seq) else {
        return Ok(());
    };
    let said = format!(
        "{seq}: {note} (k{}) at {} → velocity {}",
        js_number(k),
        fmt_beat(t),
        js_number(v)
    );
    let stated = (v != 1.0).then_some(v);
    edit_or_refuse(
        h,
        &file,
        "sequence note velocity",
        |doc| set_note_velocity(doc, k, t, stated),
        said,
    )
}

/// A note out of JSON as `isPlayableNote` reads it: numbers only, never a string that looks like one.
pub fn json_note(n: &Json) -> Option<NoteIn> {
    let f = |k: &str| n.get(k).and_then(Json::as_f64);
    let note = NoteIn {
        t: f("t")?,
        d: f("d")?,
        k: f("k")?,
        v: f("v"),
        repeat: f("repeat"),
        skip: f("skip"),
    };
    is_playable(&note).then_some(note)
}

/// A velocity worth writing: 1 is what the engine applies anyway.
pub fn stated_velocity(v: Option<f64>) -> Option<f64> {
    v.filter(|v| (0.0..1.0).contains(v))
}

/// `sequence note add <sequence> <notesJSON>` — a batch of timed notes in one write; a pad's effects are checked.
pub fn record_notes(a: &str, h: &mut Host) -> Done {
    let Some((seq, rest)) = head_rest(a) else {
        h.refuse(["usage: sequence note add <sequence> <notesJSON>".to_owned()]);
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
    let effect = |x: Option<f64>| x.filter(|x| x.fract() == 0.0 && *x >= 2.0);
    let playable: Vec<NoteIn> = list
        .iter()
        .filter_map(json_note)
        .map(|n| NoteIn {
            t: n.t.max(0.0),
            v: stated_velocity(n.v),
            repeat: effect(n.repeat),
            skip: effect(n.skip),
            ..n
        })
        .collect();
    let Some(file) = sequence_file(h, seq) else {
        return Ok(());
    };
    if playable.is_empty() {
        h.refuse(["no valid notes to record".to_owned()]);
        return Ok(());
    }
    ws_edit(h.fs, &file, EditOpts::default(), |doc| append_notes(doc, &playable))?;
    h.log([format!("recorded {} note(s) → {seq}", playable.len())]);
    h.reload();
    Ok(())
}
