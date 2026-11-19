//! local_vectors.rs — the local engine and the remote forward against `fixtures/local-vectors.json`, the
//! committed reference: snapshots byte for byte from the supervisor's fixtures, and the rest.

use serde_json::{Map, Value};

fn vectors() -> Value {
    serde_json::from_str(include_str!("../../../fixtures/local-vectors.json")).unwrap()
}

fn cases<'a>(v: &'a Value, name: &str) -> &'a Vec<Value> {
    v[name].as_array().unwrap()
}

#[test]
fn snapshots_serialize_as_snapshot_of() {
    let v = vectors();
    for c in cases(&v, "snapshots") {
        let Some(k7_local::protocol::Frame::State(frame)) =
            k7_local::protocol::parse_frame(c["line"].as_str().unwrap())
        else {
            panic!("{} is a state frame", c["name"]);
        };
        let snapshot = Value::Object(k7_local::protocol::snapshot_of(&frame)).to_string();
        assert_eq!(snapshot, c["snapshot"].as_str().unwrap(), "{}", c["name"]);
    }
}

#[test]
fn commands_are_the_same_lines() {
    let v = vectors();
    for c in cases(&v, "commands") {
        let line = k7_local::protocol::command_line(
            c["id"].as_u64().unwrap(),
            c["cmd"].as_str().unwrap(),
            c["arg"].as_str(),
        );
        assert_eq!(line, c["line"].as_str().unwrap());
    }
}

#[test]
fn the_registry_is_pinned_the_same_way() {
    let v = vectors();
    for c in cases(&v, "pins") {
        let settings: &Map<String, Value> = c["settings"].as_object().unwrap();
        let out = k7_local::settings::pin_registry(settings, c["local"].as_str().unwrap())
            .map(|m| Value::Object(m).to_string());
        assert_eq!(out.as_deref(), c["out"].as_str(), "{c}");
    }
}
