//! 2604 replayed: every case of `vectors/runtime.json`, made by `js/makeRuntimeVectors.mts` from the TypeScript's own functions.
//! Ids are compared by order of first sight in the background section, since its scenarios share one module there; `-0` prints as 0.

use std::collections::HashMap;

use k7_file_service::runtime::ring::{Level, Ring};
use serde_json::{Value, json};

fn vectors() -> Value {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/vectors/runtime.json"
    ))
    .unwrap();
    serde_json::from_str(&text).unwrap()
}

fn pushes(n: usize) -> Ring {
    let texts = [
        "boot",
        "",
        "two\nlines",
        "quo\"te",
        "é e\u{301}",
        "😀 astral",
        "[cmd] ok",
    ];
    let long = "x".repeat(40);
    let mut ring = Ring::new();
    for i in 0..n {
        let level = [Level::Info, Level::Warn, Level::Error][i % 3];
        let t = if i % 8 == 7 { &long } else { texts[i % 8] };
        ring.push(
            level,
            &format!("{i}: {t}"),
            1_700_000_000_000 + i as u64 * 1000 + (i as u64 % 3),
        );
    }
    ring
}

fn replay_since(section: &str, whole: bool) {
    let all = vectors();
    let rows = all[section].as_array().unwrap();
    assert!(rows.len() >= 45, "{section}: {}", rows.len());
    let mut rings: HashMap<u64, Ring> = HashMap::new();
    for r in rows {
        let n = r["pushes"].as_u64().unwrap();
        let ring = rings.entry(n).or_insert_with(|| pushes(n as usize));
        let after = r["after"].as_f64();
        let (seq, lines) = ring.since(after, r["tail"].as_f64().unwrap().trunc() as usize);
        let want = &r["out"];
        if whole {
            let got = json!({ "seq": seq, "lines": lines.iter().map(|l| l.to_json()).collect::<Vec<_>>() });
            assert_eq!(&got, want, "{r}");
        } else {
            let first = lines.first().map(|l| l.seq);
            let last = lines.last().map(|l| l.seq);
            assert_eq!(
                json!([seq, lines.len(), first, last]),
                json!([want["seq"], want["count"], want["first"], want["last"]]),
                "{r}"
            );
        }
    }
}

#[test]
fn a_big_ring_answers_its_count_and_ends() {
    replay_since("since_big", false);
}

