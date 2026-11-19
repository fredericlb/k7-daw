//! The rename's and the recycle's guards, `defaultProject`, `projectInfoOf` and markers that are not UTF-8, against the
//! TypeScript's own answers (W4): every case of `vectors/projects_guards.json` (made by `js/makeGuardVectors.mts`).

use crate::support;

use k7_file_service::confine::Disk;
use k7_file_service::projects::ProjError;
use k7_file_service::projects::info::project_info;
use k7_file_service::projects::life::{kept_answer, plan_recycle, plan_rename};
use k7_file_service::projects::marker::{marker_of, read_raw, write};
use k7_file_service::projects::open::default_project;
use k7_file_service::rel::to_rel;
use k7_file_service::space::Space;
use k7_file_service::tree::Limits;
use serde_json::{Value, json};
use support::Scratch;

fn vectors() -> Vec<Value> {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/vectors/projects_guards.json"
    ))
    .unwrap();
    serde_json::from_str(&text).unwrap()
}

fn unhex(h: &str) -> Vec<u8> {
    (0..h.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&h[i..i + 2], 16).unwrap())
        .collect()
}

/// A tree from a vector's entries: a path ending in `/` is a folder, any other a file.
fn build(s: &Scratch, entries: &[Value]) {
    for e in entries {
        let (p, t) = (e[0].as_str().unwrap(), e[1].as_str().unwrap());
        if p.ends_with('/') {
            s.dir(p);
        } else {
            s.file(p, t);
        }
    }
}

/// The marker as it was left: removed, or `rel` written by the crate's own writer.
fn mark(root: &str, active: Option<&str>) {
    let _ = std::fs::remove_file(marker_of(root));
    if let Some(rel) = active {
        write(root, rel).unwrap();
    }
}

fn answer_of(body: &str) -> Value {
    serde_json::from_str(body).unwrap()
}

#[test]
fn replays_every_guard_of_a_rename_and_a_recycle_in_the_typescripts_order() {
    let all = vectors();
    let fixture = all.iter().find(|c| c["fn"] == "fixture").unwrap();
    let s = Scratch::new("pg-move");
    build(&s, fixture["entries"].as_array().unwrap());
    let root = s.path("ws");
    let home = s.home();
    let (mut renames, mut recycles, mut refused) = (0, 0, 0);
    for c in &all {
        let (f, rel) = (c["fn"].as_str().unwrap(), c["rel"].as_str().unwrap_or(""));
        if f != "rename" && f != "recycle" {
            continue;
        }
        let active = c["active"].as_str();
        mark(&root, active);
        let sp = Space::open(&home, &root);
        let want = &c["out"];
        // The sentence or the answer, then the open project after the move, as the TypeScript's `workspaceInfo` says.
        let (got, after) = if f == "rename" {
            let name = c["name"].as_str().unwrap();
            renames += 1;
            match plan_rename(&sp, rel, name, &Disk) {
                Ok(None) => (
                    Ok(answer_of(&kept_answer(&sp, rel, &Disk).unwrap())),
                    active.map(String::from),
                ),
                Ok(Some(plan)) => (
                    Ok(answer_of(&plan.answer)),
                    match (plan.was_active, &plan.active_to) {
                        (true, Some(to)) => Some(to_rel(&sp, to)),
                        _ => active.map(String::from),
                    },
                ),
                Err(e) => (Err(e), active.map(String::from)),
            }
        } else {
            recycles += 1;
            match plan_recycle(&sp, rel, &Disk) {
                Ok(plan) => (
                    Ok(answer_of(&plan.answer)),
                    (!plan.was_active)
                        .then(|| active.map(String::from))
                        .flatten(),
                ),
                Err(e) => (Err(e), active.map(String::from)),
            }
        };
        match got {
            Ok(body) => {
                assert_eq!(&body, &want["ok"], "{c}");
                assert_eq!(json!(after), want["after"], "{c}");
            }
            Err(e) => {
                refused += 1;
                assert_eq!(json!(e.to_string()), want["error"], "{c}");
                assert!(!matches!(e, ProjError::Declined(_)), "{c}");
            }
        }
    }
    assert!(
        renames > 250 && recycles > 25 && refused > 150,
        "{renames} {recycles} {refused}"
    );
}

#[test]
fn replays_defaultproject_over_markers_of_every_shape_and_every_byte_the_typescript_decodes() {
    let all = vectors();
    let mut seen = 0;
    for c in all.iter().filter(|c| c["fn"] == "default") {
        let fx = c["fixture"].as_str().unwrap();
        let s = Scratch::new(&format!("pg-def-{}", fx.replace(' ', "-")));
        // The fixtures other than the first are small enough to spell here, the first one from the vector's own list.
        let fixture = all.iter().find(|c| c["fn"] == "fixture").unwrap();
        let root = match fx {
            "projects" => {
                build(&s, fixture["entries"].as_array().unwrap());
                s.path("ws")
            }
            "none" => {
                s.file("ws/notes/n.txt", "x");
                s.path("ws")
            }
            "empty" => s.dir("ws"),
            "root is a project" => {
                s.file("r.k7/project.k7x", "x");
                s.file("r.k7/x.k7/project.k7x", "x");
                s.path("r.k7")
            }
            "hidden first" => {
                for p in [
                    "ws/zeta.k7/project.k7x",
                    "ws/.hid.k7/project.k7x",
                    "ws/alpha/b.k7/project.k7x",
                    "ws/_recycle_bin/a.k7/project.k7x",
                ] {
                    s.file(p, "x");
                }
                s.path("ws")
            }
            other => panic!("unknown fixture {other}"),
        };
        let _ = std::fs::remove_file(marker_of(&root));
        if let Some(h) = c["hex"].as_str() {
            std::fs::create_dir_all(k7_file_service::paths::dirname(&marker_of(&root))).unwrap();
            std::fs::write(marker_of(&root), unhex(h)).unwrap();
        }
        let home = s.home();
        let got = default_project(&Space::open(&home, &root), &Limits::default());
        assert_eq!(
            got.map(|o| o.map_or(Value::Null, Value::from)),
            Ok(c["out"].clone()),
            "{c}"
        );
        seen += 1;
    }
    assert!(seen > 100, "{seen}");
}

#[test]
fn reads_a_marker_that_is_not_utf8_as_the_typescript_decodes_it() {
    let s = Scratch::new("pg-bytes");
    let root = s.path("m/workspace/ws");
    let mut seen = 0;
    for c in vectors().iter().filter(|c| c["fn"] == "read_bytes") {
        std::fs::create_dir_all(k7_file_service::paths::dirname(&marker_of(&root))).unwrap();
        std::fs::write(marker_of(&root), unhex(c["hex"].as_str().unwrap())).unwrap();
        let got = read_raw(&root)
            .unwrap()
            .map(|abs| abs.replace(&root, "<root>"));
        assert_eq!(got.map_or(Value::Null, Value::from), c["out"], "{c}");
        seen += 1;
    }
    assert!(seen >= 10, "{seen}");
}

#[test]
fn names_a_project_as_projectinfoof_does_or_declines_a_text_it_cannot_keep() {
    let (mut named, mut declined) = (0, 0);
    for (i, c) in vectors().iter().filter(|c| c["fn"] == "info").enumerate() {
        let s = Scratch::new(&format!("pg-info-{i}"));
        let root = s.dir("ws");
        s.file("ws/n.txt", "x");
        if let Some(text) = c["text"].as_str() {
            s.file("ws/a.k7/project.k7x", text);
        } else {
            s.dir("ws/a.k7");
        }
        mark(&root, Some("a.k7"));
        let home = s.home();
        match project_info(&Space::open(&home, &root)) {
            Ok(body) => {
                named += 1;
                assert_eq!(answer_of(&body)["name"], c["name"], "{c}");
            }
            Err(e) => {
                declined += 1;
                assert_eq!(c["lossy"], json!(true), "{c}: {e:?}");
            }
        }
    }
    assert!(named >= 20 && declined == 2, "{named} {declined}");
}
