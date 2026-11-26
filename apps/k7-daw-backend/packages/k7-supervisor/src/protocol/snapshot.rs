//! snapshot.rs — the whole of what an app draws, and its `state` frame in a fixed key order: a frame is
//! sent only when its bytes change. The writer is by hand because the wire is not a
//! derive's: `error` absent when empty, `downloads` and `rendering` absent at zero, `pluginCount` null.

use serde_json::{Map, Value, json};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct AudioDevice {
    pub name: String,
    pub device_type: String,
    pub current: bool,
    /// The device the engine takes its INPUTS from, another than `current` when the setup is split.
    pub current_input: bool,
    pub inputs: u32,
    pub outputs: u32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct EditPlugin {
    pub session: String,
    pub id: String,
    pub name: String,
    pub has_native_plugin_ui: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Plugin {
    pub name: String,
    pub format: String,
    pub manufacturer: String,
    pub role: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Session {
    pub id: String,
    pub project: String,
    pub loaded: bool,
    pub playing: bool,
    pub tracks: u32,
    pub listeners: u32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PairRequest {
    pub request_id: String,
    pub name: String,
    pub ip: String,
    pub code: String,
    /// Epoch milliseconds.
    pub since: i64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PairedDevice {
    pub device_id: String,
    pub name: String,
    pub created: i64,
    pub last_seen: i64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SlotRow {
    pub id: String,
    pub name: String,
    pub chain: String,
    pub saved_at: String,
    pub protocol: i64,
    /// The CC whose press bypasses its chain.
    pub footswitch: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Instance {
    pub url: String,
    pub removable: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct InstanceCheck {
    pub url: String,
    /// "checking" or "refused".
    pub state: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Download {
    pub name: String,
    pub label: String,
    pub received: u64,
    pub total: u64,
    /// "queued", "downloading", "unpacking" or "failed".
    pub phase: String,
    pub error: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct InstrumentRow {
    pub name: String,
    pub label: String,
    pub bytes: u64,
    pub wanted: bool,
    pub installed: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SafeMode {
    pub crashes: u32,
    pub suspect: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    /// "stopped", "running" or "error".
    pub status: String,
    pub error: String,
    pub safe_mode: Option<SafeMode>,
    pub owns_engine: bool,
    pub user_stopped: bool,
    pub engine_id: String,
    pub engine_port: u16,
    pub registry_url: String,
    pub last_heartbeat_ok: bool,
    pub sessions: Vec<Session>,
    pub sessions_sig: String,
    pub plugin_count: Option<u32>,
    pub plugins: Vec<Plugin>,
    pub plugins_sig: String,
    pub audio_devices: Vec<AudioDevice>,
    pub audio_priority: Vec<String>,
    pub audio_menu_sig: String,
    pub lan_open: bool,
    pub edit_plugins: Vec<EditPlugin>,
    pub edit_plugins_sig: String,
    pub instances: Vec<Instance>,
    pub instance_check: Option<InstanceCheck>,
    pub root_path: String,
    pub merged_log_path: String,
    pub downloads: Vec<Download>,
    /// Renders running in the engine for a server's job; the app stays busy while one does.
    pub rendering: u32,
    pub instruments: Vec<InstrumentRow>,
    pub pair_requests: Vec<PairRequest>,
    pub paired_devices: Vec<PairedDevice>,
    pub slots: Vec<SlotRow>,
    /// The enabled slot's id, empty for none.
    pub enabled_slot: String,
}

impl Default for Snapshot {
    fn default() -> Self {
        Snapshot {
            status: "stopped".into(),
            error: String::new(),
            safe_mode: None,
            owns_engine: false,
            user_stopped: false,
            engine_id: String::new(),
            engine_port: 8765,
            registry_url: String::new(),
            last_heartbeat_ok: false,
            sessions: Vec::new(),
            sessions_sig: String::new(),
            plugin_count: None,
            plugins: Vec::new(),
            plugins_sig: String::new(),
            audio_devices: Vec::new(),
            audio_priority: Vec::new(),
            audio_menu_sig: String::new(),
            lan_open: false,
            edit_plugins: Vec::new(),
            edit_plugins_sig: String::new(),
            instances: Vec::new(),
            instance_check: None,
            root_path: String::new(),
            merged_log_path: String::new(),
            downloads: Vec::new(),
            rendering: 0,
            instruments: Vec::new(),
            pair_requests: Vec::new(),
            paired_devices: Vec::new(),
            slots: Vec::new(),
            enabled_slot: String::new(),
        }
    }
}

fn rows<T>(items: &[T], row: impl Fn(&T) -> Value) -> Value {
    Value::Array(items.iter().map(row).collect())
}

/// The `state` frame for a snapshot, keys in the fixtures' order.
pub fn state_frame(s: &Snapshot) -> Value {
    let mut f = Map::new();
    let mut put = |k: &str, v: Value| {
        f.insert(k.to_string(), v);
    };
    put("t", json!("state"));
    put("owns", json!({"engine": true, "engineQueries": true}));
    put("status", json!(s.status));
    if !s.error.is_empty() {
        put("error", json!(s.error));
    }
    if let Some(m) = &s.safe_mode {
        put(
            "safeMode",
            json!({"crashes": m.crashes, "suspect": m.suspect}),
        );
    }
    put("ownsEngine", json!(s.owns_engine));
    put("userStopped", json!(s.user_stopped));
    put("engineId", json!(s.engine_id));
    put("enginePort", json!(s.engine_port));
    put("registryUrl", json!(s.registry_url));
    put("lastHeartbeatOk", json!(s.last_heartbeat_ok));
    put(
        "pluginCount",
        s.plugin_count.map_or(Value::Null, |n| json!(n)),
    );
    put(
        "plugins",
        rows(
            &s.plugins,
            |p| json!({"name": p.name, "format": p.format, "manufacturer": p.manufacturer, "role": p.role}),
        ),
    );
    put("pluginsSig", json!(s.plugins_sig));
    put(
        "audioDevices",
        rows(
            &s.audio_devices,
            |d| json!({"name": d.name, "deviceType": d.device_type, "current": d.current, "currentInput": d.current_input, "inputs": d.inputs, "outputs": d.outputs}),
        ),
    );
    put("audioPriority", json!(s.audio_priority));
    put("audioMenuSig", json!(s.audio_menu_sig));
    put("lanOpen", json!(s.lan_open));
    put(
        "editPlugins",
        rows(
            &s.edit_plugins,
            |e| json!({"session": e.session, "id": e.id, "name": e.name, "hasNativePluginUI": e.has_native_plugin_ui}),
        ),
    );
    put("editPluginsSig", json!(s.edit_plugins_sig));
    put(
        "sessions",
        rows(
            &s.sessions,
            |x| json!({"id": x.id, "project": x.project, "loaded": x.loaded, "playing": x.playing, "tracks": x.tracks, "listeners": x.listeners}),
        ),
    );
    put("sessionsSig", json!(s.sessions_sig));
    put(
        "instances",
        rows(
            &s.instances,
            |i| json!({"url": i.url, "removable": i.removable}),
        ),
    );
    put(
        "instanceCheck",
        match &s.instance_check {
            None => Value::Null,
            Some(c) if c.message.is_empty() => json!({"url": c.url, "state": c.state}),
            Some(c) => json!({"url": c.url, "state": c.state, "message": c.message}),
        },
    );
    put(
        "paths",
        json!({"root": s.root_path, "mergedLog": s.merged_log_path}),
    );
    if s.rendering > 0 {
        put("rendering", json!(s.rendering));
    }
    if !s.downloads.is_empty() {
        put(
            "downloads",
            rows(&s.downloads, |d| {
                let mut o = json!({"name": d.name, "label": d.label, "received": d.received, "total": d.total, "phase": d.phase});
                if !d.error.is_empty() {
                    o["error"] = json!(d.error);
                }
                o
            }),
        );
    }
    put(
        "instruments",
        rows(
            &s.instruments,
            |i| json!({"name": i.name, "label": i.label, "bytes": i.bytes, "wanted": i.wanted, "installed": i.installed}),
        ),
    );
    put(
        "pairRequests",
        rows(
            &s.pair_requests,
            |r| json!({"requestId": r.request_id, "name": r.name, "ip": r.ip, "code": r.code, "since": r.since}),
        ),
    );
    put(
        "pairedDevices",
        rows(
            &s.paired_devices,
            |d| json!({"deviceId": d.device_id, "name": d.name, "created": d.created, "lastSeen": d.last_seen}),
        ),
    );
    put(
        "slots",
        rows(
            &s.slots,
            |x| json!({"id": x.id, "name": x.name, "chain": x.chain, "savedAt": x.saved_at, "protocol": x.protocol, "footswitch": x.footswitch}),
        ),
    );
    put(
        "enabledSlot",
        if s.enabled_slot.is_empty() {
            Value::Null
        } else {
            json!(s.enabled_slot)
        },
    );
    Value::Object(f)
}
