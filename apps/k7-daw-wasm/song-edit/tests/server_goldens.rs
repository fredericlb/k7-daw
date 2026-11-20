// server_goldens.rs — replays the goldens of makeServerGoldens.mts: each server edit over the same tree, the same files after.
// A reader would get wrong the failures: the TypeScript leaves what it wrote before it threw, a replay only checks the message.

use serde_json::Value as Json;
use song_edit::MemFs;
use song_edit::confine::Env;
use song_edit::format_stamp::stamp_format_for_seconds;
use song_edit::fs::Fs;
use song_edit::open_repairs::repair_picture_strips;
use song_edit::record_take::{Meter, RecordedTake, TakeTarget, add_take_to_event, add_take_to_track};
use std::collections::BTreeMap;

const ROOT: &str = "/w";

fn cases(name: &str) -> Vec<Json> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/goldens/server/{name}.json"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str::<Json>(&text).unwrap()["cases"]
        .as_array()
        .unwrap()
        .clone()
}

fn mem(c: &Json) -> MemFs {
    let files = c["tree"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| (k.clone(), v.as_str().unwrap().as_bytes().to_vec()));
    MemFs::from_tree(ROOT, files)
}

fn dir(text: &str) -> String {
    text.replace("$DIR", ROOT)
}

/// The files a case says changed must read so; a thrown case checks the message only.
fn check(c: &Json, fs: &MemFs, got: Result<Json, String>) {
    let name = c["name"].as_str().unwrap();
    if let Some(msg) = c["thrown"].as_str() {
        assert_eq!(got.expect_err(name), dir(msg), "{name}");
        return;
    }
    assert_eq!(got.unwrap_or_else(|e| panic!("{name}: {e}")), c["ret"], "{name}");
    let have: BTreeMap<String, String> = fs
        .files()
        .map(|(p, b)| (p.to_owned(), String::from_utf8_lossy(b).into_owned()))
        .collect();
    assert_eq!(have, expected_tree(c), "{name}");
}

fn meter(c: &Json) -> Meter {
    let d = &c["args"]["doc"];
    Meter {
        tempo: d["tempo"].as_f64().unwrap(),
        num: d["timeSigNumerator"].as_f64().unwrap(),
        den: d["timeSigDenominator"].as_f64().unwrap(),
    }
}

fn recorded(c: &Json) -> RecordedTake {
    let t = &c["args"]["take"];
    RecordedTake {
        track: t["track"].as_str().unwrap().to_owned(),
        file: dir(t["file"].as_str().unwrap()),
        start: t["start"].as_f64().unwrap(),
        length: t["length"].as_f64().unwrap(),
    }
}

#[test]
fn record_take_replays_the_typescript() {
    let all = cases("recordTake");
    assert!(all.len() >= 12);
    for c in &all {
        let mut fs = mem(c);
        let (m, take, stamp) = (meter(c), recorded(c), c["args"]["stamp"].as_str().unwrap());
        let d = format!("{ROOT}/p.k7");
        let got = if c["fn"] == "addTakeToTrack" {
            add_take_to_track(&mut fs, &d, m, &take, stamp)
        } else {
            let t = &c["args"]["target"];
            let punch = t
                .get("punch")
                .map(|p| (p["from"].as_f64().unwrap(), p.get("to").and_then(Json::as_f64)));
            let target = TakeTarget {
                event_beats: t["eventBeats"].as_f64().unwrap(),
                punch,
            };
            add_take_to_event(&mut fs, &d, m, &take, &target, stamp)
        };
        check(c, &fs, got.map(Json::from).map_err(|e| e.to_string()));
    }
}

/// The repository's `@meta` modules, mounted where the loader reads them.
fn mount_meta(fs: &mut MemFs, from: &std::path::Path, to: &str) {
    let _ = Fs::mkdir_all(fs, to);
    for e in std::fs::read_dir(from).into_iter().flatten().flatten() {
        let (path, name) = (e.path(), e.file_name().to_string_lossy().into_owned());
        if path.is_dir() {
            mount_meta(fs, &path, &format!("{to}/{name}"));
        } else if let Ok(bytes) = std::fs::read(&path) {
            let _ = Fs::write_atomic(fs, &format!("{to}/{name}"), &bytes);
        }
    }
}

/// The tree a case ends with: what it began with, the changed files laid over it.
fn expected_tree(c: &Json) -> BTreeMap<String, String> {
    let mut out: BTreeMap<String, String> = c["tree"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| (format!("{ROOT}/{k}"), v.as_str().unwrap().to_owned()))
        .collect();
    for (k, v) in c["changed"].as_object().unwrap() {
        match v.as_str() {
            Some(t) => out.insert(format!("{ROOT}/{k}"), t.to_owned()),
            None => out.remove(&format!("{ROOT}/{k}")),
        };
    }
    out
}

fn open_cases() -> Vec<(Json, MemFs, Vec<Json>)> {
    let meta = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../song/meta");
    cases("openRepairs")
        .into_iter()
        .map(|c| {
            let mut fs = mem(&c);
            mount_meta(&mut fs, &meta, &format!("{ROOT}/meta"));
            let env = Env::at(&format!("{ROOT}/home"), &format!("{ROOT}/meta"));
            let mut n = 0;
            let mut new_id = || {
                n += 1;
                format!("00000000-0000-4000-8000-{n:012}")
            };
            let dir = format!("{ROOT}/p.k7");
            let notes = repair_picture_strips(&mut fs, &env, &dir, "p.k7", &mut new_id).unwrap_or_default();
            stamp_format_for_seconds(&mut fs, &dir);
            let said = notes
                .iter()
                .map(|n| serde_json::json!({ "warn": n.warn, "text": n.text }))
                .collect();
            (c, fs, said)
        })
        .collect()
}

#[test]
fn open_repairs_replay_the_typescript() {
    let all = open_cases();
    assert!(all.len() >= 7);
    for (c, fs, said) in &all {
        let name = c["name"].as_str().unwrap();
        assert_eq!(Json::from(said.clone()), c["ret"], "{name}");
        let have: BTreeMap<String, String> = fs
            .files()
            .filter(|(p, _)| !p.starts_with(&format!("{ROOT}/meta/")))
            .map(|(p, b)| (p.to_owned(), String::from_utf8_lossy(b).into_owned()))
            .collect();
        assert_eq!(have, expected_tree(c), "{name}");
    }
}
