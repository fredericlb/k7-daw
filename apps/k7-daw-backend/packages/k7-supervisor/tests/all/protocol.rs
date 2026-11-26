//! protocol.rs — the frames an app parses: the snapshot is the schema's fixtures byte for byte, the optional
//! keys are absent rather than empty, and a line survives as a command only when it is one.

use k7_supervisor::protocol::*;
use serde_json::Value;

fn fixture(name: &str) -> String {
    let dir = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../k7-daw-engine/k7-supervisor/schema/fixtures"
    );
    let text = std::fs::read_to_string(format!("{dir}/{name}.json")).expect("a fixture");
    serde_json::from_str::<Value>(&text)
        .expect("JSON")
        .to_string()
}

fn full() -> Snapshot {
    Snapshot {
        status: "error".into(),
        error: "engine binary not found (build it)".into(),
        safe_mode: Some(SafeMode {
            crashes: 3,
            suspect: "Diva".into(),
        }),
        owns_engine: true,
        lan_open: true,
        engine_id: "e1".into(),
        engine_port: 8766,
        registry_url: "https://app.k7.rocks".into(),
        last_heartbeat_ok: true,
        plugin_count: Some(1),
        plugins: vec![Plugin {
            name: "Diva".into(),
            format: "VST3".into(),
            manufacturer: "u-he".into(),
            role: "instrument".into(),
        }],
        plugins_sig: "p1".into(),
        audio_devices: vec![
            AudioDevice {
                name: "Focusrite USB ASIO".into(),
                device_type: "ASIO".into(),
                ..Default::default()
            },
            AudioDevice {
                name: "Speakers (Focusrite USB Audio)".into(),
                device_type: "Windows Audio".into(),
                current: true,
                current_input: true,
                inputs: 2,
                outputs: 2,
            },
        ],
        audio_priority: vec![
            "Focusrite USB ASIO".into(),
            "Speakers (Focusrite USB Audio)".into(),
        ],
        audio_menu_sig: "a1".into(),
        edit_plugins: vec![EditPlugin {
            session: "s1".into(),
            id: "n4".into(),
            name: "Diva".into(),
            has_native_plugin_ui: true,
        }],
        edit_plugins_sig: "x1".into(),
        sessions: vec![Session {
            id: "s1".into(),
            project: "@local/w/song".into(),
            loaded: true,
            playing: true,
            tracks: 4,
            listeners: 2,
        }],
        sessions_sig: "s1".into(),
        instances: vec![Instance {
            url: "https://app.k7.rocks".into(),
            removable: true,
        }],
        instance_check: Some(InstanceCheck {
            url: "https://play.st0.app".into(),
            state: "refused".into(),
            message: "certificate not trusted".into(),
        }),
        root_path: "/home/u/.k7".into(),
        merged_log_path: "/home/u/.k7/logs/merged.log".into(),
        downloads: vec![Download {
            name: "nbo-wurt".into(),
            label: "Wurlitzer".into(),
            received: 1024,
            total: 7947927,
            phase: "failed".into(),
            error: "disk full".into(),
        }],
        instruments: vec![InstrumentRow {
            name: "nbo-wurt".into(),
            label: "Wurlitzer".into(),
            bytes: 7947927,
            wanted: true,
            installed: false,
        }],
        pair_requests: vec![PairRequest {
            request_id: "r1".into(),
            name: "iPhone · Safari".into(),
            ip: "192.168.1.20".into(),
            code: "482913".into(),
            since: 1760000000000,
        }],
        paired_devices: vec![PairedDevice {
            device_id: "d1".into(),
            name: "iPad".into(),
            created: 1760000000000,
            last_seen: 1760000600000,
        }],
        slots: vec![SlotRow {
            id: "warm".into(),
            name: "Warm".into(),
            chain: "@account/warm".into(),
            saved_at: "2026-11-01T18:00:00Z".into(),
            protocol: 7,
            footswitch: Some(64),
        }],
        enabled_slot: "warm".into(),
        ..Snapshot::default()
    }
}

#[test]
fn the_snapshots_are_the_fixtures_byte_for_byte() {
    assert_eq!(
        state_frame(&Snapshot::default()).to_string(),
        fixture("state-empty")
    );
    assert_eq!(state_frame(&full()).to_string(), fixture("state-full"));
    assert_eq!(result_frame(9, false, "no").to_string(), fixture("result"));
}

#[test]
fn the_optional_keys_are_absent_and_a_frame_is_one_line() {
    let s = Snapshot {
        instance_check: Some(InstanceCheck {
            url: "u".into(),
            state: "checking".into(),
            message: String::new(),
        }),
        ..Snapshot::default()
    };
    let f = state_frame(&s);
    assert!(
        f.get("error").is_none() && f.get("downloads").is_none() && f.get("rendering").is_none()
    );
    assert!(f["pluginCount"].is_null() && f["instanceCheck"].get("message").is_none());
    assert_eq!(f["instruments"], Value::Array(vec![]));
    assert!(!f.to_string().contains('\n'));
    assert!(result_frame(1, true, "").get("message").is_none());
    let busy = state_frame(&Snapshot {
        rendering: 2,
        ..Snapshot::default()
    });
    assert_eq!(busy["rendering"], 2);
}

#[test]
fn a_command_is_parsed_and_anything_else_is_not() {
    let c = parse_command(r#"{"t":"cmd","id":4,"cmd":"audio.priority","arg":"A\u001fB"}"#)
        .expect("a command");
    assert_eq!((c.id, c.cmd.as_str()), (4, "audio.priority"));
    assert_eq!(split_units(&c.arg), ["A", "B"]);
    for no in [
        "",
        "   ",
        r#"{"t":"state"}"#,
        "{not json",
        r#"{"t":"cmd","cmd":3}"#,
    ] {
        assert!(parse_command(no).is_none(), "{no}");
    }
    assert!(split_units("").is_empty());
}
