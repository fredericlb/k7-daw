//! The engine core held to `fixtures/engine-vectors.json`, the committed reference: a heartbeat read and
//! cleaned in the same words, a job respelt, the frames a slow peer may lose.

use k7_engine::frame::Frame;
use k7_engine::registry::heartbeat::read_heartbeat;
use k7_engine::tunnel::backpressure::is_droppable;
use k7_engine::tunnel::frames::Root;
use k7_engine::tunnel::spelling::tunnel_spelling;
use serde_json::{Value, json};

fn vectors() -> Value {
    let file = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/engine-vectors.json"
    );
    serde_json::from_str(&std::fs::read_to_string(file).expect("the vectors")).expect("JSON")
}

#[test]
fn heartbeats_read_as_the_typescript_reads_them() {
    for v in vectors()["heartbeats"].as_array().unwrap() {
        let got = match read_heartbeat(&v["input"]) {
            Ok(b) => json!({ "ok": true, "beat": b.json() }),
            Err(e) => json!({ "ok": false, "error": e }),
        };
        assert_eq!(got, v["out"], "{}", v["input"]);
    }
}

#[test]
fn jobs_are_respelt_and_frames_dropped_as_the_typescript_does() {
    let v = vectors();
    let roots: Vec<Root> = serde_json::from_value(v["roots"].clone()).unwrap();
    for s in v["spellings"].as_array().unwrap() {
        assert_eq!(
            tunnel_spelling(s["text"].as_str().unwrap(), &roots, '\\'),
            s["out"].as_str().unwrap()
        );
    }
    for f in v["droppable"].as_array().unwrap() {
        let data = f["data"].as_str().unwrap();
        let frame = if f["binary"].as_bool().unwrap() {
            let bytes: Vec<u8> = (0..data.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&data[i..i + 2], 16).unwrap())
                .collect();
            Frame::binary(bytes)
        } else {
            Frame::text(data)
        };
        assert_eq!(is_droppable(&frame), f["out"].as_bool().unwrap(), "{f}");
    }
}
