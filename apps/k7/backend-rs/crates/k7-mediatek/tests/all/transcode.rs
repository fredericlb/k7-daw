//! The recorder's cuts and joins written by the waveform module in Wasmtime, held to the bytes, frames and gain
//! node's `transcode()` gave for the same pieces (`fixtures/transcode-vectors.json`, `makeTranscodeVectors.ts`).

use std::path::{Path, PathBuf};

use base64::Engine;
use k7_mediatek::transcode::{Loudness, Opus, Piece, transcode};
use k7_mediatek::waveform::Waveform;
use serde_json::Value;
use sha2::{Digest, Sha256};

const CLIP_LOUDNESS: Loudness = Loudness {
    lufs: -14.0,
    true_peak: -1.0,
};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("k7-transcode-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The vectors' sources written into `dir`.
fn sources(v: &Value, dir: &Path) {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../../e2e/specs/conformance/audio");
    for (name, b64) in v["sources"].as_object().unwrap() {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(b64.as_str().unwrap())
            .unwrap();
        std::fs::write(dir.join(name), bytes).unwrap();
    }
    for name in v["conformance"].as_array().unwrap() {
        let name = name.as_str().unwrap();
        std::fs::copy(root.join(name), dir.join(name)).unwrap();
    }
}

fn never(_: &Path) -> Option<String> {
    None
}

#[test]
fn writes_the_cuts_and_joins_node_writes() {
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/transcode-vectors.json");
    let v: Value = serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap();
    let dir = scratch("vectors");
    sources(&v, &dir);
    let waveform = Waveform::new(Some(&std::env::temp_dir().join("k7-stream-cache"))).unwrap();
    let cases = v["cases"].as_array().unwrap();
    assert!(cases.len() >= 9);
    for c in cases {
        let name = c["name"].as_str().unwrap();
        let pieces: Vec<Piece> = c["pieces"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| Piece {
                source: dir.join(p["source"].as_str().unwrap()),
                start: p["start"].as_f64(),
                duration: p["duration"].as_f64(),
            })
            .collect();
        let opus = Opus {
            bitrate: 128_000,
            channels: c["channels"].as_i64().unwrap() as i32,
        };
        let loudness = c["loudness"].as_bool().unwrap().then_some(&CLIP_LOUDNESS);
        let dest = dir.join(format!("{name}.webm"));
        let done = transcode(&waveform, &pieces, &dest, &opus, loudness, &never).unwrap();
        assert_eq!(done.frames, c["frames"].as_f64().unwrap(), "{name}");
        assert_eq!(done.gain_db, c["gainDb"].as_f64().unwrap(), "{name}");
        let webm = std::fs::read(&dest).unwrap();
        assert_eq!(webm.len() as u64, c["bytes"].as_u64().unwrap(), "{name}");
        let hex: String = Sha256::digest(&webm)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(hex, c["sha256"].as_str().unwrap(), "{name}");
    }
    let parts = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().ends_with(".part"))
        .count();
    assert_eq!(parts, 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_stop_between_blocks_leaves_nothing_and_says_why() {
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/transcode-vectors.json");
    let v: Value = serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap();
    let dir = scratch("stop");
    sources(&v, &dir);
    let waveform = Waveform::new(None).unwrap();
    let dest = dir.join("out.webm");
    let pieces = [Piece::whole(dir.join("silence.flac"))];
    let opus = Opus {
        bitrate: 128_000,
        channels: 2,
    };
    let stop = |src: &Path| Some(format!("transcoding {} was aborted", src.display()));
    let err = transcode(&waveform, &pieces, &dest, &opus, None, &stop).unwrap_err();
    assert!(err.ends_with("silence.flac was aborted"), "{err}");
    let broken = dir.join("broken.wav");
    std::fs::write(&broken, b"not audio").unwrap();
    let err = transcode(
        &waveform,
        &[Piece::whole(broken)],
        &dest,
        &opus,
        None,
        &never,
    )
    .unwrap_err();
    assert!(err.starts_with("cannot open "), "{err}");
    let join = [
        Piece::whole(dir.join("phone.wav")),
        Piece::whole(dir.join("phone.wav")),
    ];
    assert!(transcode(&waveform, &join, &dest, &opus, Some(&CLIP_LOUDNESS), &never).is_err());
    let left: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.file_name())
        .collect();
    assert!(
        !left
            .iter()
            .any(|n| n.to_string_lossy().ends_with(".part") || n == "out.webm"),
        "{left:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
