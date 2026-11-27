// tests_eyes.rs — the eyes.* ops through the bytes the ABI carries, and the grip drag's typed export.
// A reader would get wrong what is proved here: the rules are `eyes`'s own tests; these are the request shapes and the buffers.

use crate::ops::{ERROR, OK};
use crate::*;
use serde_json::{Value, json};

fn run(s: &mut Session, request: &Value) -> (u32, Value) {
    let mut out = Vec::new();
    let status = s.handle(request.to_string().as_bytes(), &mut out);
    (status, serde_json::from_slice(&out).unwrap())
}

#[test]
fn a_schedule_is_asked_for_by_clips_and_answers_camel_case() {
    let mut s = Session::new();
    let clips = json!([
        { "id": "a", "start": 0.0, "length": 5.0 },
        { "id": "b", "start": 4.0, "length": 5.0, "fadeIn": { "over": 1.0, "with": "dissolve" } },
    ]);
    let (status, v) = run(&mut s, &json!({ "op": "eyes.schedule", "args": { "clips": clips } }));
    assert_eq!(status, OK);
    assert_eq!(v["ends"], json!([5.0, 9.0]));
    let blend = &v["spans"][1]["blend"];
    assert_eq!(blend["name"], "dissolve");
    assert_eq!(blend["progress"], json!({ "start": 4.0, "len": 1.0, "rising": true }));
}

#[test]
fn the_builtins_are_the_dissolve_and_the_over() {
    let (status, v) = run(&mut Session::new(), &json!({ "op": "eyes.builtins", "args": {} }));
    assert_eq!(status, OK);
    let names: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(names, ["dissolve", "over"]);
    assert_eq!(v["dissolve"]["params"]["P4"], 0.0);
}

#[test]
fn a_lens_is_matched_and_a_missing_one_says_why() {
    let mut s = Session::new();
    let devices = json!([{ "deviceId": "a1", "kind": "videoinput", "label": "FaceTime HD Camera" }]);
    let ask = |label: &str| json!({ "op": "eyes.match_camera", "args": { "label": label, "devices": devices } });
    assert_eq!(run(&mut s, &ask("facetime")).1, json!({ "deviceId": "a1" }));
    let (_, miss) = run(&mut s, &ask("Blackmagic"));
    assert_eq!(miss["deviceId"], Value::Null);
    assert!(miss["problem"].as_str().unwrap().contains("FaceTime HD Camera"));
}

#[test]
fn a_cube_answers_its_size_and_leaves_its_floats_in_the_buffer() {
    let mut s = Session::new();
    let text = "LUT_3D_SIZE 2\n0 0 0\n1 0 0\n0 1 0\n1 1 0\n0 0 1\n1 0 1\n0 1 1\n1 1 1\n";
    let (status, v) = run(&mut s, &json!({ "op": "eyes.cube", "args": { "text": text } }));
    assert_eq!((status, v), (OK, json!({ "size": 2 })));
    assert_eq!(s.floats.len(), 32);
    assert_eq!(&s.floats[4..8], &[1.0, 0.0, 0.0, 1.0]);
    let (status, e) = run(&mut s, &json!({ "op": "eyes.cube", "args": { "text": "0 0 0" } }));
    assert_eq!(status, ERROR);
    assert_eq!(e["code"], "bad_request");
}

#[test]
fn the_grip_drag_is_a_typed_export_over_the_scalars() {
    use crate::abi::{NONE, k7_drag_rect, k7_scalars_ptr};
    let read = || unsafe { std::slice::from_raw_parts(k7_scalars_ptr(), 4).to_vec() };
    assert_eq!(k7_drag_rect(100.0, 100.0, 200.0, 200.0, 4, 50.0, 0.0), OK);
    assert_eq!(read(), [150.0, 100.0, 150.0, 200.0]);
    assert_eq!(k7_drag_rect(100.0, 100.0, 200.0, 200.0, 99, 0.0, 0.0), NONE);
}
