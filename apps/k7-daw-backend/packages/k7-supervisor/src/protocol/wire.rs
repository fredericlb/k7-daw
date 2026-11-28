//! wire.rs — the `state` frame as serde structs, in the order the fixtures print their keys; the page's
//! `HostState` type is generated from them (docs/ts-types.md). A reader would get wrong that these are the wire,
//! not `Snapshot`: a key absent when empty, `null` where the app reads null, and rows without the engine-only fields.

use super::Snapshot;
use serde::Serialize;

macro_rules! wire {
    ($(#[$m:meta])* $name:ident { $($(#[$f:meta])* $field:ident : $ty:ty),* $(,)? }) => {
        $(#[$m])*
        #[derive(Serialize)]
        #[cfg_attr(feature = "ts", derive(ts_rs::TS))]
        #[serde(rename_all = "camelCase")]
        pub struct $name { $($(#[$f])* pub $field: $ty),* }
    };
}

wire!(HostOwns {
    engine: bool,
    engine_queries: bool
});
wire!(HostSafeMode {
    crashes: u32,
    suspect: String
});
wire!(HostPlugin {
    name: String,
    format: String,
    manufacturer: String,
    role: String
});
wire!(HostAudioDevice {
    name: String,
    device_type: String,
    current: bool,
    current_input: bool,
    inputs: u32,
    outputs: u32,
});
wire!(HostEditPlugin {
    session: String,
    id: String,
    name: String,
    #[serde(rename = "hasNativePluginUI")]
    #[cfg_attr(feature = "ts", ts(rename = "hasNativePluginUI"))]
    has_native_plugin_ui: bool,
});
wire!(HostSession {
    id: String,
    project: String,
    loaded: bool,
    playing: bool,
    tracks: u32,
    listeners: u32
});
wire!(HostInstance {
    url: String,
    removable: bool
});
wire!(
    HostInstanceCheck {
        url: String,
        state: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        message: Option<String>,
    }
);
wire!(HostPaths {
    root: String,
    merged_log: String
});
wire!(
    HostDownload {
        name: String,
        label: String,
        #[cfg_attr(feature = "ts", ts(type = "number"))]
        received: u64,
        #[cfg_attr(feature = "ts", ts(type = "number"))]
        total: u64,
        #[cfg_attr(feature = "ts", ts(type = "\"queued\" | \"downloading\" | \"unpacking\" | \"failed\""))]
        phase: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        error: Option<String>,
    }
);
wire!(HostInstrument {
    name: String,
    label: String,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    bytes: u64,
    wanted: bool,
    installed: bool,
});
wire!(HostPairRequest {
    request_id: String,
    name: String,
    ip: String,
    code: String,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    since: i64,
});
wire!(HostPairedDevice {
    device_id: String,
    name: String,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    created: i64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    last_seen: i64,
});
wire!(
    HostSlot {
        id: String,
        name: String,
        chain: String,
        saved_at: String,
        #[cfg_attr(feature = "ts", ts(type = "number"))]
        protocol: i64,
        #[cfg_attr(feature = "ts", ts(type = "number | null"))]
        footswitch: Option<i64>,
    }
);

wire!(
    /// The supervisor's whole picture, one `state` frame.
    HostState {
        #[cfg_attr(feature = "ts", ts(type = "\"state\""))]
        t: &'static str,
        owns: HostOwns,
        #[cfg_attr(feature = "ts", ts(type = "\"stopped\" | \"running\" | \"error\""))]
        status: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        error: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        safe_mode: Option<HostSafeMode>,
        owns_engine: bool,
        user_stopped: bool,
        engine_id: String,
        engine_port: u16,
        registry_url: String,
        last_heartbeat_ok: bool,
        plugin_count: Option<u32>,
        plugins: Vec<HostPlugin>,
        plugins_sig: String,
        audio_devices: Vec<HostAudioDevice>,
        audio_priority: Vec<String>,
        audio_menu_sig: String,
        lan_open: bool,
        edit_plugins: Vec<HostEditPlugin>,
        edit_plugins_sig: String,
        sessions: Vec<HostSession>,
        sessions_sig: String,
        instances: Vec<HostInstance>,
        instance_check: Option<HostInstanceCheck>,
        paths: HostPaths,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        rendering: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        downloads: Option<Vec<HostDownload>>,
        instruments: Vec<HostInstrument>,
        pair_requests: Vec<HostPairRequest>,
        paired_devices: Vec<HostPairedDevice>,
        slots: Vec<HostSlot>,
        enabled_slot: Option<String>,
    }
);

fn some(s: &str) -> Option<String> {
    (!s.is_empty()).then(|| s.to_string())
}

impl From<&Snapshot> for HostState {
    fn from(s: &Snapshot) -> Self {
        HostState {
            t: "state",
            owns: HostOwns {
                engine: true,
                engine_queries: true,
            },
            status: s.status.clone(),
            error: some(&s.error),
            safe_mode: s.safe_mode.as_ref().map(|m| HostSafeMode {
                crashes: m.crashes,
                suspect: m.suspect.clone(),
            }),
            owns_engine: s.owns_engine,
            user_stopped: s.user_stopped,
            engine_id: s.engine_id.clone(),
            engine_port: s.engine_port,
            registry_url: s.registry_url.clone(),
            last_heartbeat_ok: s.last_heartbeat_ok,
            plugin_count: s.plugin_count,
            plugins: s
                .plugins
                .iter()
                .map(|p| HostPlugin {
                    name: p.name.clone(),
                    format: p.format.clone(),
                    manufacturer: p.manufacturer.clone(),
                    role: p.role.clone(),
                })
                .collect(),
            plugins_sig: s.plugins_sig.clone(),
            audio_devices: s
                .audio_devices
                .iter()
                .map(|d| HostAudioDevice {
                    name: d.name.clone(),
                    device_type: d.device_type.clone(),
                    current: d.current,
                    current_input: d.current_input,
                    inputs: d.inputs,
                    outputs: d.outputs,
                })
                .collect(),
            audio_priority: s.audio_priority.clone(),
            audio_menu_sig: s.audio_menu_sig.clone(),
            lan_open: s.lan_open,
            edit_plugins: s
                .edit_plugins
                .iter()
                .map(|e| HostEditPlugin {
                    session: e.session.clone(),
                    id: e.id.clone(),
                    name: e.name.clone(),
                    has_native_plugin_ui: e.has_native_plugin_ui,
                })
                .collect(),
            edit_plugins_sig: s.edit_plugins_sig.clone(),
            sessions: s
                .sessions
                .iter()
                .map(|x| HostSession {
                    id: x.id.clone(),
                    project: x.project.clone(),
                    loaded: x.loaded,
                    playing: x.playing,
                    tracks: x.tracks,
                    listeners: x.listeners,
                })
                .collect(),
            sessions_sig: s.sessions_sig.clone(),
            instances: s
                .instances
                .iter()
                .map(|i| HostInstance {
                    url: i.url.clone(),
                    removable: i.removable,
                })
                .collect(),
            instance_check: s.instance_check.as_ref().map(|c| HostInstanceCheck {
                url: c.url.clone(),
                state: c.state.clone(),
                message: some(&c.message),
            }),
            paths: HostPaths {
                root: s.root_path.clone(),
                merged_log: s.merged_log_path.clone(),
            },
            rendering: (s.rendering > 0).then_some(s.rendering),
            downloads: (!s.downloads.is_empty()).then(|| {
                s.downloads
                    .iter()
                    .map(|d| HostDownload {
                        name: d.name.clone(),
                        label: d.label.clone(),
                        received: d.received,
                        total: d.total,
                        phase: d.phase.clone(),
                        error: some(&d.error),
                    })
                    .collect()
            }),
            instruments: s
                .instruments
                .iter()
                .map(|i| HostInstrument {
                    name: i.name.clone(),
                    label: i.label.clone(),
                    bytes: i.bytes,
                    wanted: i.wanted,
                    installed: i.installed,
                })
                .collect(),
            pair_requests: s
                .pair_requests
                .iter()
                .map(|r| HostPairRequest {
                    request_id: r.request_id.clone(),
                    name: r.name.clone(),
                    ip: r.ip.clone(),
                    code: r.code.clone(),
                    since: r.since,
                })
                .collect(),
            paired_devices: s
                .paired_devices
                .iter()
                .map(|d| HostPairedDevice {
                    device_id: d.device_id.clone(),
                    name: d.name.clone(),
                    created: d.created,
                    last_seen: d.last_seen,
                })
                .collect(),
            slots: s
                .slots
                .iter()
                .map(|x| HostSlot {
                    id: x.id.clone(),
                    name: x.name.clone(),
                    chain: x.chain.clone(),
                    saved_at: x.saved_at.clone(),
                    protocol: x.protocol,
                    footswitch: x.footswitch,
                })
                .collect(),
            enabled_slot: some(&s.enabled_slot),
        }
    }
}
