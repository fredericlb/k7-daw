//! snapshot.rs — the whole of what an app draws, and its `state` frame in a fixed key order: a frame is
//! sent only when its bytes change. The wire is `wire.rs`'s structs, not these: `error` absent when empty,
//! `downloads` and `rendering` absent at zero, `pluginCount` null.

use super::wire::HostState;
use serde_json::Value;

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

/// The `state` frame for a snapshot, keys in the fixtures' order.
pub fn state_frame(s: &Snapshot) -> Value {
    serde_json::to_value(HostState::from(s)).unwrap_or(Value::Null)
}
