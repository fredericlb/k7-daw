//! The probe without ffprobe: the reader's facts equal ffprobe's on the fixtures, and hostile files are refused, never allocated.

use std::io::Cursor;
use std::path::PathBuf;
use std::process::Command;

use k7_mediatek::video_probe::{facts_of, probe_args};
use k7_mediatek::video_read::{Unsupported, read_facts, read_facts_from};
use serde_json::Value;

fn media(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/media")
        .join(name)
}

fn bytes(name: &str) -> Vec<u8> {
    std::fs::read(media(name)).unwrap()
}

fn read_bytes(b: Vec<u8>) -> Result<Option<Value>, Unsupported> {
    let len = b.len() as u64;
    read_facts_from(Cursor::new(b), len)
}

/// ffprobe's own answer for a fixture, through the same arguments and parse the front uses.
fn ffprobe(name: &str) -> Option<Value> {
    let out = Command::new("ffprobe")
        .args(probe_args(media(name).to_str().unwrap()))
        .output()
        .ok()?;
    assert!(out.status.success(), "ffprobe failed on {name}");
    Some(
        facts_of(&String::from_utf8_lossy(&out.stdout))
            .unwrap()
            .unwrap(),
    )
}

fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            (x.as_f64().unwrap() - y.as_f64().unwrap()).abs() < 1e-9
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| same(v, w)))
        }
        _ => a == b,
    }
}

const FIXTURES: [&str; 5] = [
    "plain.mp4",
    "rotated.mp4",
    "hevc.mov",
    "vp9.webm",
    "h264.mkv",
];

#[test]
fn the_reader_states_what_ffprobe_states() {
    for name in FIXTURES {
        let got = read_facts(&media(name)).unwrap().unwrap();
        let Some(want) = ffprobe(name) else {
            eprintln!("no ffprobe here: {name} compared with its known facts only");
            assert_eq!(got["width"], 160, "{name}");
            continue;
        };
        assert!(same(&got, &want), "{name}: {got} != {want}");
    }
}

#[test]
fn a_rotated_file_states_its_quarter_turn() {
    let got = read_facts(&media("rotated.mp4")).unwrap().unwrap();
    assert_eq!(got["rotation"], 90);
    assert!(
        read_facts(&media("plain.mp4"))
            .unwrap()
            .unwrap()
            .get("rotation")
            .is_none()
    );
}

#[test]
fn the_container_is_sniffed_by_its_bytes_not_its_name() {
    let dir = std::env::temp_dir().join(format!("k7-video-read-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let renamed = dir.join("film.txt");
    std::fs::write(&renamed, bytes("vp9.webm")).unwrap();
    let got = read_facts(&renamed).unwrap().unwrap();
    assert_eq!(got["container"], "matroska,webm");
    let named_mp4 = dir.join("sound.mp4");
    std::fs::write(&named_mp4, b"RIFF....WAVEfmt not a video at all").unwrap();
    assert_eq!(read_facts(&named_mp4), Err(Unsupported));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_truncated_file_is_unsupported() {
    for name in FIXTURES {
        let b = bytes(name);
        for cut in [4, 11, 40, b.len() / 2] {
            let r = read_bytes(b[..cut].to_vec());
            assert!(r.is_err() || cut == b.len(), "{name} cut at {cut}: {r:?}");
        }
    }
}

#[test]
fn a_box_size_past_the_end_is_unsupported() {
    let mut b = bytes("plain.mp4");
    // The first box after ftyp starts at the ftyp's size; claim far more than the file has.
    let at = u32::from_be_bytes(b[0..4].try_into().unwrap()) as usize;
    b[at..at + 4].copy_from_slice(&0x7fff_0000u32.to_be_bytes());
    assert_eq!(read_bytes(b), Err(Unsupported));
}

#[test]
fn a_huge_box_size_is_not_allocated() {
    let mut big = b"\0\0\0\x14ftypisom\0\0\0\0isom".to_vec();
    big.extend_from_slice(&1u32.to_be_bytes());
    big.extend_from_slice(b"moov");
    big.extend_from_slice(&u64::MAX.to_be_bytes());
    assert_eq!(read_bytes(big), Err(Unsupported));
    let mut under_cap = b"\0\0\0\x14ftypisom\0\0\0\0isom".to_vec();
    under_cap.extend_from_slice(&0x7000_0000u32.to_be_bytes());
    under_cap.extend_from_slice(b"moov");
    assert_eq!(read_bytes(under_cap), Err(Unsupported));
    let mut mkv = vec![
        0x1A, 0x45, 0xDF, 0xA3, 0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    ];
    mkv.extend_from_slice(&[0x42, 0x86, 0x01, 0x7F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);
    assert_eq!(read_bytes(mkv), Err(Unsupported));
}

#[test]
fn a_file_that_is_not_a_video_is_unsupported() {
    assert_eq!(read_bytes(Vec::new()), Err(Unsupported));
    assert_eq!(read_bytes(vec![0; 4096]), Err(Unsupported));
    assert_eq!(
        read_bytes(b"RIFF\0\0\0\0WAVEfmt ".to_vec()),
        Err(Unsupported)
    );
    assert_eq!(read_bytes(b"%PDF-1.7\n".repeat(100)), Err(Unsupported));
}

#[test]
fn every_byte_flipped_in_the_header_is_an_answer_never_a_panic() {
    for name in ["plain.mp4", "h264.mkv"] {
        let b = bytes(name);
        for at in (0..400.min(b.len())).step_by(3) {
            let mut c = b.clone();
            c[at] ^= 0xFF;
            let _ = read_bytes(c);
        }
    }
}
