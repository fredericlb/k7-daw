//! The engine link's pure rules against `vectors/engine.json`: the session names, `servedProject`, the backoff, and what
//! the send does to a document over the golden corpus (the paths it names, a take's hole, the groups' mix).

use k7_file_service::engine::docs::{TakeArm, prepare};
use k7_file_service::engine::sync::collect;
use k7_file_service::engine::{
    LOCAL_RECONNECT_MAX_MS, OUTBOX_MAX, OUTBOX_TTL_MS, RECONNECT_MAX_MS, RECONNECT_MIN_MS,
    jittered_backoff_ms, served_project, session_for, session_of,
};
use serde_json::{Value, json};

use crate::support::engine::{js_numbers, vectors};

#[test]
fn the_constants_are_the_typescripts() {
    let c = &vectors()["constants"];
    assert_eq!(c["reconnect_min_ms"], json!(RECONNECT_MIN_MS));
    assert_eq!(c["reconnect_max_ms"], json!(RECONNECT_MAX_MS));
    assert_eq!(c["outbox_max"], json!(OUTBOX_MAX));
    assert_eq!(c["outbox_ttl_ms"], json!(OUTBOX_TTL_MS));
    assert_eq!(
        c["bench_workspace"],
        json!(k7_file_service::engine::BENCH_WORKSPACE)
    );
    assert_eq!(
        c["services"],
        json!({ "k7": "k7", "pl8": "pl8", "bench": "banc" })
    );
}

#[test]
fn every_session_is_named_as_the_typescript_names_it() {
    let cases = vectors()["session_of"].as_array().unwrap();
    assert!(cases.len() >= 392);
    for c in cases {
        let got = session_of(
            Some(c["root"].as_str().unwrap()),
            c["files_root"].as_str().unwrap(),
            c["owner"].as_str().unwrap(),
        );
        assert_eq!(json!(got), c["session"], "{c}");
    }
    let cases = vectors()["session_for"].as_array().unwrap();
    for c in cases {
        let got = session_for(c["service"].as_str().unwrap(), c["who"].as_str());
        assert_eq!(json!(got), c["session"], "{c}");
    }
}

#[test]
fn the_served_project_is_named_by_address() {
    let cases = vectors()["served_project"].as_array().unwrap();
    assert!(cases.len() >= 1260);
    for c in cases {
        let (owner, files_root, root, asked) = (
            c["owner"].as_str().unwrap(),
            c["files_root"].as_str().unwrap(),
            c["root"].as_str().unwrap(),
            c["asked"].as_str().unwrap(),
        );
        // The session the workspace is served under, and the two keys `servedProject` looks it up by.
        let held = session_of(Some(root), files_root, owner);
        let found = held == session_for("k7", Some(asked)) || held == "k7";
        let got = found
            .then(|| served_project(files_root, root, c["project"].as_str().unwrap()))
            .flatten()
            .map_or(
                Value::Null,
                |(workspace, path)| json!({ "workspace": workspace, "path": path }),
            );
        assert_eq!(got, c["out"], "{c}");
    }
}

#[test]
fn the_backoff_is_half_jittered_and_capped() {
    let cases = vectors()["backoff"].as_array().unwrap();
    assert!(cases.len() >= 195);
    for c in cases {
        let got = jittered_backoff_ms(
            c["attempt"].as_u64().unwrap() as u32,
            c["max"].as_u64().unwrap(),
            c["random"].as_f64().unwrap(),
        );
        assert_eq!(js_numbers(&json!(got)), c["ms"], "{c}");
    }
    // The link's own ceiling is the short one.
    assert!(
        jittered_backoff_ms(12, LOCAL_RECONNECT_MAX_MS, 0.999999) <= LOCAL_RECONNECT_MAX_MS as f64
    );
}

fn documents() -> Vec<(String, Value)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/golden");
    let roots = vectors()["corpus"]["roots"].as_object().unwrap().clone();
    let absolute = |v: &Value| -> Value {
        let mut text = v.to_string();
        for (token, dir) in &roots {
            text = text.replace(token.as_str(), dir.as_str().unwrap());
        }
        serde_json::from_str(&text).unwrap()
    };
    let mut out = vec![];
    let mut sources: Vec<_> = std::fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    sources.sort();
    for dir in sources.into_iter().filter(|p| p.is_dir()) {
        let mut files: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        files.sort();
        for f in files {
            let v: Value = serde_json::from_str(&std::fs::read_to_string(&f).unwrap()).unwrap();
            if let Some(engine) = v.get("engine").filter(|e| !e.is_null()) {
                let name = format!(
                    "{}/{}",
                    dir.file_name().unwrap().to_string_lossy(),
                    f.file_name().unwrap().to_string_lossy()
                );
                out.push((name, absolute(engine)));
            }
        }
    }
    out
}

#[test]
fn the_paths_a_document_names_are_the_typescripts() {
    let docs = documents();
    let want = vectors()["corpus"]["collect"].as_array().unwrap();
    assert_eq!(docs.len(), want.len());
    for ((name, doc), w) in docs.iter().zip(want) {
        assert_eq!(&json!(name), &w["doc"]);
        let mut got = vec![];
        collect(doc, &mut got, false);
        assert_eq!(json!(got), w["paths"], "{name}");
    }
}

#[test]
fn a_take_s_hole_is_cut_as_the_typescript_cuts_it() {
    let docs = documents();
    let holes = vectors()["corpus"]["take_hole"].as_array().unwrap();
    assert!(!holes.is_empty());
    for h in holes {
        let name = h["doc"].as_str().unwrap();
        let doc = &docs.iter().find(|(n, _)| n == name).unwrap().1;
        let arm = TakeArm {
            track: h["track"].as_str().unwrap().to_string(),
            event_beats: 0.0,
            from: h["punch"]["from"].as_f64().unwrap(),
            to: h["punch"]["to"].as_f64(),
        };
        let sent = prepare(doc, Some(&arm), None);
        assert!(
            sent.typed,
            "{name}: the typed reader takes a compiled document"
        );
        assert_eq!(sent.doc["recordMute"], h["recordMute"], "{name}");
        let lane = sent.doc["tracks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["id"] == h["track"])
            .unwrap();
        assert_eq!(
            js_numbers(&lane["clips"]),
            js_numbers(&h["clips"]),
            "{name} {}",
            h["punch"]
        );
    }
}

#[test]
fn the_groups_fold_onto_their_members() {
    let docs = documents();
    for m in vectors()["corpus"]["group_mix"].as_array().unwrap() {
        let name = m["doc"].as_str().unwrap();
        let doc = &docs.iter().find(|(n, _)| n == name).unwrap().1;
        let sent = prepare(doc, None, None);
        assert!(sent.typed, "{name}");
        if m["same"] == json!(true) {
            assert_eq!(
                js_numbers(&sent.doc),
                js_numbers(doc),
                "{name}: nothing folded"
            );
        } else {
            let got: Vec<Value> = sent.doc["tracks"]
                .as_array()
                .unwrap()
                .iter()
                .map(|t| json!([t["id"], t["mute"], t["solo"]]))
                .collect();
            assert_eq!(Value::Array(got), m["tracks"], "{name}");
        }
    }
}
