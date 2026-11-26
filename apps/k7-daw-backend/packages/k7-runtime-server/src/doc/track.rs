//! track.rs — one track off the wire, split out so the `tracks` delta reads a track exactly as a whole document does.
//! What a reader gets wrong: a `crossfade`, a `warp` and an `inputFrom` are always written as objects, absent
//! ones with their defaults, because the C++ reader reads each field with `at`.

use super::Checks;
use super::bounds::{MAX_KEY, MAX_PAD_REPEAT, MAX_PAIR, clamp};
use super::plugin::plugin;
use super::read::{Obj, Read, as_int, pair, wrong_type};
use serde_json::{Value, json};

/// The positive integers of a channel list; anything else in it is dropped.
fn channels(t: &Obj, key: &str) -> Read<Vec<i32>> {
    Ok(t.list(key)?
        .iter()
        .filter(|v| matches!(v, Value::Number(n) if n.is_i64() || n.is_u64()))
        .filter_map(|v| as_int(v).ok())
        .filter(|ch| *ch > 0)
        .collect())
}

fn pairs(list: &[Value]) -> Read<Vec<Value>> {
    list.iter()
        .map(|p| pair(p).map(|(a, b)| json!([a, b])))
        .collect()
}

fn note(n: &Value) -> Read<Value> {
    let n = Obj(n);
    Ok(json!({
        "time": n.f64("time", 0.0)?,
        "duration": n.f64("duration", 0.0)?,
        "key": clamp(n.int("key", 0)?, 0, MAX_KEY),
        "vel": n.f64("vel", 1.0)?,
        "channel": n.int("channel", 0)?,
        "rel": n.f64("rel", 1.0)?,
        "repeat": clamp(n.int("repeat", 0)?, 0, MAX_PAD_REPEAT),
        "skip": clamp(n.int("skip", 0)?, 0, MAX_PAD_REPEAT),
    }))
}

fn clip(c: &Value, checks: &Checks) -> Read<Value> {
    let c = Obj(c);
    let id = c.str("id", "")?;
    let midi = c.str("type", "audio")? == "midi";
    let name = c.str("name", "")?;
    let source = checks.readable(c.str("source", "")?, "clip");
    let (start, length, offset) = (
        c.f64("start", 0.0)?,
        c.f64("length", 0.0)?,
        c.f64("offset", 0.0)?,
    );
    let (from, to) = (c.f64("sourceFrom", 0.0)?, c.f64("sourceTo", 0.0)?);
    let (fade_in, fade_out) = (c.f64("fadeIn", 0.0)?, c.f64("fadeOut", 0.0)?);
    let gain = c.f32("gainDb", 0.0)?;
    let (mut x_in, mut x_out) = (false, false);
    if c.has("crossfade") {
        let x = Obj(c.at("crossfade")?);
        (x_in, x_out) = (x.bool("in", false)?, x.bool("out", false)?);
    }
    let (loop_start, loop_end) = (c.f64("loopStart", 0.0)?, c.f64("loopEnd", 0.0)?);
    let disabled = c.bool("disabled", false)?;
    let (mut algorithm, mut markers) = (String::new(), Vec::new());
    if c.has("warp") {
        let w = Obj(c.at("warp")?);
        algorithm = w.str("algorithm", "")?;
        if w.has("markers") {
            markers = pairs(w.list("markers")?)?;
        }
    }
    let notes: Read<Vec<Value>> = c.list("notes")?.iter().map(note).collect();
    Ok(json!({
        "id": id,
        "type": if midi { "midi" } else { "audio" },
        "name": name,
        "source": source,
        "start": start,
        "length": length,
        "offset": offset,
        "sourceFrom": from,
        "sourceTo": to,
        "fadeIn": fade_in,
        "fadeOut": fade_out,
        "gainDb": gain,
        "crossfade": {"in": x_in, "out": x_out},
        "loopStart": loop_start,
        "loopEnd": loop_end,
        "disabled": disabled,
        "warp": {"algorithm": algorithm, "markers": markers},
        "notes": notes?,
    }))
}

pub fn track(v: &Value, checks: &Checks) -> Read<Value> {
    let t = Obj(v);
    let id = t.str("id", "")?;
    let name = t.str("name", &id)?;
    let role = t.str("role", "regular")?;
    let color = t.str("color", "")?;
    let (mute, solo, disabled) = (
        t.bool("mute", false)?,
        t.bool("solo", false)?,
        t.bool("disabled", false)?,
    );
    let monitor = t.str("monitor", "auto")?;
    let input_gain = t.f64("inputGain", 0.0)?;
    let input_channels = channels(&t, "inputChannels")?;
    let output_channels = channels(&t, "outputChannels")?;
    let clips: Read<Vec<Value>> = t.list("clips")?.iter().map(|c| clip(c, checks)).collect();
    let plugins: Read<Vec<Value>> = t
        .list("plugins")?
        .iter()
        .map(|p| plugin(p, checks))
        .collect();
    let mut sends = Vec::new();
    for s in t.list("sends")? {
        let s = Obj(s);
        sends.push(json!({
            "to": s.str("to", "")?,
            "level": s.f64("level", 0.0)?,
            "enabled": s.bool("enabled", false)?,
        }));
    }
    let mut input_from = json!({"plugin": "", "pair": 0});
    if let Some(Value::Object(_)) = t.field("inputFrom")? {
        let f = Obj(t.at("inputFrom")?);
        input_from = json!({
            "plugin": f.str("plugin", "")?,
            "pair": clamp(f.int("pair", 0)?, 0, MAX_PAIR),
        });
    }
    let mut automation = Vec::new();
    for a in t.list("automation")? {
        let a = Obj(a);
        let (param, plugin_id, bipolar) = (
            a.str("param", "")?,
            a.str("pluginId", "")?,
            a.bool("bipolar", false)?,
        );
        let points = match a.field("points")? {
            None | Some(Value::Null) => Vec::new(),
            Some(Value::Array(list)) => pairs(list)?,
            Some(other) => return Err(wrong_type("array", other)),
        };
        automation.push(
            json!({"param": param, "pluginId": plugin_id, "bipolar": bipolar, "points": points}),
        );
    }
    Ok(json!({
        "id": id,
        "name": name,
        "role": role,
        "color": color,
        "mute": mute,
        "solo": solo,
        "disabled": disabled,
        "monitor": monitor,
        "inputGain": input_gain,
        "inputChannels": input_channels,
        "outputChannels": output_channels,
        "clips": clips?,
        "plugins": plugins?,
        "sends": sends,
        "inputFrom": input_from,
        "automation": automation,
    }))
}
