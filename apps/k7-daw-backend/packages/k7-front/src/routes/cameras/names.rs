//! names.rs — `@studio/web-protocol/cameras`' pure helpers as the hub runs them: a device name cleaned and made
//! unique, the camera a label names, a frame read, and `K7_CAMERA_ICE_SERVERS` parsed. The browser keeps the
//! TypeScript copies, so both read `fixtures/camera-vectors.json` (`apps/k7-daw-backend/docs/routes.md`).

use serde_json::{Map, Value, json};

use crate::js::{is_js_space, truthy};

/// The longest device name kept, in UTF-16 units as `String.prototype.slice` counts.
const CAMERA_NAME_MAX: usize = 60;

/// What a remote label starts with, in UTF-16 units: `remoteName` slices that many whatever the label is.
const PREFIX_UNITS: usize = "remote:".len();

/// At most `units` UTF-16 units, never splitting a character (where JavaScript would leave half a pair).
fn utf16_prefix(s: &str, units: usize) -> &str {
    let mut used = 0;
    for (i, c) in s.char_indices() {
        used += c.len_utf16();
        if used > units {
            return &s[..i];
        }
    }
    s
}

/// `cleanCameraName`: one line, trimmed, capped; empty falls back to `fallback`.
pub fn clean_camera_name(raw: &str, fallback: &str) -> String {
    let mut one = String::new();
    let mut gap = false;
    for c in raw.chars() {
        let c = if (c as u32) < 32 || c == '\u{7f}' {
            ' '
        } else {
            c
        };
        if is_js_space(c) {
            gap = true;
            continue;
        }
        if gap && !one.is_empty() {
            one.push(' ');
        }
        gap = false;
        one.push(c);
    }
    let one = if one.is_empty() { fallback } else { &one };
    utf16_prefix(one, CAMERA_NAME_MAX).to_string()
}

/// `uniqueCameraName`: its own name, else suffixed ` (2)`, ` (3)`… among those taken, ignoring case.
pub fn unique_camera_name(want: &str, taken: &[&str]) -> String {
    let used: Vec<String> = taken.iter().map(|t| t.to_lowercase()).collect();
    let free = |n: &str| !used.contains(&n.to_lowercase());
    if free(want) {
        return want.to_string();
    }
    (2..)
        .map(|n| format!("{want} ({n})"))
        .find(|n| free(n))
        .unwrap_or_default()
}

/// One camera that is on: its name and when it came on, in epoch ms.
pub struct Listed<'a> {
    pub name: &'a str,
    pub since: f64,
}

/// `matchRemoteCamera`: the name a label picks (exact, ignoring case, containment, else the newest) and why, when it is not the one named.
pub fn match_remote_camera(label: &str, cameras: &[Listed]) -> (Option<String>, Option<String>) {
    let Some(mut newest) = cameras.first() else {
        return (None, Some("no remote camera is on".into()));
    };
    for c in cameras {
        if c.since > newest.since {
            newest = c;
        }
    }
    let want = label[utf16_prefix(label, PREFIX_UNITS).len()..].trim_matches(is_js_space);
    if want.is_empty() {
        return (Some(newest.name.into()), None);
    }
    let low = want.to_lowercase();
    let hit = cameras
        .iter()
        .find(|c| c.name == want)
        .or_else(|| cameras.iter().find(|c| c.name.to_lowercase() == low))
        .or_else(|| {
            cameras
                .iter()
                .find(|c| c.name.to_lowercase().contains(&low))
        })
        .or_else(|| {
            cameras
                .iter()
                .find(|c| low.contains(&c.name.to_lowercase()))
        });
    if let Some(hit) = hit {
        return (Some(hit.name.into()), None);
    }
    let on: Vec<&str> = cameras.iter().map(|c| c.name).collect();
    let problem = format!(
        "no remote camera called \"{want}\" is on — showing \"{}\" (on: {})",
        newest.name,
        on.join(", ")
    );
    (Some(newest.name.into()), Some(problem))
}

/// `readCameraFrame`: a JSON object with a string `type`, else None.
pub fn read_camera_frame(text: &str) -> Option<Map<String, Value>> {
    match serde_json::from_str::<Value>(text) {
        Ok(Value::Object(f)) if f.get("type").is_some_and(Value::is_string) => Some(f),
        _ => None,
    }
}

/// `iceServersFromEnv`: a JSON array of `RTCIceServer`, or comma-separated URLs; unset is none.
pub fn ice_servers(raw: Option<&str>) -> Vec<Value> {
    let text = raw.unwrap_or("").trim_matches(is_js_space);
    if text.is_empty() {
        return Vec::new();
    }
    if text.starts_with('[') {
        return match serde_json::from_str::<Value>(text) {
            Ok(Value::Array(list)) => list
                .into_iter()
                .filter(|s| s.get("urls").is_some_and(truthy))
                .collect(),
            Ok(_) => Vec::new(),
            Err(_) => {
                eprintln!("K7_CAMERA_ICE_SERVERS is not valid JSON — ignored");
                Vec::new()
            }
        };
    }
    text.split(',')
        .map(|u| u.trim_matches(is_js_space))
        .filter(|u| !u.is_empty())
        .map(|u| json!({ "urls": u }))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vectors() -> Value {
        // Compiled in, so the test binary runs on a phone that has no checkout.
        serde_json::from_str(include_str!("../../../../../fixtures/camera-vectors.json")).unwrap()
    }

    fn s(v: &Value) -> &str {
        v.as_str().unwrap()
    }

    #[test]
    fn cleans_as_typescript() {
        for v in vectors()["clean"].as_array().unwrap() {
            let fallback = v.get("fallback").map_or("camera", s);
            assert_eq!(
                clean_camera_name(s(&v["raw"]), fallback),
                s(&v["out"]),
                "{v}"
            );
        }
    }

    #[test]
    fn uniques_as_typescript() {
        for v in vectors()["unique"].as_array().unwrap() {
            let taken: Vec<&str> = v["taken"].as_array().unwrap().iter().map(s).collect();
            assert_eq!(
                unique_camera_name(s(&v["want"]), &taken),
                s(&v["out"]),
                "{v}"
            );
        }
    }

    #[test]
    fn matches_as_typescript() {
        for v in vectors()["match"].as_array().unwrap() {
            let cameras: Vec<Listed> = v["cameras"]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| Listed {
                    name: s(&c["name"]),
                    since: c["since"].as_f64().unwrap(),
                })
                .collect();
            let (name, problem) = match_remote_camera(s(&v["label"]), &cameras);
            assert_eq!(name.as_deref(), v["out"]["name"].as_str(), "{v}");
            assert_eq!(
                problem.as_deref(),
                v["out"].get("problem").and_then(Value::as_str),
                "{v}"
            );
        }
    }

    #[test]
    fn reads_frames_as_typescript() {
        for v in vectors()["frame"].as_array().unwrap() {
            let read = read_camera_frame(s(&v["text"])).map(Value::Object);
            assert_eq!(read.unwrap_or(Value::Null), v["out"], "{v}");
        }
    }

    #[test]
    fn reads_ice_servers_as_typescript() {
        for v in vectors()["ice"].as_array().unwrap() {
            let raw = v.get("raw").and_then(Value::as_str);
            assert_eq!(Value::Array(ice_servers(raw)), v["out"], "{v}");
        }
    }

    #[test]
    fn a_cap_never_splits_a_character() {
        let name = format!("{}🎥", "x".repeat(59));
        assert_eq!(clean_camera_name(&name, "camera"), "x".repeat(59));
    }
}
