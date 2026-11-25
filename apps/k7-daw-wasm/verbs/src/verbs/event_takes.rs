// event_takes.rs — an event's takes: comp a span, drop a take, file a MIDI pass, arm the next takes into it; after verbs/eventTakes.ts.
// A reader would get wrong how the event is found: in the LANE FILE by its start, since a comped event's first piece may start elsewhere.
// Arming is the host's (`takeArm`), asked in the answer; a MIDI take is named with a fresh id where the TypeScript stamps the clock.

use super::shared::{Done, meter, split_words};
use crate::host::Host;
use k7x::edit::Value;
use serde_json::{Value as Json, json};
use song_edit::material_notes::Note;
use song_edit::take_edit::{
    EventAt, add_event_take, remove_event_take, set_event_comp, set_event_crossfade, write_take_sequence,
};
use song_edit::take_file::Span;
use song_schema::{TimeSig, beats_to_position_on, parse_duration, parse_position_on};

/// The lane id, the event's start in beats and the meter, or None with the refusal said.
struct Ev {
    track: String,
    at: f64,
    num: f64,
    den: f64,
}

fn event_of(h: &mut Host, track_arg: &str, at_tc: &str) -> Result<Option<Ev>, crate::host::Thrown> {
    let p = h.project()?;
    let Some(track) = p.tracks.iter().find(|t| t.id == track_arg || t.name == track_arg) else {
        h.refuse([format!("no track \"{track_arg}\"")]);
        return Ok(None);
    };
    let (num, den) = meter(&p);
    match parse_position_on(at_tc, num, den) {
        Ok(at) => Ok(Some(Ev {
            track: track.id.clone(),
            at,
            num,
            den,
        })),
        Err(e) => {
            h.refuse([format!("bad timecode: {e}")]);
            Ok(None)
        }
    }
}

impl Ev {
    fn at<'a>(&'a self, dir: &'a str) -> EventAt<'a> {
        EventAt {
            dir,
            track: &self.track,
            event_beats: self.at,
            num: self.num,
            den: self.den,
        }
    }
}

/// `event comp <track> <at> <take> <from> <to>` — the event's own material is a take like the others.
pub fn comp(a: &str, h: &mut Host) -> Done {
    let w = split_words(a);
    if w.len() != 5 {
        h.refuse([
            "usage: event comp <track> <at> <take> <from> <to>".to_owned(),
            "       (from/to are song positions, e.g. event comp gtr 5:1 gtr_5.1_261008T101500 6:1 7:3)".to_owned(),
        ]);
        return Ok(());
    }
    let (take, from, to) = (w[2], w[3], w[4]);
    let Some(ev) = event_of(h, w[0], w[1])? else {
        return Ok(());
    };
    let dir = h.song_path.clone();
    match set_event_comp(h.fs, &ev.at(&dir), &Span { take, from, to }) {
        Ok(_) => {
            h.log([format!("{}: {take} plays {from} → {to}", ev.track)]);
            h.reload();
        }
        Err(e) => h.refuse([format!("event comp failed: {e}")]),
    }
    Ok(())
}

/// `event take remove <track> <at> <take>` — its material stays on disk, as a deleted clip's does.
pub fn take_remove(a: &str, h: &mut Host) -> Done {
    let w = split_words(a);
    if w.len() != 3 {
        h.refuse(["usage: event take remove <track> <at> <take>".to_owned()]);
        return Ok(());
    }
    let take = w[2];
    let Some(ev) = event_of(h, w[0], w[1])? else {
        return Ok(());
    };
    let dir = h.song_path.clone();
    match remove_event_take(h.fs, &ev.at(&dir), take) {
        Ok(_) => {
            h.log([format!("{}: take {take} removed", ev.track)]);
            h.reload();
        }
        Err(e) => h.refuse([format!("event take remove failed: {e}")]),
    }
    Ok(())
}

fn num_of(v: Option<&Json>) -> f64 {
    v.and_then(Json::as_f64).unwrap_or(f64::NAN)
}

/// A pass's note, read as the browser sends it: `{t, d, k, v?}` in song beats.
fn note_of(n: &Json) -> Note {
    Note {
        t: num_of(n.get("t")),
        d: num_of(n.get("d")),
        k: num_of(n.get("k")),
        v: n.get("v").and_then(Json::as_f64).map(Value::Num),
    }
}

/// `event take midi <track> <at> <pass-json>` — the pass as its own sequence, added to the event and comped in.
pub fn take_midi(a: &str, h: &mut Host) -> Done {
    let w = split_words(a);
    if w.len() != 3 {
        h.refuse(["usage: event take midi <track> <at> <pass-json>".to_owned()]);
        return Ok(());
    }
    let Some(ev) = event_of(h, w[0], w[1])? else {
        return Ok(());
    };
    let Ok(pass) = serde_json::from_str::<Json>(w[2]) else {
        h.refuse(["event take midi: the pass is not JSON".to_owned()]);
        return Ok(());
    };
    let (start, end) = (num_of(pass.get("start")), num_of(pass.get("end")));
    let Some(raw) = pass.get("notes").and_then(Json::as_array).filter(|_| end > start) else {
        h.refuse(["event take midi: a pass needs a start before its end".to_owned()]);
        return Ok(());
    };
    let (num, den) = (ev.num, ev.den);
    let pos = |b: f64| beats_to_position_on(b, num, den).map_err(|e| e.to_string());
    let notes: Vec<Note> = raw
        .iter()
        .map(note_of)
        .filter(|n| n.d > 0.0 && n.t >= start)
        .map(|n| Note { t: n.t - start, ..n })
        .collect();
    let punch = pass.get("punch");
    let from = start.max(
        punch
            .and_then(|p| p.get("from"))
            .and_then(Json::as_f64)
            .unwrap_or(start),
    );
    let to = end.min(punch.and_then(|p| p.get("to")).and_then(Json::as_f64).unwrap_or(end));
    let dir = h.song_path.clone();
    let lane: String = ev
        .track
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "._-".contains(c) {
                c
            } else {
                '-'
            }
        })
        .collect();
    let written = pos(start).and_then(|start_tc| {
        let bar_beat = start_tc.split(':').take(2).collect::<Vec<_>>().join(".");
        let id = format!("{lane}_{bar_beat}_{}", (h.new_id)());
        let span = if to > from { Some((pos(from)?, pos(to)?)) } else { None };
        write_take_sequence(h.fs, &dir, &id, &notes, end - start, (num, den)).map_err(|e| e.to_string())?;
        let span_ref = span.as_ref().map(|(f, t)| (f.as_str(), t.as_str()));
        add_event_take(h.fs, &ev.at(&dir), (&id, &start_tc), span_ref).map_err(|e| e.to_string())?;
        Ok(id)
    });
    match written {
        Ok(id) => {
            h.log([format!("{}: take {id}, {} note(s)", ev.track, notes.len())]);
            h.reload();
        }
        Err(e) => h.refuse([format!("event take midi failed: {e}")]),
    }
    Ok(())
}

/// `event take arm <track> <at> <from> [to]` — the next takes go INTO that event; the host arms, asked in the answer.
pub fn take_arm(a: &str, h: &mut Host) -> Done {
    let w = split_words(a);
    if w.len() != 3 && w.len() != 4 {
        h.refuse([
            "usage: event take arm <track> <at> <from> [to]".to_owned(),
            "       (from/to are song positions: the punch, or from the playhead to the stop)".to_owned(),
        ]);
        return Ok(());
    }
    let (from_tc, to_tc) = (w[2], w.get(3).copied());
    let Some(ev) = event_of(h, w[0], w[1])? else {
        return Ok(());
    };
    let parsed = parse_position_on(from_tc, ev.num, ev.den).and_then(|f| {
        to_tc
            .map(|t| parse_position_on(t, ev.num, ev.den))
            .transpose()
            .map(|t| (f, t))
    });
    let (from, to) = match parsed {
        Ok(v) => v,
        Err(e) => {
            h.refuse([format!("bad timecode: {e}")]);
            return Ok(());
        }
    };
    if to.is_some_and(|t| t <= from) {
        h.refuse(["event take arm: the punch must end after it starts".to_owned()]);
        return Ok(());
    }
    let punch = match to {
        Some(t) => json!({ "from": from, "to": t }),
        None => json!({ "from": from }),
    };
    h.ask(
        "takeArm",
        json!([{ "track": ev.track, "eventBeats": ev.at, "punch": punch }]),
    );
    h.log([format!(
        "{}: takes go into the event at {}, punched {from_tc} → {}",
        ev.track,
        w[1],
        to_tc.unwrap_or("the stop")
    )]);
    Ok(())
}

/// `event take disarm` — recording goes back onto the lane.
pub fn take_disarm(_a: &str, h: &mut Host) -> Done {
    h.ask("takeArm", json!([null]));
    h.log(["takes disarmed: a take lands on its lane again".to_owned()]);
    Ok(())
}

/// `event comp fade <track> <at> <seam> <length>` — a crossfade at one seam of the comp; a length of 0 removes it.
pub fn comp_fade(a: &str, h: &mut Host) -> Done {
    let w = split_words(a);
    if w.len() != 4 {
        h.refuse([
            "usage: event comp fade <track> <at> <seam> <length>".to_owned(),
            "       (seam a song position, length a duration — 0 removes it)".to_owned(),
        ]);
        return Ok(());
    }
    let (seam, length_tc) = (w[2], w[3]);
    let Some(ev) = event_of(h, w[0], w[1])? else {
        return Ok(());
    };
    let beats = TimeSig::new(ev.num as u32, ev.den as u32)
        .and_then(|s| parse_duration(length_tc, s))
        .and_then(|b| parse_position_on(seam, ev.num, ev.den).map(|_| b));
    let beats = match beats {
        Ok(b) => b,
        Err(e) => {
            h.refuse([format!("bad timecode: {e}")]);
            return Ok(());
        }
    };
    let dir = h.song_path.clone();
    let length = (beats > 0.0).then_some(length_tc);
    match set_event_crossfade(h.fs, &ev.at(&dir), seam, length) {
        Ok(_) => {
            h.log([if beats > 0.0 {
                format!("{}: crossfade of {length_tc} at {seam}", ev.track)
            } else {
                format!("{}: no crossfade at {seam}", ev.track)
            }]);
            h.reload();
        }
        Err(e) => h.refuse([format!("event comp fade failed: {e}")]),
    }
    Ok(())
}
