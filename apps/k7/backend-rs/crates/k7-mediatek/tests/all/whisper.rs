//! Whisper's pure half replayed over what the TypeScript answered (`makeHostVectors.mts`): both CLIs, the audio extract, the SRT read and written.

use std::path::Path;

use k7_mediatek::whisper::{
    BINS, Segment, cli_args, extract_audio_args, parse_srt, srt_of, srt_path,
};
use serde_json::Value;

fn vectors() -> Value {
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/vectors/host-whisper.json");
    serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap()
}

fn segments(v: &Value) -> Vec<Segment> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|s| Segment {
            start: s["start"].as_f64().unwrap(),
            end: s["end"].as_f64().unwrap(),
            text: s["text"].as_str().unwrap().to_string(),
        })
        .collect()
}

#[test]
fn the_clis_are_tried_in_the_typescripts_order_with_its_arguments_and_files() {
    let v = vectors();
    let order: Vec<&str> = v["order"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b.as_str().unwrap())
        .collect();
    assert_eq!(order, BINS);
    let cases = v["cli"].as_array().unwrap();
    assert!(cases.len() >= 18);
    for c in cases {
        let (bin, wav, dir) = (
            c["bin"].as_str().unwrap(),
            c["wav"].as_str().unwrap(),
            c["outDir"].as_str().unwrap(),
        );
        let model = c["model"].as_str();
        let got = cli_args(bin, wav, dir, model);
        match c["error"].as_str() {
            Some(e) => assert_eq!(got, Err(e.to_string()), "{bin} {model:?}"),
            None => {
                let want: Vec<String> = serde_json::from_value(c["args"].clone()).unwrap();
                assert_eq!(got, Ok(want), "{bin} {wav} {model:?}");
            }
        }
        assert_eq!(
            srt_path(bin, wav, dir),
            c["srt"].as_str().unwrap(),
            "{bin} {wav}"
        );
    }
}

#[test]
fn the_audio_extract_asks_for_sixteen_kilohertz_mono_under_the_whitelist() {
    for c in vectors()["extractAudio"].as_array().unwrap() {
        let want: Vec<String> = serde_json::from_value(c["args"].clone()).unwrap();
        assert_eq!(
            extract_audio_args(c["video"].as_str().unwrap(), c["wav"].as_str().unwrap()),
            want
        );
    }
}

#[test]
fn an_srt_is_read_into_the_typescripts_segments() {
    let v = vectors();
    let cases = v["parse"].as_array().unwrap();
    assert!(cases.len() >= 10);
    for c in cases {
        let want = segments(&c["segments"]);
        let got = parse_srt(c["text"].as_str().unwrap());
        assert_eq!(got.len(), want.len(), "{}", c["name"]);
        for (g, w) in got.iter().zip(&want) {
            assert!(
                (g.start - w.start).abs() < 1e-9 && (g.end - w.end).abs() < 1e-9,
                "{}",
                c["name"]
            );
            assert_eq!(g.text, w.text, "{}", c["name"]);
        }
    }
}

#[test]
fn segments_are_written_as_the_typescripts_srt_with_its_rounding() {
    let v = vectors();
    for c in v["write"].as_array().unwrap() {
        assert_eq!(
            srt_of(&segments(&c["segments"])),
            c["srt"].as_str().unwrap(),
            "{}",
            c["name"]
        );
    }
}
