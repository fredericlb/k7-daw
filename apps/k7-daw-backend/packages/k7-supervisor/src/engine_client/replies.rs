//! replies.rs — the engine's reply frames as the supervisor's rows, read leniently: a missing or mistyped
//! field is its empty value, as the C++ reader had it, since an engine of another build must not crash us.

use serde_json::Value;

use crate::protocol::{AudioDevice, EditPlugin, PairRequest, PairedDevice, Plugin, Session};

/// The installed catalogue, and whether the engine is still scanning it, which means this list is about to change.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Catalogue {
    pub plugins: Vec<Plugin>,
    pub scanning: bool,
}

fn text(o: &Value, key: &str) -> String {
    o.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn flag(o: &Value, key: &str) -> bool {
    o.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn int(o: &Value, key: &str) -> i64 {
    match o.get(key) {
        Some(Value::Number(n)) => n
            .as_i64()
            .unwrap_or_else(|| n.as_f64().unwrap_or(0.0) as i64),
        _ => 0,
    }
}

fn count(o: &Value, key: &str) -> u32 {
    int(o, key).clamp(0, i64::from(u32::MAX)) as u32
}

fn rows<'a>(msg: &'a Value, key: &str) -> impl Iterator<Item = &'a Value> {
    msg.get(key).and_then(Value::as_array).into_iter().flatten()
}

pub fn devices(msg: &Value) -> Vec<AudioDevice> {
    rows(msg, "devices")
        .map(|d| AudioDevice {
            name: text(d, "name"),
            device_type: text(d, "type"),
            current: flag(d, "current"),
            current_input: flag(d, "currentInput"),
            inputs: count(d, "inputs"),
            outputs: count(d, "outputs"),
        })
        .collect()
}

/// Sorted by lowercase name then format, so a reordered scan is not a changed catalogue.
pub fn catalogue(msg: &Value) -> Catalogue {
    let mut plugins: Vec<Plugin> = rows(msg, "plugins")
        .map(|p| Plugin {
            name: text(p, "name"),
            format: text(p, "format"),
            manufacturer: text(p, "manufacturer"),
            role: text(p, "role"),
        })
        .collect();
    plugins.sort_by_cached_key(|p| (p.name.to_ascii_lowercase(), p.format.clone()));
    Catalogue {
        plugins,
        scanning: flag(msg, "scanning"),
    }
}

pub fn edit_plugins(msg: &Value) -> Vec<EditPlugin> {
    rows(msg, "plugins")
        .map(|e| EditPlugin {
            session: text(e, "session"),
            id: text(e, "id"),
            name: text(e, "name"),
            has_native_plugin_ui: flag(e, "hasNativePluginUI"),
        })
        .collect()
}

pub fn sessions(msg: &Value) -> Vec<Session> {
    rows(msg, "sessions")
        .map(|s| Session {
            id: text(s, "id"),
            project: text(s, "project"),
            loaded: flag(s, "loaded"),
            playing: flag(s, "playing"),
            tracks: count(s, "tracks"),
            listeners: count(s, "listeners"),
        })
        .collect()
}

pub fn pair_requests(msg: &Value) -> Vec<PairRequest> {
    rows(msg, "requests")
        .map(|r| PairRequest {
            request_id: text(r, "requestId"),
            name: text(r, "name"),
            ip: text(r, "ip"),
            code: text(r, "code"),
            since: int(r, "since"),
        })
        .collect()
}

pub fn paired_devices(msg: &Value) -> Vec<PairedDevice> {
    rows(msg, "devices")
        .map(|d| PairedDevice {
            device_id: text(d, "deviceId"),
            name: text(d, "name"),
            created: int(d, "created"),
            last_seen: int(d, "lastSeen"),
        })
        .collect()
}
