//! tray_status.rs — what the tray's icon and menu say, worded here so the tray draws and decides nothing.
//! Held to the words of the committed `fixtures/local-vectors.json`.

use serde_json::{Map, Value, json};

use crate::protocol::js_text;

/// The download being fetched and how far, how many wait behind it, or the first failure.
pub fn downloads_line(downloads: &[Value]) -> Option<String> {
    let live: Vec<&Value> = downloads
        .iter()
        .filter(|d| d.get("phase").and_then(Value::as_str) != Some("failed"))
        .collect();
    if let Some(d) = live.first() {
        let more = if live.len() > 1 {
            format!(" (+{} more)", live.len() - 1)
        } else {
            String::new()
        };
        let label = js_text(d.get("label"));
        if d.get("phase").and_then(Value::as_str) == Some("unpacking") {
            return Some(format!("↓ Unpacking {label}…{more}"));
        }
        let num = |k: &str| d.get(k).and_then(Value::as_f64).unwrap_or(f64::NAN);
        let (received, total) = (num("received"), num("total"));
        let pct = if total > 0.0 {
            (received * 100.0 / total).floor().min(100.0)
        } else {
            0.0
        };
        return Some(format!("↓ Downloading {label}… {}%{more}", pct as i64));
    }
    downloads
        .first()
        .map(|f| format!("⚠ {}: download failed", js_text(f.get("label"))))
}

/// `trayStatus` of the engine's state: the icon, the menu's line, and the downloads' line or null.
pub fn tray_status(snapshot: Option<&Map<String, Value>>, down: Option<&str>) -> Value {
    let out = |icon: &str, line: String, downloads: Option<String>| json!({ "icon": icon, "line": line, "downloads": downloads });
    if let Some(d) = down.filter(|d| !d.is_empty()) {
        return out("error", format!("⚠ {d}"), None);
    }
    let Some(s) = snapshot else {
        return out("loading", "Starting the engine…".into(), None);
    };
    let list = s
        .get("downloads")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let downloads = downloads_line(list);
    let status = s.get("status").and_then(Value::as_str);
    if status == Some("error") {
        let error = s
            .get("error")
            .filter(|e| crate::truthy(e))
            .map_or("engine error".into(), |e| js_text(Some(e)));
        return out("error", format!("⚠ {error}"), downloads);
    }
    if status != Some("running") {
        let stopped = s.get("userStopped").is_some_and(crate::truthy);
        let line = if stopped {
            "○ Engine stopped"
        } else {
            "○ Engine starting…"
        };
        return out("stopped", line.into(), downloads);
    }
    // Scanning plugins or fetching an instrument: up, but not ready to play yet.
    let busy = !s.contains_key("pluginCount") || downloads.is_some();
    out(
        if busy { "loading" } else { "running" },
        format!("● Engine running · :{}", js_text(s.get("enginePort"))),
        downloads,
    )
}
