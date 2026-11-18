//! The engine core held to `fixtures/engine-vectors.json`, the committed reference: a heartbeat read and
//! cleaned in the same words, share lists and roles, a job respelt, the frames a slow peer may lose.

use k7_engine::frame::Frame;
use k7_engine::registry::heartbeat::read_heartbeat;
use k7_engine::registry::owners::{Ownership, clean_shares, role_of};
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
fn shares_and_roles_as_the_typescript_reads_them() {
    let v = vectors();
    for s in v["shares"].as_array().unwrap() {
        let raw: Vec<String> = serde_json::from_value(s["raw"].clone()).unwrap();
        let got = match clean_shares(&raw) {
            Ok(emails) => json!({ "ok": true, "emails": emails }),
            Err(bad) => json!({ "ok": false, "bad": bad }),
        };
        assert_eq!(got, s["out"], "{raw:?}");
    }
    for r in v["roles"].as_array().unwrap() {
        let o = (!r["o"].is_null()).then(|| Ownership {
            owner: r["o"]["owner"].as_str().unwrap().into(),
            shared: serde_json::from_value(r["o"]["shared"].clone()).unwrap(),
            everyone: r["o"]["everyone"].as_bool().unwrap(),
            since: 0.0,
        });
        let got = role_of(
            o.as_ref(),
            r["holder"].as_str().unwrap(),
            r["server"].as_bool().unwrap(),
        );
        assert_eq!(got.as_str(), r["out"], "{r}");
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
