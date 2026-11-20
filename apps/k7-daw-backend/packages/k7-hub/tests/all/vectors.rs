//! vectors.rs — the hub held to `fixtures/hub-vectors.json`, the reference, as committed: tickets minted byte for byte and read alike, the allow-list and the sync target.

use std::collections::BTreeMap;

use serde_json::Value;

fn vectors() -> Value {
    let file = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/hub-vectors.json"
    );
    serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap()
}

fn s(v: &Value) -> &str {
    v.as_str().unwrap()
}

#[test]
fn tickets_minted_and_read_as_the_typescript_does() {
    let v = vectors();
    for c in v["tickets"].as_array().unwrap() {
        let minted = k7_hub::ticket::mint(
            s(&c["key"]).as_bytes(),
            s(&c["user"]),
            c["now"].as_f64().unwrap(),
            c["ttl"].as_f64().unwrap(),
        );
        assert_eq!(minted, s(&c["ticket"]), "{c}");
    }
    for c in v["reads"].as_array().unwrap() {
        let read = k7_hub::ticket::read(
            s(&c["key"]).as_bytes(),
            s(&c["presented"]),
            c["now"].as_f64().unwrap(),
        );
        let want = &c["read"];
        match read {
            None => assert!(want.is_null(), "{c}"),
            Some(t) => {
                assert_eq!(t.user, s(&want["user"]), "{c}");
                assert_eq!(t.engine_id.as_deref(), want["engineId"].as_str(), "{c}");
            }
        }
    }
}

#[test]
fn the_services_environment() {
    let v = vectors();
    let parent: Vec<(String, String)> = v["serviceEnv"]["parent"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| (k.clone(), s(v).to_string()))
        .collect();
    let got = k7_hub::env::service_env(parent.iter().map(|(k, v)| (k.as_str(), v.as_str())));
    let want: BTreeMap<String, String> = v["serviceEnv"]["env"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| (k.clone(), s(v).to_string()))
        .collect();
    assert_eq!(got, want);
}
