//! video_probe.rs — what a video file IS, asked of ffprobe: `probeArgs` and the facts read from its JSON (`video-prep/probe.ts`).
//! What a reader gets wrong: this is the pure half; the spawn is the front's, and the vectors (`tests/vectors/host-probe.json`)
//! pin every JS coercion (`Number(x) || 0`, `a ?? b`, `Math.round`) this module spells by hand.

use serde_json::{Map, Value, json};

/// Only local files and pipes: a crafted container must not make ffprobe open a URL.
const PROTOCOLS: [&str; 2] = ["-protocol_whitelist", "file,pipe"];

/// A path named as a file, so a colon in its name is never read as a protocol.
fn as_file_input(source: &str) -> String {
    if source == "-" || source.starts_with("pipe:") || source.starts_with("file:") {
        source.to_string()
    } else {
        format!("file:{source}")
    }
}

/// ffprobe's arguments for `source`, as `probeArgs` spells them.
pub fn probe_args(source: &str) -> Vec<String> {
    let show = "stream=codec_name,width,height,pix_fmt,avg_frame_rate:stream_side_data=rotation:stream_tags=rotate:format=duration,format_name";
    let mut args: Vec<String> = [
        "-v",
        "error",
        "-select_streams",
        "v:0",
        "-show_entries",
        show,
        "-of",
        "json",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    args.extend(PROTOCOLS.iter().map(|s| s.to_string()));
    args.push("-i".into());
    args.push(as_file_input(source));
    args
}

/// JavaScript's `Number(v) || 0` of a JSON value: a number as it is, a string by its numeric reading, anything else 0.
fn number(v: Option<&Value>) -> f64 {
    let n = match v {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0),
        Some(Value::String(s)) => {
            let t = s.trim();
            if t.is_empty() {
                0.0
            } else {
                t.parse::<f64>().unwrap_or(0.0)
            }
        }
        Some(Value::Bool(true)) => 1.0,
        _ => 0.0,
    };
    if n.is_finite() { n } else { 0.0 }
}

/// A whole number as an integer, as JavaScript writes one, so a reader sees 1280 and not 1280.0.
pub(crate) fn json_number(n: f64) -> Value {
    if n.fract() == 0.0 && n.abs() < 9_007_199_254_740_992.0 {
        json!(n as i64)
    } else {
        json!(n)
    }
}

/// `String(v ?? "")` of a JSON value.
fn text(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Bool(b)) => b.to_string(),
        _ => String::new(),
    }
}

/// `"30000/1001"` as 29.97: ffprobe states a rate as a fraction; a missing one is `0/1`.
fn rate(v: Option<&Value>) -> f64 {
    let s = match v {
        None | Some(Value::Null) => "0/1".to_string(),
        other => text(other),
    };
    let mut parts = s
        .split('/')
        .map(|p| p.trim().parse::<f64>().unwrap_or(f64::NAN));
    let (n, d) = (parts.next().unwrap_or(f64::NAN), parts.next());
    let r = match d {
        Some(d) if d != 0.0 && !d.is_nan() => n / d,
        _ => {
            if n.is_nan() {
                0.0
            } else {
                n
            }
        }
    };
    if r.is_finite() { r } else { 0.0 }
}

/// A stream's display rotation as a quarter turn in [0, 360): side data first, then the `rotate` tag of older builds.
pub fn rotation_of(stream: &Value) -> u32 {
    let side = stream["side_data_list"]
        .as_array()
        .and_then(|l| l.iter().find(|d| d.get("rotation").is_some()))
        .map(|d| &d["rotation"]);
    let tag = stream["tags"].get("rotate");
    let chosen = [side, tag].into_iter().flatten().find(|v| !v.is_null());
    let deg = number(chosen);
    // `Math.round` rounds a half up, toward +infinity.
    let quarter = (deg / 90.0 + 0.5).floor() as i64;
    (quarter * 90).rem_euclid(360) as u32
}

/// The facts ffprobe's JSON states for the first video stream, or `None` when it names none; `Err` for text that is no JSON.
pub fn facts_of(out: &str) -> Result<Option<Value>, String> {
    let j: Value =
        serde_json::from_str(out).map_err(|e| format!("ffprobe's answer is no JSON: {e}"))?;
    let Some(s) = j["streams"]
        .as_array()
        .and_then(|l| l.first())
        .filter(|s| !s.is_null())
    else {
        return Ok(None);
    };
    let f = &j["format"];
    let mut m = Map::new();
    m.insert("codec".into(), json!(text(s.get("codec_name"))));
    m.insert("width".into(), json_number(number(s.get("width"))));
    m.insert("height".into(), json_number(number(s.get("height"))));
    m.insert("pixFmt".into(), json!(text(s.get("pix_fmt"))));
    m.insert("fps".into(), json_number(rate(s.get("avg_frame_rate"))));
    m.insert("seconds".into(), json_number(number(f.get("duration"))));
    m.insert("container".into(), json!(text(f.get("format_name"))));
    let rotation = rotation_of(s);
    if rotation != 0 {
        m.insert("rotation".into(), json!(rotation));
    }
    Ok(Some(Value::Object(m)))
}
