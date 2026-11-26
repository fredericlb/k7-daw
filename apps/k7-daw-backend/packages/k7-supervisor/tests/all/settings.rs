//! settings.rs — the settings file is hand-edited, so each field stands alone and a round trip gives back what
//! went in; a config file is read only, its `--port` fixed, and its environment keys handed back.

use k7_supervisor::settings::{Settings, Store, config_environment};
use serde_json::json;

use crate::scratch::Scratch;

#[test]
fn one_bad_key_loses_nothing_else() {
    let s = Settings::from_json(
        &json!({"port": "nope", "audioPriority": ["Scarlett", "Scarlett"], "registryUrls": 7}),
    );
    assert_eq!(s.port, 8765);
    assert_eq!(s.audio_priority, ["Scarlett"]);
    assert!(s.registry_urls.is_empty());
    assert_eq!(Settings::from_json(&json!({"port": 70000})).port, 8765);
    assert_eq!(Settings::from_json(&json!([1])), Settings::default());
}

#[test]
fn providers_are_lowercased_and_deduplicated() {
    let s = Settings::from_json(&json!({"onnxProviders": [" DML ", "cpu", "dml", ""]}));
    assert_eq!(s.onnx_providers, ["dml", "cpu"]);
}

#[test]
fn a_round_trip_is_exact_and_absent_instruments_differ_from_none() {
    let mut s = Settings {
        registry_urls: vec!["https://play.st0.app".into()],
        port: 9001,
        audio_priority: vec!["Scarlett".into(), "Built-in".into()],
        onnx_providers: vec!["coreml".into(), "cpu".into()],
        instruments: Some(vec![]),
        lan_open: true,
        stems_chain: "@k7-core/stems".into(),
        ..Settings::default()
    };
    s.device_tokens
        .insert("https://play.st0.app".into(), "t1".into());
    assert_eq!(Settings::from_json(&s.to_json()), s);
    assert!(Settings::from_json(&json!({})).instruments.is_none());
    assert!(Settings::default().to_json().get("instruments").is_none());
    assert!(!Settings::default().lan_open);
}

#[test]
fn a_config_files_one_token_is_its_own_studios() {
    let s = Settings::from_json(&json!({
        "registryUrls": ["https://a.example", "https://b.example"], "deviceToken": " t ",
        "deviceTokens": {"https://b.example": "own"}
    }));
    assert_eq!(s.device_tokens["https://a.example"], "t");
    assert_eq!(s.device_tokens["https://b.example"], "own");
    let env = config_environment(
        &json!({"machineName": " Pi ", "extraCaCerts": "", "instruments": ["all"]}),
    );
    assert_eq!(
        env,
        [
            ("STUDIO_MACHINE_NAME".to_string(), "Pi".to_string()),
            ("STUDIO_INSTRUMENTS".into(), "all".into())
        ]
    );
}

#[test]
fn a_config_file_is_never_written_and_its_port_is_fixed() {
    let dir = Scratch::new("settings");
    let file = dir.path().join("engine.json");
    std::fs::write(&file, r#"{"port": 9100}"#).unwrap();
    let store = Store {
        file: file.clone(),
        read_only: true,
        fixed_port: Some(9200),
    };
    let s = store.load();
    assert_eq!((s.port, s.port_fixed), (9200, true));
    store.save(&Settings::default());
    assert_eq!(std::fs::read_to_string(&file).unwrap(), r#"{"port": 9100}"#);
    let own = Store::new(dir.path().join("engine-host.json"));
    own.save(&Settings {
        port: 9300,
        port_fixed: true,
        ..Settings::default()
    });
    let back = own.load();
    assert_eq!((back.port, back.port_fixed), (9300, false));
}
