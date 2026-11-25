// transport.rs — play, stop, toggle, rewind, panic, record-arm, the click and filing a recording; after verbs/transport.ts and verbs/project.ts.
// A reader would get wrong that the engine answers here: every engine move is an ask, so no line claims what only its answer could say.

use super::shared::{Done, join, split_words};
use crate::host::Host;
use k7x::edit::{PathSeg, Value, js_number};
use serde_json::{Map, json};
use song_edit::js_text::{js_trim, str_to_number};
use song_edit::start_time::js_round;
use song_edit::yaml_edit::set_file_value;

/// The click's lowest level, Tracktion's floor (`METRONOME_MIN` of engine-protocol).
const METRONOME_MIN: f64 = 0.2;

fn transport(h: &mut Host, action: &str) -> Done {
    h.ask("transport", json!([action]));
    Ok(())
}

pub fn play(_a: &str, h: &mut Host) -> Done {
    transport(h, "play")
}

pub fn stop(_a: &str, h: &mut Host) -> Done {
    transport(h, "stop")
}

pub fn toggle(_a: &str, h: &mut Host) -> Done {
    transport(h, "toggle")
}

pub fn rewind(_a: &str, h: &mut Host) -> Done {
    transport(h, "rewind")
}

pub fn panic_all(_a: &str, h: &mut Host) -> Done {
    transport(h, "panic")
}

/// `transport arm <track> [on|off]` — the engine's state, no file written; a bare arm toggles there.
pub fn arm(a: &str, h: &mut Host) -> Done {
    let w = split_words(a);
    let state = w.get(1).copied();
    if w.is_empty() || state.is_some_and(|s| s != "on" && s != "off") {
        h.refuse(["usage: transport arm <track> [on|off]".to_owned()]);
        return Ok(());
    }
    let id = w[0];
    let p = h.project()?;
    let Some(t) = p.tracks.iter().find(|t| t.id == id || t.name == id) else {
        let grouped = p.groups.iter().any(|g| g.id == id || g.name == id);
        h.refuse([if grouped {
            format!("\"{id}\" is a group — arm takes a track (a group carries no input)")
        } else {
            format!("no track \"{id}\"")
        }]);
        return Ok(());
    };
    if t.input_channels.is_empty() {
        h.refuse([format!("\"{}\" has no input — nothing to record", t.id)]);
        return Ok(());
    }
    if p.record_dir.is_empty() {
        h.refuse([format!(
            "\"{}\" is a single-file song — it has no clips/ to record into",
            p.name
        )]);
        return Ok(());
    }
    let track = t.id.clone();
    h.ask("arm", json!([track, state.map(|s| s == "on")]));
    // Armed or not is the engine's echo, which never comes back here: the line says what was asked.
    h.log([match state {
        Some("on") => format!("{track}: arm asked"),
        Some(_) => format!("{track}: disarm asked"),
        None => format!("{track}: arm toggle asked"),
    }]);
    Ok(())
}

fn settings_key(h: &mut Host, key: &str, value: Value) -> Done {
    let file = join(&h.song_path, "settings.k7x");
    set_file_value(h.fs, &file, &[PathSeg::Key(key.to_owned())], &value)?;
    Ok(())
}

/// `transport metronome` — the project's click flipped in settings.k7x; the caller pushes the new mix.
pub fn metronome(_a: &str, h: &mut Host) -> Done {
    let on = !h.project()?.settings.metronome;
    if let Err(e) = settings_key(h, "metronome", Value::Bool(on)) {
        h.refuse([format!("metronome failed: {}", e.0)]);
        return Ok(());
    }
    h.apply_project();
    h.log([format!("metronome {}", if on { "on" } else { "off" })]);
    Ok(())
}

/// `transport metronome volume <0.2..1>` — the click's level, kept apart from the switch, to two decimals.
pub fn metronome_volume(a: &str, h: &mut Host) -> Done {
    let arg = js_trim(a);
    let v = str_to_number(arg);
    if arg.is_empty() || !v.is_finite() {
        h.refuse(["usage: transport metronome volume <0.2..1>".to_owned()]);
        return Ok(());
    }
    if !(METRONOME_MIN..=1.0).contains(&v) {
        let hint = if v < METRONOME_MIN {
            "; to silence it use `transport metronome`, which is the switch"
        } else {
            ""
        };
        h.refuse([format!(
            "the click's level runs {} to 1 — {arg} is outside it{hint}",
            js_number(METRONOME_MIN)
        )]);
        return Ok(());
    }
    let level = js_round(v * 100.0) / 100.0;
    // The project must compile before its click is touched, as the TypeScript reads it first.
    h.project()?;
    if let Err(e) = settings_key(h, "metronome_level", Value::Num(level)) {
        h.refuse([format!("metronome volume failed: {}", e.0)]);
        return Ok(());
    }
    h.apply_project();
    h.log([format!("metronome level {}", js_number(level))]);
    Ok(())
}

/// `firstArg`: a JSON-quoted string or a bare word, and the trimmed rest; None for a quote that does not parse.
fn first_arg(a: &str) -> Option<(String, &str)> {
    if a.starts_with('"') {
        let bytes = a.as_bytes();
        let mut i = 1;
        while i < bytes.len() && bytes[i] != b'"' {
            i += if bytes[i] == b'\\' { 2 } else { 1 };
        }
        // An unclosed quote is no quoted string: the bare word below takes it, quote and all.
        if let Some(quoted) = a.get(..=i).filter(|q| q.len() > 1 && q.ends_with('"')) {
            let head: String = serde_json::from_str(quoted).ok()?;
            return Some((head, js_trim(&a[quoted.len()..])));
        }
    }
    let end = a.find(char::is_whitespace).unwrap_or(a.len());
    (end > 0).then(|| (a[..end].to_owned(), js_trim(&a[end..])))
}

/// The take and picture `project record save` names; `none` for no sound.
fn recorded_files(a: &str) -> Option<(Option<String>, Option<String>)> {
    let (take, rest) = first_arg(js_trim(a))?;
    let video = if rest.is_empty() {
        None
    } else {
        let (v, more) = first_arg(rest)?;
        if !more.is_empty() {
            return None;
        }
        Some(v)
    };
    Some(((take != "none").then_some(take), video))
}

/// `project record save <take|none> [video]` — the host files the output's recording, sound and picture.
pub fn record_save(a: &str, h: &mut Host) -> Done {
    let Some((file, video)) = recorded_files(a).filter(|(f, v)| f.is_some() || v.is_some()) else {
        h.refuse(["usage: project record save <take.wav|none> [<picture.webm>]".to_owned()]);
        return Ok(());
    };
    let mut take = Map::new();
    if let Some(f) = &file {
        take.insert("file".to_owned(), json!(f));
    }
    if let Some(v) = &video {
        take.insert("video".to_owned(), json!(v));
    }
    h.ask("saveTake", json!([take]));
    // Where it lands is the host's answer: the line names what was handed over.
    let named: Vec<&str> = file.iter().chain(video.iter()).map(String::as_str).collect();
    h.log([format!("saving {}", named.join(" and "))]);
    Ok(())
}
