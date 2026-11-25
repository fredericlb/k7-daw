// event_strip.rs — an event's own strip: fades, window, gain, loop and its points, the mute and warp flags; after verbs/eventStrip.ts.
// A reader would get wrong `on` in `event status`: it means the NAMED state is on, so `warp on` makes the event follow its clip's map.

use super::shared::{Done, Found, label, resolve_event, split_words};
use crate::host::Host;
use k7x::edit::js_number;
use song::clips::ClipKind;
use song_edit::event_gain::{EVENT_GAIN_MAX_DB, event_gain_value, set_track_placement_gain};
use song_edit::event_loop::{At, set_track_placement_loop, set_track_placement_loop_points};
use song_edit::js_text::str_to_number;
use song_edit::track_mix::{
    set_track_placement_disabled, set_track_placement_fade, set_track_placement_warp, set_track_placement_window,
};
use song_schema::{TimeSig, parse_duration};

/// `Number#toFixed`: a tie on the exact binary value rounds as Rust's formatter does, which JS rounds up.
fn to_fixed(x: f64, digits: usize) -> String {
    format!("{x:.digits$}")
}

/// `+x.toFixed(3)` printed back as a number.
fn fixed_number(x: f64) -> String {
    js_number(str_to_number(&to_fixed(x, 3)))
}

fn state_of(arg: Option<&str>) -> String {
    arg.unwrap_or("toggle").to_lowercase()
}

/// `event fadein|fadeout <track> <timecode> <length>` — a position then a duration; 0 removes the fade.
fn fade(a: &str, h: &mut Host, which: &str) -> Done {
    let verb = if which == "fadeIn" {
        "event fadein"
    } else {
        "event fadeout"
    };
    let w = split_words(a);
    if w.len() != 3 {
        h.refuse([
            format!("usage: {verb} <track> <timecode> <length>"),
            format!("       (e.g. {verb} gtr 2:1:1 0:1 — a length, not a position; 0 removes it)"),
        ]);
        return Ok(());
    }
    let (at_tc, len_tc) = (w[1], w[2]);
    let Some(Found {
        track,
        at_beats,
        hit,
        num,
        den,
        ..
    }) = resolve_event(h, w[0], at_tc, false, false)?
    else {
        return Ok(());
    };
    let beats = match TimeSig::new(num as u32, den as u32).and_then(|s| parse_duration(len_tc, s)) {
        Ok(b) => b,
        Err(e) => {
            h.refuse([format!("bad length: {e}")]);
            return Ok(());
        }
    };
    if beats < 0.0 {
        h.refuse([format!("a fade cannot be negative — got {len_tc}")]);
        return Ok(());
    }
    let dir = h.song_path.clone();
    match set_track_placement_fade(h.fs, &dir, &track.id, at_beats, num, den, which, beats) {
        Ok(false) => h.refuse([format!("no event on '{}' at {at_tc}", track.id)]),
        Ok(true) => {
            let what = if which == "fadeIn" { "fades in" } else { "fades out" };
            h.log([if beats > 0.0 {
                format!("{}: {} at {at_tc} {what} over {len_tc}", track.id, label(&hit))
            } else {
                format!("{}: {} at {at_tc} no longer {what}", track.id, label(&hit))
            }]);
            h.reload();
        }
        Err(e) => h.refuse([format!("{verb} failed: {e}")]),
    }
    Ok(())
}

pub fn fade_in(a: &str, h: &mut Host) -> Done {
    fade(a, h, "fadeIn")
}

pub fn fade_out(a: &str, h: &mut Host) -> Done {
    fade(a, h, "fadeOut")
}

/// `event window <track> <at> <from> <to>` — which slice one event plays, in seconds of its file.
pub fn window(a: &str, h: &mut Host) -> Done {
    let w = split_words(a);
    if w.len() != 4 {
        h.refuse([
            "usage: event window <track> <timecode> <from> <to>".to_owned(),
            "       (from/to in SECONDS of the file, e.g. event window vox 24:1 52.36 69.82)".to_owned(),
        ]);
        return Ok(());
    }
    let (at_tc, from_arg, to_arg) = (w[1], w[2], w[3]);
    let Some(Found {
        track,
        at_beats,
        hit,
        num,
        den,
        ..
    }) = resolve_event(h, w[0], at_tc, true, false)?
    else {
        return Ok(());
    };
    let (from, to) = (str_to_number(from_arg), str_to_number(to_arg));
    if !from.is_finite() || !to.is_finite() || from < 0.0 {
        h.refuse([format!(
            "'{from_arg} {to_arg}' is not a window in seconds — two numbers, the first not negative"
        )]);
        return Ok(());
    }
    if to <= from {
        h.refuse([
            format!("a window has to run forwards — got {from_arg} to {to_arg}"),
            "nothing was changed: a window of zero length would play nothing at all".to_owned(),
        ]);
        return Ok(());
    }
    let dur = if hit.source.is_empty() {
        0.0
    } else {
        h.seconds(&hit.source).unwrap_or(0.0)
    };
    let dir = h.song_path.clone();
    match set_track_placement_window(h.fs, &dir, &track.id, at_beats, num, den, (from, to), dur) {
        Ok(false) => h.refuse([format!("no event on '{}' at {at_tc}", track.id)]),
        Ok(true) => {
            h.log([format!(
                "{}: {} at {at_tc} plays {}s → {}s of its file",
                track.id,
                label(&hit),
                to_fixed(from, 3),
                to_fixed(to, 3)
            )]);
            h.reload();
        }
        Err(e) => h.refuse([format!("event window failed: {e}")]),
    }
    Ok(())
}

/// `event gain <track> <at> <dB>` — one audio event's own trim, −6 to +6.
pub fn gain(a: &str, h: &mut Host) -> Done {
    let w = split_words(a);
    if w.len() != 3 {
        h.refuse(["usage: event gain <track> <timecode> <dB>   (-6 … +6; 0 is unity)".to_owned()]);
        return Ok(());
    }
    let (at_tc, db_arg) = (w[1], w[2]);
    let db = str_to_number(db_arg);
    if !db.is_finite() || db.abs() > EVENT_GAIN_MAX_DB {
        h.refuse([format!(
            "'{db_arg}' is not a gain from -{EVENT_GAIN_MAX_DB} to +{EVENT_GAIN_MAX_DB} dB"
        )]);
        return Ok(());
    }
    let Some(Found {
        track,
        at_beats,
        hit,
        num,
        den,
        ..
    }) = resolve_event(h, w[0], at_tc, false, true)?
    else {
        return Ok(());
    };
    if hit.kind != ClipKind::Audio {
        h.refuse([
            format!("the event at {at_tc} is MIDI: it shares its lane's instrument, so it has no gain of its own"),
            "its notes' velocities are the way to make it louder or softer".to_owned(),
        ]);
        return Ok(());
    }
    let dir = h.song_path.clone();
    match set_track_placement_gain(h.fs, &dir, &track.id, at_beats, (num, den), db) {
        Ok(false) => h.refuse([format!("no event on '{}' at {at_tc}", track.id)]),
        Ok(true) => {
            let said = match event_gain_value(db) {
                None => "at unity".to_owned(),
                Some(v) => format!("trimmed {}{} dB", if v > 0.0 { "+" } else { "" }, js_number(v)),
            };
            h.log([format!("{}: {} at {at_tc} {said}", track.id, label(&hit))]);
            h.reload();
        }
        Err(e) => h.refuse([format!("event gain failed: {e}")]),
    }
    Ok(())
}

/// `event loop <track> <at> [on|off|toggle]` — whether one event repeats its window.
pub fn event_loop(a: &str, h: &mut Host) -> Done {
    let w = split_words(a);
    if !(2..=3).contains(&w.len()) {
        h.refuse(["usage: event loop <track> <timecode> [on|off|toggle]".to_owned()]);
        return Ok(());
    }
    let at_tc = w[1];
    let state = state_of(w.get(2).copied());
    if !["on", "off", "toggle"].contains(&state.as_str()) {
        h.refuse([format!("'{}' is not on, off or toggle", w[2])]);
        return Ok(());
    }
    let Some(Found {
        track,
        at_beats,
        hit,
        num,
        den,
        tempo,
    }) = resolve_event(h, w[0], at_tc, true, false)?
    else {
        return Ok(());
    };
    let looped = hit.event.as_ref().and_then(|e| e.loop_).unwrap_or(1) > 1;
    let on = if state == "toggle" { !looped } else { state == "on" };
    let secs = if hit.source.is_empty() {
        None
    } else {
        h.seconds(&hit.source)
    };
    let dir = h.song_path.clone();
    let at = At {
        dir: &dir,
        id: &track.id,
        from_beats: at_beats,
        num,
        den,
        tempo,
    };
    match set_track_placement_loop(h.fs, &at, on, secs) {
        Ok(None) => h.refuse([format!("no event on '{}' at {at_tc}", track.id)]),
        Ok(Some(done)) => {
            let plays = if done.loops > 1.0 {
                format!("loops, {} passes", js_number(done.loops))
            } else {
                "plays once".to_owned()
            };
            let already = if done.changed { "" } else { " (already)" };
            h.log([format!("{}: {} at {at_tc} {plays}{already}", track.id, label(&hit))]);
            if done.changed {
                h.reload();
            }
        }
        Err(e) => h.refuse([format!("event loop failed: {e}")]),
    }
    Ok(())
}

/// `event loop points <track> <at> <from> <to>` — the window an event loops, beats into a sequence, seconds into a file.
pub fn loop_points(a: &str, h: &mut Host) -> Done {
    let w = split_words(a);
    if w.len() != 4 {
        h.refuse([
            "usage: event loop points <track> <timecode> <from> <to>".to_owned(),
            "       (beats into a sequence, seconds into an audio file)".to_owned(),
        ]);
        return Ok(());
    }
    let (at_tc, from_arg, to_arg) = (w[1], w[2], w[3]);
    let (from, to) = (str_to_number(from_arg), str_to_number(to_arg));
    if !from.is_finite() || !to.is_finite() || from < 0.0 || to <= from {
        h.refuse([format!(
            "'{from_arg} {to_arg}' are not loop points — two numbers, running forwards, the first not negative"
        )]);
        return Ok(());
    }
    let Some(Found {
        track,
        at_beats,
        hit,
        num,
        den,
        tempo,
    }) = resolve_event(h, w[0], at_tc, true, false)?
    else {
        return Ok(());
    };
    let dir = h.song_path.clone();
    let at = At {
        dir: &dir,
        id: &track.id,
        from_beats: at_beats,
        num,
        den,
        tempo,
    };
    match set_track_placement_loop_points(h.fs, &at, from, to) {
        Ok(None) => h.refuse([format!("no event on '{}' at {at_tc}", track.id)]),
        Ok(Some(done)) => {
            let unit = if done.kind.name() == "sequence" { "b" } else { "s" };
            let passes = if done.loops > 1.0 {
                format!(", {} passes", js_number(done.loops))
            } else {
                String::new()
            };
            h.log([format!(
                "{}: {} at {at_tc} loops {}{unit} → {}{unit}{passes}",
                track.id,
                label(&hit),
                fixed_number(done.from),
                fixed_number(done.to)
            )]);
            h.reload();
        }
        Err(e) => h.refuse([format!("event loop points failed: {e}")]),
    }
    Ok(())
}

/// `event status mute|warp <track> <timecode> [on|off|toggle]`.
fn status(a: &str, h: &mut Host, aspect: &str) -> Done {
    let verb = format!("event status {aspect}");
    let w = split_words(a);
    if !(2..=3).contains(&w.len()) {
        h.refuse([format!("usage: {verb} <track> <timecode> [on|off|toggle]")]);
        return Ok(());
    }
    let at_tc = w[1];
    let mute = aspect == "mute";
    let Some(Found {
        track,
        at_beats,
        hit,
        num,
        den,
        ..
    }) = resolve_event(h, w[0], at_tc, false, mute)?
    else {
        return Ok(());
    };
    let state = state_of(w.get(2).copied());
    if !["on", "off", "toggle"].contains(&state.as_str()) {
        h.refuse([format!("'{}' is not on, off or toggle", w[2])]);
        return Ok(());
    }
    let now = if mute {
        hit.disabled == Some(true)
    } else {
        hit.warp_off != Some(true)
    };
    let next = if state == "toggle" { !now } else { state == "on" };
    let dir = h.song_path.clone();
    let done = if mute {
        set_track_placement_disabled(h.fs, &dir, &track.id, at_beats, num, den, next)
    } else {
        set_track_placement_warp(h.fs, &dir, &track.id, at_beats, num, den, next)
    };
    match done {
        Ok(false) => h.refuse([format!("no event on '{}' at {at_tc}", track.id)]),
        Ok(true) => {
            let said = match (mute, next) {
                (true, true) => "muted",
                (true, false) => "unmuted",
                (false, true) => "warped — follows the clip's map",
                (false, false) => "unwarped — plays at its own rate",
            };
            h.log([format!("{}: {} at {at_tc} {said}", track.id, label(&hit))]);
            h.reload();
        }
        Err(e) => h.refuse([format!("{verb} failed: {e}")]),
    }
    Ok(())
}

pub fn status_mute(a: &str, h: &mut Host) -> Done {
    status(a, h, "mute")
}

pub fn status_warp(a: &str, h: &mut Host) -> Done {
    status(a, h, "warp")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_digits_print_back_as_numbers() {
        assert_eq!(to_fixed(0.1, 3), "0.100");
        assert_eq!(fixed_number(0.1), "0.1");
        assert_eq!(fixed_number(2.0), "2");
    }
}
