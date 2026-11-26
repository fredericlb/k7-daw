//! plugin.rs — one plugin off the wire, shared by a track's list and the master chain's so the two cannot drift.
//! What a reader gets wrong: every field is written, defaults included, since the C++ reader reads each with
//! `at`; and the order the paths are checked in is the order their warnings are said in.

use super::Checks;
use super::bounds::{MAX_KEY, MAX_PAIR, clamp};
use super::read::{Obj, Read, as_f64, wrong_type};
use serde_json::{Map, Value, json};

pub fn plugin(v: &Value, c: &Checks) -> Read<Value> {
    let p = Obj(v);
    let mut out = Map::new();
    let mut put = |k: &str, v: Value| out.insert(k.to_owned(), v);
    put("id", p.str("id", "")?.into());
    put("type", p.str("type", "")?.into());
    put("name", p.str("name", "")?.into());
    put("path", c.readable(p.str("path", "")?, "preset").into());
    put(
        "presetSave",
        c.preset_target(p.str("presetSave", "")?).into(),
    );
    put("role", p.str("role", "")?.into());
    put("enabled", p.bool("enabled", true)?.into());
    put(
        "namFile",
        c.readable(p.str("namFile", "")?, "amp model").into(),
    );
    put(
        "irFile",
        c.readable(p.str("irFile", "")?, "impulse response").into(),
    );
    put(
        "deckFile",
        c.readable(p.str("deckFile", "")?, "record").into(),
    );
    put("sfzFile", c.instrument_ref(p.str("sfzFile", "")?).into());
    put(
        "surgePatch",
        c.readable(p.str("surgePatch", "")?, "surge patch").into(),
    );
    put("dspGraph", p.str("dspGraph", "")?.into());
    put("patternSource", p.str("patternSource", "")?.into());
    put("patternLabel", p.str("patternLabel", "")?.into());
    put("patternCatchAll", p.bool("patternCatchAll", false)?.into());
    put("patternClaimed", json!(p.strings("patternClaimed")?));
    put("patternPlays", p.str("patternPlays", "")?.into());
    put("patternOnly", p.bool("patternOnly", true)?.into());
    let banks: Vec<String> = p
        .strings("patternBanks")?
        .into_iter()
        .map(|bank| c.readable(bank, "sample bank"))
        .filter(|dir| !dir.is_empty())
        .collect();
    put("patternBanks", json!(banks));
    put("sidechainFrom", p.str("sidechainFrom", "")?.into());
    let mut taps = Vec::new();
    for o in p.array_if("tapOuts") {
        let o = Obj(o);
        taps.push(json!({
            "note": clamp(o.int("note", 0)?, 0, MAX_KEY),
            "pair": clamp(o.int("pair", 0)?, 0, MAX_PAIR),
        }));
    }
    put("tapOuts", Value::Array(taps));
    let mut params = Map::new();
    match p.field("params")? {
        None | Some(Value::Null) => {}
        Some(Value::Object(m)) => {
            for (k, v) in m {
                params.insert(k.clone(), json!(as_f64(v)?));
            }
        }
        Some(other) => return Err(wrong_type("object", other)),
    }
    put("params", Value::Object(params));
    let mut rack = Vec::new();
    for e in p.array_if("rack") {
        let e = Obj(e);
        rack.push(json!({
            "symbol": e.str("symbol", "")?,
            "note": clamp(e.int("note", 0)?, 0, MAX_KEY),
            "clip": c.readable(e.str("clip", "")?, "rack sample"),
            "start": e.f64("start", 0.0)?,
            "length": e.f64("length", 0.0)?,
        }));
    }
    put("rack", Value::Array(rack));
    Ok(Value::Object(out))
}
