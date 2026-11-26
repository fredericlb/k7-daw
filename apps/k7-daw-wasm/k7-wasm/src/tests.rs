// tests.rs — the dispatch and the handle table, through the same bytes the ABI carries.

use crate::ops::curve::{CloseArgs, Head, OpenArgs};
use crate::ops::{ERROR, OK};
use crate::*;
use serde_json::{Value, json};

const SAW: &str = r#"{"type":"lfo","polarity":"unipolar","amplitude":1,"shape":"saw","frequency":1,"phase":0}"#;

fn run(session: &mut Session, request: &Value) -> (u32, Value) {
    let mut out = Vec::new();
    let status = session.handle(request.to_string().as_bytes(), &mut out);
    (status, serde_json::from_slice(&out).unwrap())
}

fn saw() -> Value {
    serde_json::from_str(SAW).unwrap()
}

fn open_args() -> OpenArgs {
    serde_json::from_value(json!({"spec": saw(), "num": 4, "den": 4})).unwrap()
}

#[test]
fn previews_a_spec_in_one_call() {
    let mut s = Session::new();
    let (status, v) = run(
        &mut s,
        &json!({"op": "curve.preview", "args": {"spec": saw(), "num": 4, "den": 4, "n": 10, "playhead": 10}}),
    );
    assert_eq!(status, OK);
    assert_eq!(v["span"], json!({"beats": 5.0, "solid": 4.0}));
    assert_eq!(v["beats"].as_array().unwrap().len(), 6);
    assert!((v["playhead"]["x"].as_f64().unwrap() - 0.4).abs() < 1e-9);
}

#[test]
fn leaves_the_playhead_null_when_not_asked() {
    let mut s = Session::new();
    let (_, v) = run(
        &mut s,
        &json!({"op": "curve.preview", "args": {"spec": saw(), "num": 4, "den": 4}}),
    );
    assert_eq!(v["playhead"], Value::Null);
    assert_eq!(
        v["solid"].as_array().unwrap().len() + v["faded"].as_array().unwrap().len(),
        162
    );
}

#[test]
fn opens_answers_a_preview_and_closes() {
    let mut s = Session::new();
    let (status, v) = run(
        &mut s,
        &json!({"op": "curve.open", "args": {"spec": saw(), "num": 4, "den": 4}}),
    );
    assert_eq!(status, OK);
    let handle = v["handle"].as_u64().unwrap();
    assert!(matches!(s.curves.playhead(handle as u32, 10.0), Head::At(_)));
    let (status, v) = run(&mut s, &json!({"op": "curve.close", "args": {"handle": handle}}));
    assert_eq!((status, v), (OK, Value::Null));
    assert_eq!(s.curves.playhead(handle as u32, 10.0), Head::Unknown);
    let (status, v) = run(&mut s, &json!({"op": "curve.close", "args": {"handle": handle}}));
    assert_eq!((status, &v["code"]), (ERROR, &json!("unknown_handle")));
}

#[test]
fn an_envelope_and_a_negative_beat_have_no_playhead() {
    let mut s = Session::new();
    let adsr = json!({"type":"adsr","polarity":"unipolar","amplitude":1,"attack":1,"decay":1,"sustain":1,"release":1,"level":1});
    let a = s
        .curves
        .open(serde_json::from_value(json!({"spec": adsr, "num": 4, "den": 4})).unwrap())
        .unwrap();
    assert_eq!(s.curves.playhead(a.handle, 1.0), Head::Nothing);
    let b = s.curves.open(open_args()).unwrap();
    assert_eq!(s.curves.playhead(b.handle, -1.0), Head::Nothing);
    assert_eq!(s.curves.playhead(b.handle, f64::NAN), Head::Nothing);
}

#[test]
fn evicts_the_least_recently_used_curve_when_the_table_is_full() {
    let mut s = Session::new();
    let mut handles: Vec<u32> = (0..256).map(|_| s.curves.open(open_args()).unwrap().handle).collect();
    // Using the first makes the second the oldest.
    assert!(matches!(s.curves.playhead(handles[0], 1.0), Head::At(_)));
    handles.push(s.curves.open(open_args()).unwrap().handle);
    let unknown: Vec<usize> = (0..handles.len())
        .filter(|i| s.curves.playhead(handles[*i], 1.0) == Head::Unknown)
        .collect();
    assert_eq!(unknown, [1]);
}

#[test]
fn a_reused_slot_does_not_answer_to_the_old_handle() {
    let mut s = Session::new();
    let old = s.curves.open(open_args()).unwrap().handle;
    s.curves.close(CloseArgs { handle: old }).unwrap();
    let new = s.curves.open(open_args()).unwrap().handle;
    assert_ne!(old, new);
    assert_eq!(s.curves.playhead(old, 1.0), Head::Unknown);
    assert!(matches!(s.curves.playhead(new, 1.0), Head::At(_)));
}

#[test]
fn refuses_what_a_caller_got_wrong() {
    let mut s = Session::new();
    let code = |s: &mut Session, req: Value| {
        let (status, v) = run(s, &req);
        assert_eq!(status, ERROR, "{req}");
        v["code"].as_str().unwrap().to_string()
    };
    let at = |extra: Value| {
        let mut args = json!({"spec": saw(), "num": 4, "den": 4});
        args.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        json!({"op": "curve.preview", "args": args})
    };
    assert_eq!(code(&mut s, at(json!({"n": 0}))), "bad_request");
    assert_eq!(code(&mut s, at(json!({"n": 4097}))), "bad_request");
    assert_eq!(code(&mut s, at(json!({"den": 0}))), "bad_request");
    assert_eq!(code(&mut s, at(json!({"num": 0}))), "bad_request");
    assert_eq!(code(&mut s, at(json!({"num": 4.5}))), "bad_request");
    assert_eq!(code(&mut s, json!({"op": "curve.nope", "args": {}})), "bad_request");
    assert_eq!(code(&mut s, json!({"op": "nope.preview", "args": {}})), "bad_request");
    assert_eq!(code(&mut s, json!({"op": "edit.call", "args": {}})), "bad_request");
    assert_eq!(code(&mut s, json!({"args": {}})), "bad_request");
    assert_eq!(
        code(
            &mut s,
            json!({"op": "timecode.parse", "args": {"tc": "x", "num": 4, "den": 4, "position": true}})
        ),
        "bad_timecode"
    );
    let (_, v) = run(
        &mut s,
        &json!({"op": "timecode.parse", "args": {"tc": "x", "num": 4, "den": 4, "position": true}}),
    );
    assert_eq!(
        v["message"],
        "bad timecode 'x' — expected \"mesure:noire:croche:doublecroche\", optionally \".ticks\""
    );
}

#[test]
fn garbage_and_empty_requests_are_bad_requests() {
    let mut s = Session::new();
    let mut out = Vec::new();
    assert_eq!(s.handle(b"not json", &mut out), ERROR);
    assert_eq!(s.handle(b"", &mut out), ERROR);
    assert_eq!(s.handle(&[0xff, 0xfe], &mut out), ERROR);
}

#[test]
fn parses_and_formats_timecodes() {
    let mut s = Session::new();
    let (status, v) = run(
        &mut s,
        &json!({"op": "timecode.parse", "args": {"tc": "2:1", "num": 4, "den": 4, "position": true}}),
    );
    assert_eq!((status, v), (OK, json!(4.0)));
    let (_, v) = run(
        &mut s,
        &json!({"op": "timecode.format", "args": {"beats": 4, "num": 4, "den": 4, "position": true}}),
    );
    assert_eq!(v, json!("2:1"));
    let (status, v) = run(
        &mut s,
        &json!({"op": "timecode.format", "args": {"beats": 1, "num": 4, "den": 4, "position": false}}),
    );
    assert_eq!((status, v), (OK, json!("0:1")));
}

#[test]
fn hashes_a_text_as_the_file_service_does() {
    let mut s = Session::new();
    let (status, v) = run(&mut s, &json!({"op": "text.hash", "args": {"text": "tempo(120)\n"}}));
    assert_eq!(status, OK);
    assert_eq!(v.as_str().map(str::len), Some(14));
    let (_, again) = run(&mut s, &json!({"op": "text.hash", "args": {"text": "tempo(120)\n"}}));
    assert_eq!(v, again);
}

#[test]
fn scores_a_project_against_a_catalogue() {
    let mut s = Session::new();
    let needs = json!([{"format": "vst3", "name": "Serum"}, {"format": "vst3", "name": "Diva"}]);
    let catalogue = json!([{"format": "VST3", "name": "Serum"}]);
    let (_, v) = run(
        &mut s,
        &json!({"op": "engine.compat", "args": {"needs": needs, "catalogue": catalogue}}),
    );
    assert_eq!(v, json!({"score": 50, "missing": [{"format": "vst3", "name": "Diva"}]}));
}

#[test]
fn bands_a_link_and_reads_a_warning() {
    let mut s = Session::new();
    let (_, v) = run(
        &mut s,
        &json!({"op": "engine.latency", "args": {"networkMs": 2, "bufferFrames": 256, "sampleRate": 48000}}),
    );
    assert_eq!(v["quality"], "Q1");
    let (_, v) = run(
        &mut s,
        &json!({"op": "engine.warning", "args": {"message": "clip source not found/invalid: /a/take.wav"}}),
    );
    assert_eq!(v["kind"], json!({"kind": "clipMissing", "file": "take.wav"}));
    assert_eq!(
        v["missing"],
        json!({"what": "clip", "name": "take.wav", "why": "not-found"})
    );
}

#[test]
fn picks_an_engine_by_index() {
    let mut s = Session::new();
    let one = |id: &str, here: bool| {
        json!({"id": id, "reachable": true, "direct": true, "here": here, "assigned": false,
               "latency": {"quality": "Q1", "totalMs": 5}})
    };
    let args = json!({"candidates": [one("a", false), one("b", true)]});
    let (_, v) = run(&mut s, &json!({"op": "engine.best", "args": args}));
    assert_eq!(v, json!({"index": 1}));
    let (_, v) = run(&mut s, &json!({"op": "engine.choose", "args": args}));
    assert_eq!(v, json!({"index": null, "ambiguous": true}));
}

#[test]
fn reads_and_writes_timecodes_on_a_float_meter() {
    let mut s = Session::new();
    let (status, v) = run(
        &mut s,
        &json!({"op": "timecode.parse", "args": {"tc": "1:0", "num": 7, "den": 8, "position": false}}),
    );
    assert_eq!((status, v), (OK, json!(3.5)));
    let (status, v) = run(
        &mut s,
        &json!({"op": "timecode.format", "args": {"beats": 3.5, "num": 7.0, "den": 8.0, "position": true}}),
    );
    assert_eq!((status, v), (OK, json!("2:1")));
    let (status, v) = run(
        &mut s,
        &json!({"op": "timecode.format", "args": {"beats": 1e300, "num": 4, "den": 4, "position": true}}),
    );
    assert_eq!((status, &v["code"]), (ERROR, &json!("bad_timecode")));
}

#[test]
fn answers_the_version_it_was_built_from() {
    let mut s = Session::new();
    let (status, v) = run(&mut s, &json!({"op": "version"}));
    assert_eq!(status, OK);
    assert!(v["sources"].is_string());
}

#[test]
fn rounds_a_beat_through_the_hot_call() {
    assert_eq!(crate::abi::k7_round_beat(0.1 + 0.2), 0.3);
    assert_eq!(crate::abi::k7_round_beat(-2.5), -2.5);
}

#[test]
fn a_song_op_reaches_the_song_family() {
    let mut s = Session::new();
    let (status, v) = run(&mut s, &json!({"op": "song.close", "args": {"handle": 9}}));
    assert_eq!((status, &v["code"]), (ERROR, &json!("unknown_handle")));
}
