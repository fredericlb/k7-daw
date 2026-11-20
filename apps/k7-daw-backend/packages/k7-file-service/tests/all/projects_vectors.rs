//! The marker's bytes and reads, the rename's name and the open's guards against the TypeScript's own functions
//! (2021, 2023, 2030, 2031): every case of `vectors/projects.json` (made by `js/makeProjectVectors.mts`).

use crate::support;

use std::collections::BTreeSet;

use k7_file_service::confine::Disk;
use k7_file_service::entries::plan::clean_name;
use k7_file_service::filter::project_app;
use k7_file_service::libraries::Home;
use k7_file_service::projects::ProjError;
use k7_file_service::projects::life::plan_rename;
use k7_file_service::projects::marker::{marker_of, read_raw, write};
use k7_file_service::projects::open::plan_open;
use k7_file_service::space::Space;
use serde_json::{Value, json};
use support::{Scratch, can_link};

fn vectors() -> Vec<Value> {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/vectors/projects.json"
    ))
    .unwrap();
    serde_json::from_str(&text).unwrap()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A read as the vectors store it: the scratch's depth replaced by `<root>` and `<scratch>`.
fn shown(abs: &str, mroot: &str, scratch: &str) -> String {
    let under = |base: &str| abs == base || abs.starts_with(&format!("{base}/"));
    if under(mroot) {
        format!("<root>{}", &abs[mroot.len()..])
    } else if under(scratch) {
        format!("<scratch>{}", &abs[scratch.len()..])
    } else {
        abs.to_string()
    }
}

/// The marker file's folder, made, with `text` as the marker.
fn put(root: &str, text: &str) {
    let at = marker_of(root);
    std::fs::create_dir_all(k7_file_service::paths::dirname(&at)).unwrap();
    std::fs::write(at, text).unwrap();
}

fn open_fixture(name: &str, s: &Scratch) -> String {
    let (ws, entries): (&str, &[(&str, &str)]) = if name == "plain" {
        (
            "ws",
            &[
                ("ws/a.k7/project.k7x", "x"),
                ("ws/notes/n.txt", "x"),
                ("ws/deep/b.k7/project.k7x", "x"),
                ("ws/deep/c.txt", "x"),
                ("ws/f.k7", "file"),
                ("ws/_recycle_bin/x.k7/project.k7x", "x"),
                ("ws/.hidden.k7/project.k7x", "x"),
                ("ws/.k7/project.k7x", "x"),
                ("ws/nested/a.k7/project.k7x", "x"),
                ("libraries/drums/demo.k7/project.k7x", "x"),
                ("libraries/drums/plain/p.txt", "x"),
            ],
        )
    } else {
        (
            "r.k7",
            &[
                ("r.k7/x.k7/project.k7x", "x"),
                ("r.k7/sub/y.k7/project.k7x", "x"),
                ("r.k7/project.k7x", "x"),
                ("libraries/drums/demo.k7/project.k7x", "x"),
            ],
        )
    };
    for (p, t) in entries {
        s.file(p, t);
    }
    if name == "plain" && can_link(s) {
        support::link(&s.path("ws/a.k7"), &s.path("ws/ln.k7")).unwrap();
    }
    s.path(ws)
}

#[test]
fn replays_every_vector_of_the_projects_rules() {
    let all = vectors();
    assert!(all.len() >= 1000, "{} cases", all.len());
    let mut seen = BTreeSet::new();
    let s = Scratch::new("pv-open");
    let kept: Vec<(String, Scratch, String)> = ["plain", "root is a project"]
        .iter()
        .map(|n| {
            let sub = Scratch::new(&format!("pv-{}", n.replace(' ', "-")));
            let root = open_fixture(n, &sub);
            (n.to_string(), sub, root)
        })
        .collect();
    let m = Scratch::new("pv-marker");
    let mroot = m.path("m/workspace/ws");
    let top = m.path("");
    for c in &all {
        let f = c["fn"].as_str().unwrap();
        seen.insert(f.to_string());
        match f {
            "marker_of" => assert_eq!(
                marker_of(c["root"].as_str().unwrap()),
                c["out"].as_str().unwrap(),
                "{c}"
            ),
            "marker" => {
                let rel = c["rel"].as_str().unwrap();
                write(&mroot, rel).unwrap();
                let bytes = std::fs::read(marker_of(&mroot)).unwrap();
                assert_eq!(hex(&bytes), c["hex"].as_str().unwrap(), "{rel:?}");
                let got = read_raw(&mroot).unwrap();
                let read = got
                    .as_deref()
                    .map_or(String::new(), |abs| shown(abs, &mroot, &top));
                assert_eq!(got.is_none(), c["none"] == json!(true), "{rel:?}");
                assert_eq!(read, c["read"].as_str().unwrap(), "{rel:?}");
            }
            "read_raw" => {
                let text = c["text"].as_str().unwrap();
                put(&mroot, text);
                let got = read_raw(&mroot)
                    .unwrap()
                    .map(|abs| shown(&abs, &mroot, &top));
                assert_eq!(got.map_or(Value::Null, Value::from), c["out"], "{text:?}");
            }
            "rename_name" => {
                let name = c["name"].as_str().unwrap();
                let clean = clean_name(name);
                assert_eq!(clean, c["clean"].as_str().unwrap(), "{c}");
                assert_eq!(
                    project_app(&clean).map_or(Value::Null, Value::from),
                    c["app"],
                    "{c}"
                );
                // The same name through the plan: the dest is the cleaned name beside the project, or the suffix's sentence.
                let root = &kept[0].2;
                let base = k7_file_service::paths::dirname(root);
                let home = Home {
                    studio: format!("{base}/studio"),
                    libraries: format!("{base}/libraries"),
                };
                let planned = plan_rename(&Space::open(&home, root), "a.k7", name, &Disk);
                match (c["app"].as_str(), planned) {
                    (None, Err(ProjError::Refused(said))) => {
                        assert!(said.contains(&format!("'{clean}' should end in")), "{said}")
                    }
                    (Some(_), Ok(None)) => assert_eq!(clean, "a.k7"),
                    (Some(_), Ok(Some(plan))) => {
                        assert_eq!(plan.dest, format!("{root}/{clean}"), "{name:?}")
                    }
                    (Some(_), Err(ProjError::Exists(rel))) => assert_eq!(rel, clean, "{name:?}"),
                    (app, other) => panic!("{name:?}: app {app:?} planned {other:?}"),
                }
            }
            "open" => {
                let rel = c["rel"].as_str().unwrap();
                if rel.contains("ln.k7") && !can_link(&s) {
                    continue;
                }
                // Windows reads a name of spaces as the folder itself, so the vector's "no such project" cannot hold.
                // A trailing space or dot is dropped by Windows too, so "a.k7 " names the project that exists.
                if cfg!(windows) && (rel.trim().is_empty() || rel.ends_with([' ', '.'])) {
                    continue;
                }
                let fixture = c["fixture"].as_str().unwrap();
                let root = &kept.iter().find(|(n, _, _)| n == fixture).unwrap().2;
                let base = k7_file_service::paths::dirname(root);
                let home = Home {
                    studio: format!("{base}/studio"),
                    libraries: format!("{base}/libraries"),
                };
                let sp = Space::open(&home, root);
                let got = match plan_open(&sp, rel, &Disk) {
                    Ok(plan) => json!({ "ok": plan.rel }),
                    Err(e) => json!({ "error": e.to_string() }),
                };
                assert_eq!(got, c["out"], "{fixture} {rel:?}");
            }
            other => panic!("a vector of a function the crate lacks: {other}"),
        }
    }
    assert_eq!(seen.len(), 5, "{seen:?}");
}
