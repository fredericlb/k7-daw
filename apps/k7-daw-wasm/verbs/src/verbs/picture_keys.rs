// picture_keys.rs — `picture key`, `unkey`, `speed` and `silence`: one thing about a shot at a time, after pictureKeys.ts and pictureSpeed.ts.
// A reader would get wrong a key's time: it is a TIMELINE time like every `picture …` verb's, and the file stores it from the shot's start.

use super::picture_animate::{ANIMATED_PROPS, EASES, remove_key_at, set_key_at};
use super::picture_file::{set_silent_at, set_speed_at};
use super::picture_shared::{put, seconds, target};
use super::shared::Done;
use crate::host::Host;
use k7x::edit::js_number;
use song_edit::js_text::str_to_number;
use song_edit::start_time::js_round;

fn prop_of<'a>(h: &mut Host, word: Option<&'a str>) -> Option<&'a str> {
    if let Some(w) = word.filter(|w| ANIMATED_PROPS.contains(w)) {
        return Some(w);
    }
    h.refuse([format!(
        "'{}' is not something a shot animates — {}",
        word.unwrap_or(""),
        ANIMATED_PROPS.join(", ")
    )]);
    None
}

fn who(ref_: &str) -> &str {
    if ref_.is_empty() { "activation" } else { ref_ }
}

/// `picture key <lane> <at> <prop> <when> <value> [ease]` — one key, `when` on the timeline.
pub fn key(a: &str, h: &mut Host) -> Done {
    let Some(t) = target(h, a, "key", "<prop> <when> <value> [ease]")? else {
        return Ok(());
    };
    let arg = |i: usize| t.rest.get(i).map(String::as_str);
    let Some(prop) = prop_of(h, arg(0)) else {
        return Ok(());
    };
    let Some(when) = seconds(h, arg(1)) else {
        return Ok(());
    };
    let value = arg(2).map_or(f64::NAN, str_to_number);
    if arg(2).is_none() || !value.is_finite() {
        h.refuse([format!("'{}' is not a number — {prop} takes one", arg(2).unwrap_or(""))]);
        return Ok(());
    }
    if let Some(e) = arg(3).filter(|e| !EASES.contains(e)) {
        h.refuse([format!("'{e}' is not an ease — {}", EASES.join(", "))]);
        return Ok(());
    }
    let (index, start) = (t.index, t.shot().start);
    let ease = arg(3).unwrap_or("linear");
    if !put(h, &t.lane, "key", None, |sc| {
        set_key_at(sc, index, prop, when - start, value, ease)
    }) {
        return Ok(());
    }
    h.log([format!(
        "{}: {} {prop} = {} at {}s ({}s into the shot)",
        t.lane.rel,
        who(&t.shot().ref_),
        js_number(value),
        js_number(when),
        js_number(js_round((when - start) * 1000.0) / 1000.0)
    )]);
    h.reload();
    Ok(())
}

/// `picture unkey <lane> <at> <prop> <when>` — the key `picture key` set there.
pub fn unkey(a: &str, h: &mut Host) -> Done {
    let Some(t) = target(h, a, "unkey", "<prop> <when>")? else {
        return Ok(());
    };
    let arg = |i: usize| t.rest.get(i).map(String::as_str);
    let Some(prop) = prop_of(h, arg(0)) else {
        return Ok(());
    };
    let Some(when) = seconds(h, arg(1)) else {
        return Ok(());
    };
    let (index, start) = (t.index, t.shot().start);
    let none = format!(
        "{}: no {prop} key at {}s on the shot at {}s",
        t.lane.rel,
        js_number(when),
        js_number(t.at)
    );
    if !put(h, &t.lane, "unkey", Some(none), |sc| {
        remove_key_at(sc, index, prop, when - start)
    }) {
        return Ok(());
    }
    h.log([format!(
        "{}: {} − {prop} key at {}s",
        t.lane.rel,
        who(&t.shot().ref_),
        js_number(when)
    )]);
    h.reload();
    Ok(())
}

/// `picture speed <lane> <at> <factor>` — how fast a shot's source runs under it: 2 double, 0.5 slow, 0 a freeze, -1 backwards.
pub fn speed(a: &str, h: &mut Host) -> Done {
    let Some(t) = target(h, a, "speed", "<factor>")? else {
        return Ok(());
    };
    let word = t.rest.first().map(String::as_str);
    let pace = word.map_or(f64::NAN, str_to_number);
    if word.is_none() || !pace.is_finite() {
        h.refuse([format!(
            "'{}' is not a speed — 2 is double, 0.5 half, 0 a freeze, -1 backwards",
            word.unwrap_or("")
        )]);
        return Ok(());
    }
    let shot = t.shot().clone();
    if shot.ref_.is_empty() {
        h.refuse([format!(
            "the event at {}s is an ACTIVATION — it has no source to run",
            js_number(t.at)
        )]);
        return Ok(());
    }
    if pace < 0.0 && shot.length > 0.0 && shot.source_from + shot.length * pace < -5e-4 {
        let left: f64 = format!("{:.3}", shot.source_from / -pace).parse().unwrap_or(0.0);
        h.refuse([format!(
            "backwards at {}× from {}s, {} runs out of source after {}s — start it later in its source (picture head), or shorten it",
            js_number(-pace),
            js_number(shot.source_from),
            shot.ref_,
            js_number(left)
        )]);
        return Ok(());
    }
    let index = t.index;
    if !put(h, &t.lane, "speed", None, |sc| set_speed_at(sc, index, pace)) {
        return Ok(());
    }
    let said = if pace == 1.0 {
        "back to its own pace".to_owned()
    } else if pace == 0.0 {
        "frozen on its first frame".to_owned()
    } else {
        format!("at {}×", js_number(pace))
    };
    h.log([format!("{}: {} at {}s {said}", t.lane.rel, shot.ref_, js_number(t.at))]);
    h.reload();
    Ok(())
}

/// `picture silence <lane> <at>` — the shot's sound is not played, or is again; its picture is untouched.
pub fn silence(a: &str, h: &mut Host) -> Done {
    let Some(t) = target(h, a, "silence", "")? else {
        return Ok(());
    };
    let shot = t.shot().clone();
    if shot.ref_.is_empty() {
        h.refuse([format!(
            "the event at {}s is an ACTIVATION — it has no sound to silence",
            js_number(t.at)
        )]);
        return Ok(());
    }
    let on = !shot.silent;
    let index = t.index;
    if !put(h, &t.lane, "silence", None, |sc| set_silent_at(sc, index, on)) {
        return Ok(());
    }
    let said = if on {
        "silenced — its picture stays"
    } else {
        "heard again"
    };
    h.log([format!("{}: {} at {}s {said}", t.lane.rel, shot.ref_, js_number(t.at))]);
    h.reload();
    Ok(())
}
