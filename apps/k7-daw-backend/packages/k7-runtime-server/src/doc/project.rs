//! project.rs — a whole document off the wire: its settings bounded, its ids made unique, its tracks and master
//! chain read, and the slot's footswitch carried along. The result is the JSON `apply` hands the C++ reader.
//! What a reader gets wrong: the warnings of the parse belong to no session (it runs on a socket's task), so none is kept as a standing warning; only the engine's own build keeps those.

use super::Checks;
use super::bounds::{
    DEFAULT_SAMPLE_RATE, DEFAULT_TEMPO, SAMPLE_RATES, TIME_SIG_NUMERATORS, denominator_ok,
};
use super::plugin::plugin;
use super::read::{Obj, Read, missing_key};
use super::track::track;
use serde_json::{Map, Value, json};
use std::collections::HashSet;

/// A document read and checked, and what the log line and the handler need of it.
pub struct Parsed {
    /// The validated document, as the C++ reader reads it.
    pub doc: Value,
    pub name: String,
    pub tempo: f64,
    pub tracks: usize,
    pub clips: usize,
    pub plugins: usize,
}

/// The session a slot plays in: the supervisor's frozen chain, the one document that carries a footswitch.
pub const SLOT_SESSION: &str = "slot";

/// Ids that repeat get `#2`, `#3`… in document order, and a warning: every map downstream is keyed by id.
fn disambiguate(items: &mut [Value], seen: &mut HashSet<String>, what: &str, checks: &Checks) {
    for item in items {
        let Some(key) = item.get("id").and_then(Value::as_str).map(str::to_owned) else {
            continue;
        };
        if key.is_empty() || seen.insert(key.clone()) {
            continue;
        }
        let mut n = 2;
        while seen.contains(&format!("{key}#{n}")) {
            n += 1;
        }
        let renamed = format!("{key}#{n}");
        checks.warn(
            &format!("two {what} share the id '{key}' — the second is '{renamed}'"),
            "",
        );
        seen.insert(renamed.clone());
        item["id"] = Value::String(renamed);
    }
}

fn list_mut<'a>(v: &'a mut Value, key: &str) -> &'a mut Vec<Value> {
    match &mut v[key] {
        Value::Array(items) => items,
        _ => unreachable!("the parse wrote {key} as an array"),
    }
}

fn disambiguate_ids(tracks: &mut [Value], master: &mut [Value], checks: &Checks) {
    let (mut track_ids, mut plugin_ids) = (HashSet::new(), HashSet::new());
    disambiguate(tracks, &mut track_ids, "tracks", checks);
    // Plugins are ONE map per session, the master chain's included.
    disambiguate(master, &mut plugin_ids, "modules", checks);
    for t in tracks {
        disambiguate(list_mut(t, "plugins"), &mut plugin_ids, "modules", checks);
        disambiguate(list_mut(t, "clips"), &mut HashSet::new(), "clips", checks);
    }
}

/// The slot's footswitch, `{cc, plugins}`, when the document names a valid one and this is the slot.
fn footswitch(doc: &Obj, session: &str) -> Read<Option<Value>> {
    if session != SLOT_SESSION {
        return Ok(None);
    }
    let Some(Value::Object(fs)) = doc.field("footswitch")? else {
        return Ok(None);
    };
    let Some(cc) = fs.get("cc").filter(|v| v.is_i64() || v.is_u64()) else {
        return Ok(None);
    };
    let plugins: Vec<&str> = fs
        .get("plugins")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    Ok(Some(
        json!({"cc": super::read::as_int(cc)?, "plugins": plugins}),
    ))
}

/// `msg`'s `doc`, read and checked for `session`.
pub fn parse_project(msg: &Value, session: &str, checks: &Checks) -> Read<Parsed> {
    let j = Obj(Obj(msg).at("doc")?);
    let name = j.str("name", "untitled")?;
    let mut tempo = j.f64("tempo", DEFAULT_TEMPO)?;
    let mut rate = j.int("sampleRate", DEFAULT_SAMPLE_RATE)?;
    let (mut num, mut den) = (
        j.int("timeSigNumerator", 4)?,
        j.int("timeSigDenominator", 4)?,
    );
    if !(denominator_ok(den) && TIME_SIG_NUMERATORS.contains(&num)) {
        checks.warn(
            &format!("time signature {num}/{den} cannot be played — using 4/4"),
            "",
        );
        (num, den) = (4, 4);
    }
    if !tempo.is_finite() || tempo <= 0.0 {
        tempo = DEFAULT_TEMPO;
    }
    if !SAMPLE_RATES.contains(&rate) {
        rate = DEFAULT_SAMPLE_RATE;
    }
    let master_volume = j.f32("masterVolume", 0.0)?;
    let mut master = Vec::new();
    for p in j.array_if("masterPlugins") {
        master.push(plugin(p, checks)?);
    }
    let record_dir = checks.record_dir(j.str("recordDir", "")?);
    let record_mute = j.bool("recordMute", true)?;
    let (mut metronome, mut level, mut count_in) = (false, 0.6, 0);
    if let Some(Value::Object(_)) = j.field("settings")? {
        let s = Obj(j.at("settings")?);
        (metronome, level, count_in) = (
            s.bool("metronome", false)?,
            s.f64("metronomeLevel", 0.6)?,
            s.int("countIn", 0)?,
        );
    }
    let mut tracks = Vec::new();
    for t in j.list("tracks")? {
        tracks.push(track(t, checks)?);
    }
    disambiguate_ids(&mut tracks, &mut master, checks);
    let count = |key: &str| -> usize {
        tracks
            .iter()
            .map(|t| t[key].as_array().map_or(0, Vec::len))
            .sum()
    };
    let (clips, plugins) = (count("clips"), count("plugins"));
    let mut doc = Map::new();
    doc.insert("name".into(), name.clone().into());
    doc.insert("tempo".into(), tempo.into());
    doc.insert("sampleRate".into(), rate.into());
    doc.insert("timeSigNumerator".into(), num.into());
    doc.insert("timeSigDenominator".into(), den.into());
    doc.insert("masterVolume".into(), master_volume.into());
    doc.insert("masterPlugins".into(), Value::Array(master));
    doc.insert("recordDir".into(), record_dir.into());
    doc.insert("recordMute".into(), record_mute.into());
    doc.insert(
        "settings".into(),
        json!({"metronome": metronome, "metronomeLevel": level, "countIn": count_in}),
    );
    let n = tracks.len();
    doc.insert("tracks".into(), Value::Array(tracks));
    if let Some(fs) = footswitch(&j, session)? {
        doc.insert("footswitch".into(), fs);
    }
    Ok(Parsed {
        doc: Value::Object(doc),
        name,
        tempo,
        tracks: n,
        clips,
        plugins,
    })
}

/// `msg`'s `tracks`, each read and checked as a whole document reads one (no id is made unique: a delta replaces).
pub fn parse_tracks(msg: &Value, checks: &Checks) -> Read<Vec<Value>> {
    let list = Obj(msg)
        .field("tracks")?
        .ok_or_else(|| missing_key("tracks"))?;
    match list {
        Value::Array(items) => items.iter().map(|t| track(t, checks)).collect(),
        Value::Null => Ok(Vec::new()),
        other => Err(super::read::wrong_type("array", other)),
    }
}
