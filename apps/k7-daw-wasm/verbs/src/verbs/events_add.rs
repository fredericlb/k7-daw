// events_add.rs — `events add <json>`, a paste's receiving half: every item checked, then one write per lane; after verbs/event.ts.
// A reader would get wrong where PlacementSchema lives: in song's `files::Placement`, whose serde types this checks each field through.

use super::shared::{Done, join, makes_cycle, position, track_of};
use crate::host::Host;
use k7x::PathSeg;
use k7x::edit::Value;
use serde_json::{Map, Value as Json};
use song::clips::FIXED_FIELDS;
use song::files::Placement;
use song_edit::format_level::SECONDS_START_LEVEL;
use song_edit::format_stamp::stamp_format_level;
use song_edit::track_mix::add_track_placements;
use song_edit::workspace_file::ws_exists;

/// Everything a placement may say beyond WHERE it goes, listed rather than spread.
const EVENT_FIELDS: [&str; 7] = ["from", "to", "fadeIn", "fadeOut", "loop", "disabled", "warp"];

const USAGE: &str = "usage: events add <JSON [{track,ref,start,from?,to?,fadeIn?,fadeOut?,loop?,disabled?,warp?}]>  (e.g. events add [{\"track\":\"gtr\",\"ref\":\"riff\",\"start\":\"3:1\"}])";

/// A JSON value as k7x data.
fn k7x_of(j: &Json) -> Value {
    match j {
        Json::Null => Value::Null,
        Json::Bool(b) => Value::Bool(*b),
        Json::Number(n) => Value::Num(n.as_f64().unwrap_or(f64::NAN)),
        Json::String(s) => Value::Str(s.clone()),
        Json::Array(a) => Value::List(a.iter().map(k7x_of).collect()),
        Json::Object(o) => Value::object(o.iter().map(|(k, v)| (k.clone(), k7x_of(v))).collect()),
    }
}

/// The JSON type a refusal names, as zod names what it received.
fn received(j: &Json) -> &'static str {
    match j {
        Json::Null => "null",
        Json::Bool(_) => "boolean",
        Json::Number(_) => "number",
        Json::String(_) => "string",
        Json::Array(_) => "array",
        Json::Object(_) => "object",
    }
}

/// PlacementSchema over the fields an item carries: the first refusal as `<path> — <message>`, else the placement as written.
pub fn checked_placement(fields: &Map<String, Json>) -> Result<Map<String, Json>, String> {
    for (k, v) in fields {
        if v.is_null() {
            return Err(format!("{k} — expected a value, received null"));
        }
        let one: Map<String, Json> = std::iter::once((k.clone(), v.clone())).collect();
        if let Err(e) = serde_json::from_value::<Placement>(Json::Object(one)) {
            return Err(format!("{k} — {e} (received {})", received(v)));
        }
    }
    let whole =
        serde_json::from_value::<Placement>(Json::Object(fields.clone())).map_err(|e| format!("placement — {e}"))?;
    if let Some((path, message)) = whole.check() {
        let at: Vec<String> = path
            .iter()
            .map(|s| match s {
                PathSeg::Key(k) => k.clone(),
                PathSeg::Index(i) => i.to_string(),
            })
            .collect();
        let at = if at.is_empty() {
            "placement".to_owned()
        } else {
            at.join(".")
        };
        return Err(format!("{at} — {message}"));
    }
    // The schema widens a fade in bare seconds into its map form; what lands is its output.
    let mut out = fields.clone();
    for k in ["fadeIn", "fadeOut"] {
        if let Some(n @ Json::Number(_)) = out.get(k).cloned() {
            out.insert(k.to_owned(), serde_json::json!({ "over": n }));
        }
    }
    Ok(out)
}

/// Whether a start is a timecode, or seconds that are a number of at least 0.
fn start_ok(start: Option<&Json>) -> bool {
    match start {
        Some(Json::String(_)) => true,
        Some(Json::Number(n)) => n.as_f64().is_some_and(|s| s.is_finite() && s >= 0.0),
        _ => false,
    }
}

/// `events add <json>` — several placements, all checked before one rewrite per lane.
pub fn events_add(a: &str, h: &mut Host) -> Done {
    let payload = a.trim();
    if payload.is_empty() {
        h.refuse([USAGE.to_owned()]);
        return Ok(());
    }
    let raw: Json = match serde_json::from_str(payload) {
        Ok(v) => v,
        Err(e) => {
            h.refuse([format!("bad events JSON: {e}")]);
            return Ok(());
        }
    };
    let items = match raw {
        Json::Array(l) if !l.is_empty() => l,
        _ => {
            h.refuse(["events payload must be a non-empty JSON array of {track,ref,start}".to_owned()]);
            return Ok(());
        }
    };
    let p = h.project()?;
    let dir = h.song_path.clone();
    let mut by_track: Vec<(String, Vec<Value>)> = Vec::new();
    let mut seconds = false;
    let empty = Map::new();
    for (i, it) in items.iter().enumerate() {
        let w = format!("events add [{i}]");
        let it = it.as_object().unwrap_or(&empty);
        let (lane, r) = (
            it.get("track").and_then(Json::as_str),
            it.get("ref").and_then(Json::as_str),
        );
        let (Some(lane), Some(r), true) = (lane, r, start_ok(it.get("start"))) else {
            h.refuse([format!(
                "{w}: needs a string track and ref, and a start that is a timecode or seconds"
            )]);
            return Ok(());
        };
        let Some(track) = track_of(&p, lane) else {
            h.refuse([format!("{w}: no track \"{lane}\"")]);
            return Ok(());
        };
        if track.generated {
            h.refuse([format!(
                "{w}: '{}' is generated by an algo pattern and holds no placements",
                track.id
            )]);
            return Ok(());
        }
        let start = &it["start"];
        if let Some(tc) = start.as_str()
            && let Err(m) = position(tc, &p)
        {
            h.refuse([format!("{w}: bad timecode — {m}")]);
            return Ok(());
        }
        let exists = |folder: &str| ws_exists(&*h.fs, &join(&dir, &format!("{folder}/{r}.k7x")));
        let is_timeline = exists("timelines");
        if !is_timeline && !exists("clips") && !exists("sequences") {
            h.refuse([format!(
                "{w}: no clip, sequence or timeline \"{r}\" — a placement naming none stops the project loading"
            )]);
            return Ok(());
        }
        if is_timeline {
            let fixed: Vec<&str> = FIXED_FIELDS.iter().copied().filter(|k| it.contains_key(*k)).collect();
            if !fixed.is_empty() {
                h.refuse([format!(
                    "{w}: timeline '{r}' can't take {} — its length is fixed",
                    fixed.join(", ")
                )]);
                return Ok(());
            }
            if makes_cycle(&p, &track.id, r) {
                h.refuse([format!("{w}: timeline '{r}' would play inside itself")]);
                return Ok(());
            }
        }
        let mut fields = Map::new();
        fields.insert("ref".to_owned(), Json::String(r.to_owned()));
        fields.insert("start".to_owned(), start.clone());
        for k in EVENT_FIELDS {
            if let Some(v) = it.get(k) {
                fields.insert(k.to_owned(), v.clone());
            }
        }
        let placement = match checked_placement(&fields) {
            Ok(pl) => pl,
            Err(why) => {
                h.refuse([format!("{w}: {why}")]);
                return Ok(());
            }
        };
        // The two timecodes the schema checks only the SHAPE of: `1:0` matches the pattern and is no position.
        for k in ["fadeIn", "fadeOut"] {
            if let Some(tc) = placement.get(k).and_then(Json::as_str)
                && let Err(m) = position(tc, &p)
            {
                h.refuse([format!("{w}: bad {k} — {m}")]);
                return Ok(());
            }
        }
        seconds |= start.is_number();
        let value = k7x_of(&Json::Object(placement));
        match by_track.iter_mut().find(|(id, _)| *id == track.id) {
            Some((_, list)) => list.push(value),
            None => by_track.push((track.id.clone(), vec![value])),
        }
    }
    for (id, list) in &by_track {
        if let Err(e) = add_track_placements(h.fs, &dir, id, list) {
            h.refuse([format!("events add failed: {e}")]);
            return Ok(());
        }
    }
    // An event placed in seconds is a format a level-1 build refuses.
    if seconds {
        stamp_format_level(h.fs, &dir, SECONDS_START_LEVEL);
    }
    let n = items.len();
    let lanes: Vec<&str> = by_track.iter().map(|(id, _)| id.as_str()).collect();
    h.log([format!(
        "added {n} event{} on {}",
        if n == 1 { "" } else { "s" },
        lanes.join(", ")
    )]);
    h.reload();
    Ok(())
}
