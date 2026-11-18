//! The Opus copy of a FLAC written by the waveform module in Wasmtime, held to the bytes node's `ensureStreamCopy`
//! wrote for the same FLAC (`fixtures/stream-vectors.json`, kept as committed). The ignored test takes a
//! longer set of the same shape (`K7_STREAM_VECTORS`).

use std::path::{Path, PathBuf};

use base64::Engine;
use k7_mediatek::stream_copy::{copy_is_fresh, copy_path, make_copy};
use k7_mediatek::waveform::Waveform;
use serde_json::Value;
use sha2::{Digest, Sha256};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("k7-stream-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Every copy of the vectors at `file` made again here, its bytes hashed as node hashed them.
fn holds(file: &Path) {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap();
    let dir = scratch("vectors");
    let waveform = Waveform::new(Some(&std::env::temp_dir().join("k7-stream-cache"))).unwrap();
    let copies = v["copies"].as_array().unwrap();
    assert!(copies.len() >= 7);
    for c in copies {
        let name = c["name"].as_str().unwrap();
        let flac = dir.join(format!("{name}.flac"));
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(c["flac"].as_str().unwrap())
            .unwrap();
        std::fs::write(&flac, bytes).unwrap();
        assert!(!copy_is_fresh(&flac), "{name}");
        let copy = make_copy(&waveform, &flac).unwrap();
        assert_eq!(copy, copy_path(&flac).unwrap());
        let webm = std::fs::read(&copy).unwrap();
        assert_eq!(webm.len() as u64, c["bytes"].as_u64().unwrap(), "{name}");
        let hex: String = Sha256::digest(&webm)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        assert_eq!(hex, c["sha256"].as_str().unwrap(), "{name}");
        assert!(copy_is_fresh(&flac), "{name}");
    }
    // No part file is left beside a finished copy.
    let parts = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().ends_with(".part"))
        .count();
    assert_eq!(parts, 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn writes_the_copy_node_writes() {
    holds(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/stream-vectors.json"));
}

#[test]
#[ignore = "takes K7_STREAM_VECTORS, a longer set from makeStreamVectors.ts <out> <scale>"]
fn writes_the_copy_node_writes_for_longer_files() {
    holds(Path::new(
        &std::env::var("K7_STREAM_VECTORS").expect("K7_STREAM_VECTORS"),
    ));
}

#[test]
fn refuses_a_file_it_cannot_decode_and_leaves_nothing() {
    let dir = scratch("refused");
    let flac = dir.join("broken.flac");
    std::fs::write(&flac, b"not a flac at all").unwrap();
    let waveform = Waveform::new(None).unwrap();
    assert!(make_copy(&waveform, &flac).is_err());
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
    assert!(make_copy(&waveform, &dir.join("take.wav")).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}
