//! catalogue.rs — the engine's verbs and replies by name, and the sets the socket's rules are written over:
//! which queries a probe fires, which name another session, which may be superseded, which replies are stamped.
//! What a reader gets wrong: `ENGINE_GLOBAL_REPLIES` and `PER_SESSION_REPLIES` are engine-protocol's `sessions.ts`,
//! held to it by `tests/all/catalogue.rs`, and a client's filter is safe only while every other reply is stamped.

/// Every verb a client sends, the pairing ones included; each later milestone answers its share.
pub const CLIENT_VERBS: &[&str] = &[
    // a project: ServeProject.cpp
    "project",
    "tracks",
    "flush",
    "transport",
    "preview",
    "note-preview",
    "midi-live",
    "arm",
    "recorder",
    "record-enable",
    // plugins: ServePlugins.cpp
    "plugin-scan",
    "list-plugins",
    "rescan-plugins",
    "list-plugin-params",
    "list-edit-plugins",
    "show-native-plugin-ui",
    "show-native-plugin-ui-of",
    "snapshot-plugin-base",
    "set-plugin-param",
    "get-plugin-params",
    "list-presets",
    "set-preset",
    "watch-plugin-params",
    "subscribe-waveform",
    "unsubscribe-waveform",
    // the machine: ServeMachine.cpp
    "broadcast",
    "list-sessions",
    "drop-session",
    "audio-cost",
    "list-audio-devices",
    "io-levels",
    "set-input-stereo",
    "set-audio-device",
    "instrument-status",
    "whoami",
    "hello",
    // a bounce: ServeRender.cpp
    "render",
    // pairing: ServePairing.cpp
    "pair-request",
    "pair",
    "list-pair-requests",
    "list-paired-devices",
    "pair-approve",
    "pair-deny",
    "unpair-device",
    "lan-open",
];

/// Read-only queries a short-lived probe fires: they create no session and earn no connect line.
pub const PROBE_QUERIES: &[&str] = &[
    "whoami",
    "list-audio-devices",
    "list-plugins",
    "list-edit-plugins",
    "list-sessions",
    "io-levels",
    "instrument-status",
];

/// Verbs whose `session` names ANOTHER project than the sender's, or none, so they never bind the socket's.
pub const OVERRIDE_QUERIES: &[&str] = &[
    "drop-session",
    "show-native-plugin-ui",
    "show-native-plugin-ui-of",
    "render",
];

/// The verbs a newer one of the same session may skip while still queued (k7-runtime's docs/supersede.md).
pub const SUPERSEDABLE: &[&str] = &["project", "tracks", "set-plugin-param"];

/// The pairing verbs only the machine's own clients may send; a LAN peer is refused with `local-only`.
pub const LOCAL_ONLY: &[&str] = &[
    "pair-approve",
    "pair-deny",
    "list-pair-requests",
    "list-paired-devices",
    "unpair-device",
    "lan-open",
];

/// Replies about the machine, never stamped with a session.
pub const ENGINE_GLOBAL_REPLIES: &[&str] = &[
    "plugins",
    "plugin-scan",
    "audio-devices",
    "sessions",
    "edit-plugins",
    "whoami",
    "rendered",
    "plugin-param-list",
    "native-plugin-ui-opened",
    "plugin-base-saved",
    "pair-pending",
    "paired",
    "pair-denied",
    "pair-requests",
    "paired-devices",
];

/// Replies about one project, always stamped with its session.
pub const PER_SESSION_REPLIES: &[&str] = &[
    "state",
    "busy",
    "applied",
    "flushed",
    "error",
    "armed",
    "plugin-params",
    "plugin-params-live",
    "presets",
    "io-levels",
    "recorded",
    "engine-warning",
];

/// Whether `verb` is in `set`.
pub fn is(set: &[&str], verb: &str) -> bool {
    set.contains(&verb)
}
