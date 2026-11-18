//! The copies' pure half replayed over what the TypeScript answered (`makeHostVectors.mts`): ffmpeg's arguments and where each copy lands.

use std::path::Path;

use k7_mediatek::video_copy::{CopyKind, PROXY_HEIGHT, copy_args, copy_path_of};
use serde_json::Value;

fn vectors() -> Value {
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/vectors/host-copy.json");
    serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap()
}

fn kind(name: &str) -> CopyKind {
    match name {
        "remux" => CopyKind::Remux,
        "transcode" => CopyKind::Transcode,
        _ => CopyKind::Proxy,
    }
}

#[test]
fn ffmpegs_arguments_per_kind_are_the_typescripts() {
    let v = vectors();
    assert_eq!(v["proxyHeight"], PROXY_HEIGHT);
    let cases = v["args"].as_array().unwrap();
    assert!(cases.len() >= 20);
    for c in cases {
        let want: Vec<String> = serde_json::from_value(c["args"].clone()).unwrap();
        let (k, source, dest) = (
            c["kind"].as_str().unwrap(),
            c["source"].as_str().unwrap(),
            c["dest"].as_str().unwrap(),
        );
        assert_eq!(copy_args(kind(k), source, dest), want, "{k} {source}");
    }
}

#[test]
fn a_copy_lands_beside_its_source_and_a_name_with_no_extension_comes_back_as_it_is() {
    let v = vectors();
    let cases = v["paths"].as_array().unwrap();
    assert!(cases.len() >= 10);
    for c in cases {
        let (source, k) = (c["source"].as_str().unwrap(), c["kind"].as_str().unwrap());
        assert_eq!(
            copy_path_of(source, k),
            c["path"].as_str().unwrap(),
            "{source}"
        );
    }
}
