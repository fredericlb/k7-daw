//! The workspaces' rules against the TypeScript's own functions (1631): every case of `vectors/workspaces.json`
//! (made by `js/makeWorkspaceVectors.mts`), 5,000 cases of names, tags, numbers and sort orders.

use crate::support;

use k7_file_service::filter::project_app;
use k7_file_service::space::Declined;
use k7_file_service::workspaces::WsError;
use k7_file_service::workspaces::copy::{copy_name, playlist_path_of};
use k7_file_service::workspaces::demos::tree_stamp;
use k7_file_service::workspaces::listing::tags_in;
use k7_file_service::workspaces::names::{name_ok, project_dirs_under};
use k7_file_service::workspaces::refs::{Todo, raw_matches, refs_in};
use serde_json::Value;
use std::collections::BTreeSet;
use support::ws::Acct;

fn vectors() -> Vec<Value> {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/vectors/workspaces.json"
    ))
    .unwrap();
    serde_json::from_str(&text).unwrap()
}

#[test]
fn replays_every_vector_of_the_workspaces_rules() {
    let all = vectors();
    assert!(all.len() >= 5000, "{} cases", all.len());
    let mut seen = BTreeSet::new();
    for c in &all {
        let f = c["fn"].as_str().unwrap();
        let (name, out) = (c["name"].as_str().unwrap_or(""), &c["out"]);
        match f {
            "workspace_name" => {
                let said = if name_ok(name) {
                    Value::Bool(true)
                } else {
                    serde_json::json!({ "error": WsError::Invalid { name: name.into() }.to_string() })
                };
                assert_eq!(&said, out, "{f} {name:?}");
            }
            "project_app" => assert_eq!(
                project_app(name).map(Value::from).unwrap_or(Value::Null),
                *out,
                "{f} {name:?}"
            ),
            "tags" => {
                let text = c["text"].as_str().unwrap();
                let got = match tags_in(text) {
                    Ok(t) => Value::from(t),
                    Err(Declined::LossyText) => serde_json::json!({ "lossy": true }),
                    Err(d) => panic!("{d:?} for {text:?}"),
                };
                assert_eq!(&got, out, "{f} {text:?}");
            }
            "number" => {
                let n = c["n"].as_f64().unwrap();
                assert_eq!(
                    k7_js::number::to_string(n),
                    out.as_str().unwrap(),
                    "{f} {n:e}"
                );
            }
            "round" => {
                let n = c["n"].as_f64().unwrap();
                assert_eq!(
                    k7_js::number::math_round(n),
                    out.as_f64().unwrap(),
                    "{f} {n:e}"
                );
            }
            "sort" => sorts_like_code_points(c),
            "copy_name" => {
                let (root, src, raw) = (
                    c["root"].as_str().unwrap(),
                    c["src"].as_str().unwrap(),
                    c["raw"].as_str().unwrap(),
                );
                let got = match copy_name(root, src, raw) {
                    Ok(n) => serde_json::json!({ "rel": n }),
                    Err(e) => serde_json::json!({ "error": e.to_string() }),
                };
                assert_eq!(&got, out, "{f} {root} {src} {raw:?}");
            }
            "playlist_path" => assert_eq!(
                playlist_path_of(name)
                    .map(Value::from)
                    .unwrap_or(Value::Null),
                *out,
                "{f} {name:?}"
            ),
            "refs" => {
                let text = c["text"].as_str().unwrap();
                let mut todo = Todo::default();
                refs_in(text, &mut todo);
                assert_eq!(Value::from(todo.items()), *out, "{f} {text:?}");
            }
            "ref_raw" => {
                let text = c["text"].as_str().unwrap();
                assert_eq!(Value::from(raw_matches(text)), *out, "{f} {text:?}");
            }
            "json_string" => {
                let s = c["s"].as_str().unwrap();
                assert_eq!(
                    Value::from(s).to_string(),
                    out.as_str().unwrap(),
                    "{f} {s:?}"
                );
            }
            "stamp" => replays_a_stamp(c),
            _ => continue,
        }
        seen.insert(f.to_string());
    }
    for f in [
        "workspace_name",
        "project_app",
        "tags",
        "number",
        "round",
        "sort",
    ] {
        assert!(seen.contains(f), "{f}");
    }
}

/// The project folders of a scratch workspace come back in the order the TypeScript sorts the names.
fn sorts_like_code_points(c: &Value) {
    let names: Vec<&str> = c["names"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n.as_str().unwrap())
        .collect();
    let a = Acct::new("ws-vec-sort");
    let root = a.ws("ws");
    let mut made = Vec::new();
    for n in &names {
        let dir = format!("{n}.k7");
        if !n.starts_with('.') && std::fs::create_dir(format!("{root}/{dir}")).is_ok() {
            made.push(dir);
        }
    }
    let want: Vec<String> = c["out"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| format!("{}.k7", n.as_str().unwrap()))
        .filter(|n| made.contains(n))
        .collect();
    let got = project_dirs_under(&root, &Default::default(), &mut 0).unwrap();
    // A name the disk folds together (`e` and `E` on APFS) is made once, so both lists are the ones made.
    assert_eq!(got.len(), made.len());
    let order: Vec<&String> = want.iter().filter(|w| got.contains(w)).collect();
    let in_got: Vec<&String> = got.iter().filter(|g| want.contains(g)).collect();
    assert_eq!(order, in_got, "{names:?}");
}

/// A demo folder made of the vector's files, mtimes exact, stamps to the TypeScript's own (1641).
fn replays_a_stamp(c: &Value) {
    // NTFS keeps a time in 100 ns steps, so a vector finer than that cannot be laid down on Windows.
    let fine = |f: &Value| !f["nanos"].as_u64().unwrap().is_multiple_of(100);
    if cfg!(windows) && c["files"].as_array().unwrap().iter().any(fine) {
        return;
    }
    let a = Acct::new("ws-vec-stamp");
    let dir = a.s.dir("demo.k7");
    for f in c["files"].as_array().unwrap() {
        let rel = f["rel"].as_str().unwrap();
        let abs = a.s.file(
            &format!("demo.k7/{rel}"),
            &"x".repeat(f["size"].as_u64().unwrap() as usize),
        );
        support::ws::touch(
            &abs,
            f["secs"].as_u64().unwrap(),
            f["nanos"].as_u64().unwrap() as u32,
        );
    }
    assert_eq!(
        tree_stamp(&dir),
        c["out"].as_str().unwrap(),
        "{:?}",
        c["files"]
    );
}
