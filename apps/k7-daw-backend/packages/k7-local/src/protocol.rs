//! protocol.rs — the NDJSON frames `k7-supervisor` speaks (`apps/k7-daw-engine/k7-supervisor/schema/hostProtocol.schema.json`),
//! read into what a host keeps of them. A snapshot is serialized key for key as
//! `JSON.stringify(snapshotOf(frame))`: the page and the conformance suite compare its bytes.

use serde_json::{Map, Value, json};

/// A stdout line the host understood; anything else is logged as text.
#[derive(Debug, Clone, PartialEq)]
pub enum Frame {
    State(Map<String, Value>),
    Notify(Map<String, Value>),
    Result(Map<String, Value>),
}

/// The snapshot before the first frame, and the defaults a frame's missing fields take, in `EMPTY_SNAPSHOT`'s key order.
pub fn empty_snapshot() -> Map<String, Value> {
    let v = json!({
        "status": "stopped",
        "ownsEngine": false,
        "userStopped": false,
        "engineId": "",
        "enginePort": 8765,
        "registryUrl": "",
        "lastHeartbeatOk": false,
        "plugins": [],
        "pluginsSig": "",
        "audioDevices": [],
        "audioPriority": [],
        "audioMenuSig": "",
        "editPlugins": [],
        "editPluginsSig": "",
        "sessions": [],
        "sessionsSig": "",
        "instances": [],
        "paths": { "root": "", "mergedLog": "" },
        "downloads": [],
        "instruments": [],
        "pairRequests": [],
        "pairedDevices": [],
        "slots": []
    });
    match v {
        Value::Object(m) => m,
        _ => Map::new(),
    }
}

/// One stdout line as a frame; None for anything that is not a state, notify or result object.
pub fn parse_frame(line: &str) -> Option<Frame> {
    let Ok(Value::Object(m)) = serde_json::from_str::<Value>(line) else {
        return None;
    };
    match m.get("t").and_then(Value::as_str) {
        Some("state") => Some(Frame::State(m)),
        Some("notify") => Some(Frame::Notify(m)),
        Some("result") => Some(Frame::Result(m)),
        _ => None,
    }
}

/// `snapshotOf`: every list present, `t` and `owns` dropped, a null `pluginCount` or `instanceCheck` left out.
pub fn snapshot_of(frame: &Map<String, Value>) -> Map<String, Value> {
    let mut s = empty_snapshot();
    let empty_paths = s.get("paths").cloned();
    for (k, v) in frame {
        if !matches!(k.as_str(), "t" | "owns" | "pluginCount" | "instanceCheck") {
            s.insert(k.clone(), v.clone());
        }
    }
    let mut paths = match empty_paths {
        Some(Value::Object(p)) => p,
        _ => Map::new(),
    };
    if let Some(Value::Object(given)) = frame.get("paths") {
        for (k, v) in given {
            paths.insert(k.clone(), v.clone());
        }
    }
    s.insert("paths".into(), Value::Object(paths));
    for k in ["pluginCount", "instanceCheck"] {
        if let Some(v) = frame.get(k).filter(|v| !v.is_null()) {
            s.insert(k.into(), v.clone());
        }
    }
    s
}

/// One command as the line the supervisor's stdin reads, newline included.
pub fn command_line(id: u64, cmd: &str, arg: Option<&str>) -> String {
    let mut m = Map::new();
    m.insert("t".into(), "cmd".into());
    m.insert("id".into(), id.into());
    m.insert("cmd".into(), cmd.into());
    if let Some(a) = arg {
        m.insert("arg".into(), a.into());
    }
    format!("{}\n", Value::Object(m))
}

/// A JavaScript template literal's rendering of a frame field: a string as is, an absent one `undefined`.
pub fn js_text(v: Option<&Value>) -> String {
    match v {
        None => "undefined".into(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Null) => "null".into(),
        Some(Value::Object(_)) => "[object Object]".into(),
        Some(Value::Array(a)) => a
            .iter()
            .map(|x| match x {
                Value::Null => String::new(),
                other => js_text(Some(other)),
            })
            .collect::<Vec<_>>()
            .join(","),
        Some(other) => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lean_frame_reads_as_a_whole_snapshot() {
        let Some(Frame::State(f)) = parse_frame(
            r#"{"t":"state","owns":{"engine":true},"status":"running","lanOpen":true,"pluginCount":null,"paths":{"root":"/r"}}"#,
        ) else {
            panic!("a state frame");
        };
        let s = Value::Object(snapshot_of(&f)).to_string();
        assert!(s.starts_with(r#"{"status":"running","ownsEngine":false"#));
        assert!(s.contains(r#""paths":{"root":"/r","mergedLog":""}"#));
        assert!(s.ends_with(r#""slots":[],"lanOpen":true}"#));
        assert!(!s.contains("pluginCount") && !s.contains("owns\""));
    }

    #[test]
    fn only_three_kinds_of_line_are_frames() {
        assert!(parse_frame("[1]").is_none());
        assert!(parse_frame("hello").is_none());
        assert!(parse_frame(r#"{"t":"engine","action":"stop"}"#).is_none());
        assert!(matches!(
            parse_frame(r#"{"t":"result","id":1,"ok":true}"#),
            Some(Frame::Result(_))
        ));
    }

    #[test]
    fn a_command_is_one_line() {
        assert_eq!(
            command_line(4, "audio.priority", Some("A\u{1f}B")),
            "{\"t\":\"cmd\",\"id\":4,\"cmd\":\"audio.priority\",\"arg\":\"A\\u001fB\"}\n"
        );
        assert_eq!(
            command_line(1, "shutdown", None),
            "{\"t\":\"cmd\",\"id\":1,\"cmd\":\"shutdown\"}\n"
        );
    }
}
