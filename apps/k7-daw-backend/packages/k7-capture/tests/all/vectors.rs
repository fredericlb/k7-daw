//! vectors.rs — every pure rule against the TypeScript's own answers (`fixtures/capture-vectors.json`, 2180): the log
//! folder's name, the stamp, binary by content, the ceilings, the confined path, the local name and the files written.
//! What a reader gets wrong: the Windows-spelled rows run on every host, through the lexical path model.

use k7_capture::confirmed::is_atomic_temp;
use k7_capture::disk::{LiveLog, confined_path, load_head, log_dir_of};
use k7_capture::ledger::{keeps_text, looks_binary, stamp_of, text_ceiling};
use serde_json::{Value, json};

use crate::support::{Scratch, table, vectors};

fn s(v: &Value) -> &str {
    v.as_str().unwrap()
}

#[test]
fn the_log_folder_is_named_as_the_typescript_names_it() {
    let rows = table("logFor");
    assert!(rows.iter().any(|r| r["windows"] == json!(true)));
    for r in &rows {
        assert_eq!(
            log_dir_of(s(&r["home"]), s(&r["root"])),
            s(&r["out"]),
            "{r}"
        );
    }
}

#[test]
fn the_stamp_prints_mtime_as_javascript_prints_it() {
    let rows = table("stamp");
    assert_eq!(rows.len(), 300);
    for r in &rows {
        let (size, mtime) = (
            r["size"].as_f64().unwrap() as u64,
            r["mtimeMs"].as_f64().unwrap(),
        );
        assert_eq!(stamp_of(size, mtime), s(&r["out"]), "{r}");
    }
}

#[test]
fn binary_by_content_and_the_ceilings() {
    for r in table("binary") {
        let mut buf = vec![0x61u8; r["len"].as_u64().unwrap() as usize];
        if let Some(at) = r["nulAt"].as_u64() {
            buf[at as usize] = 0;
        }
        assert_eq!(looks_binary(&buf), r["out"].as_bool().unwrap(), "{r}");
    }
    for r in table("ceiling") {
        assert_eq!(
            text_ceiling(s(&r["rel"])),
            r["ceiling"].as_u64().unwrap(),
            "{r}"
        );
        assert_eq!(
            keeps_text(s(&r["rel"])),
            r["keeps"].as_bool().unwrap(),
            "{r}"
        );
    }
}

#[test]
fn a_confined_path_is_refused_with_the_typescripts_sentence() {
    let rows = table("confined");
    assert!(rows.iter().any(|r| r.get("error").is_some()));
    for r in &rows {
        let got = confined_path(s(&r["root"]), s(&r["rel"]));
        let got = got.as_ref().map(String::as_str).map_err(String::as_str);
        match (r.get("out"), r.get("error")) {
            (Some(out), _) => assert_eq!(got, Ok(s(out)), "{r}"),
            (_, Some(e)) => assert_eq!(got, Err(s(e)), "{r}"),
            _ => panic!("{r}"),
        }
    }
}

#[test]
fn the_log_tails_as_javascript_does() {
    let v = vectors();
    let t = &v["tail"];
    let dir = Scratch::new("tail");
    std::fs::write(dir.path().join("batches.jsonl"), s(&t["file"])).unwrap();
    let log = LiveLog::new(&dir.at(""));
    for r in t["rows"].as_array().unwrap() {
        let n = match r["n"].as_str() {
            Some("Infinity") => f64::INFINITY,
            Some(_) => f64::NEG_INFINITY,
            None => r["n"].as_f64().unwrap(),
        };
        let ids: Vec<Value> = log
            .tail(n)
            .unwrap()
            .iter()
            .map(|b| b["id"].clone())
            .collect();
        assert_eq!(Value::Array(ids), r["out"], "{r}");
    }
}

#[test]
fn the_head_and_the_temp_names() {
    for r in table("head") {
        let dir = Scratch::new("head");
        let base = dir.at("log");
        if let Some(text) = r["text"].as_str() {
            std::fs::create_dir_all(&base).unwrap();
            std::fs::write(format!("{base}/head"), text).unwrap();
        }
        let nan = r["text"]
            .as_str()
            .is_some_and(|t| k7_capture::num::js_number(t).is_nan());
        if nan {
            // The TypeScript reads 0 and replays from the start; here the head is "not now" (2109).
            assert!(load_head(&base).is_err(), "{r}");
        } else {
            assert_eq!(load_head(&base).unwrap(), r["out"].as_u64().unwrap(), "{r}");
        }
    }
    for r in table("temp") {
        assert_eq!(
            is_atomic_temp(s(&r["name"])),
            r["out"].as_bool().unwrap(),
            "{r}"
        );
    }
}

#[test]
fn what_the_rust_writes_is_byte_for_byte_what_the_typescript_writes() {
    let v = vectors();
    let w = &v["written"];
    let dir = Scratch::new("written");
    // The blob index: a file named `2024` or `10` is ordered first by JavaScript, an unsettled or pending one is left out.
    let mut ledger = indexmap::IndexMap::new();
    for r in w["index"]["ledger"].as_array().unwrap() {
        let opt = |k: &str| r.get(k).and_then(Value::as_str).map(String::from);
        ledger.insert(
            s(&r["rel"]).to_string(),
            k7_capture::ledger::Known {
                stamp: s(&r["stamp"]).into(),
                hash: s(&r["hash"]).into(),
                text: None,
                r#ref: opt("ref"),
                pending: r.get("pending").is_some(),
            },
        );
    }
    let file = dir.at("index/blob-index.json");
    let mut index = k7_capture::index::BlobIndex::open(&file);
    index
        .save(&ledger, w["index"]["now"].as_f64().unwrap())
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        s(&w["index"]["file"])
    );
    // `pending.json`.
    let files: indexmap::IndexMap<String, String> = w["pending"]["files"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| (k.clone(), s(v).to_string()))
        .collect();
    k7_capture::cache::BlobCache::new(&dir.at("pend"))
        .save_pending(&files)
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.at("pend/pending.json")).unwrap(),
        s(&w["pending"]["file"])
    );
    // `batches.jsonl`.
    let log = LiveLog::new(&dir.at("logw"));
    for b in w["log"]["batches"].as_array().unwrap() {
        log.append(b).unwrap();
    }
    assert_eq!(
        std::fs::read_to_string(dir.at("logw/batches.jsonl")).unwrap(),
        s(&w["log"]["file"])
    );
}
