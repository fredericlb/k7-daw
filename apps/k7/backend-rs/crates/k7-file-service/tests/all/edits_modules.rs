//! A library module moved or renamed, replayed from the TypeScript's goldens (`workspaceEntries.ts` through a real Workspace):
//! the plan, the folder's rename, the repoint, then the whole tree of files compared with the one the TypeScript left.

use std::collections::BTreeMap;

use k7_file_service::confine::Disk;
use k7_file_service::edits::module_make::{clone_module, create_module, create_node_of};
use k7_file_service::edits::modules::{move_answer, plan_library_move, repoint};
use k7_file_service::entries::plan::{answer_of, plan};
use k7_file_service::entries::{Op, Outcome};
use k7_file_service::libraries::{Home, library_of};
use k7_file_service::listings::engine::engine_catalogue;
use k7_file_service::rel::to_rel;
use k7_file_service::space::Space;
use serde_json::Value;

use crate::support::Scratch;

fn cases(name: &str) -> Vec<Value> {
    let p = format!(
        "{}/tests/goldens/server/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{p}: {e}"));
    serde_json::from_str::<Value>(&text).unwrap()["cases"]
        .as_array()
        .unwrap()
        .clone()
}

fn files_under(dir: &std::path::Path, base: &std::path::Path, out: &mut BTreeMap<String, String>) {
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            files_under(&p, base, out);
        } else {
            let rel = p
                .strip_prefix(base)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            out.insert(rel, std::fs::read_to_string(&p).unwrap());
        }
    }
}

fn expected(c: &Value) -> BTreeMap<String, String> {
    let mut out: BTreeMap<String, String> = c["tree"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_owned()))
        .collect();
    for (k, v) in c["changed"].as_object().unwrap() {
        match v.as_str() {
            Some(t) => out.insert(k.clone(), t.to_owned()),
            None => out.remove(k),
        };
    }
    out
}

fn sorted(mut v: Value) -> Value {
    if let Some(a) = v.get_mut("repointed").and_then(Value::as_array_mut) {
        a.sort_by(|x, y| x.as_str().cmp(&y.as_str()));
    }
    v
}

/// The two writes a route makes after its plan, done plainly: the folder renamed, then the refs repointed.
fn run(c: &Value, s: &Scratch) -> Result<Value, String> {
    let admin = c["args"]["admin"].as_bool().unwrap();
    let home = Home {
        studio: s.path("home"),
        libraries: s.path("libraries"),
        admin,
    };
    let root = s.path("ws");
    let space = Space::open(&home, &root);
    let arg = |k: &str| c["args"][k].as_str().unwrap().to_string();
    if c["fn"] == "createModule" {
        let code = format!("{}/../../../../..", env!("CARGO_MANIFEST_DIR"));
        let made = create_module(&home, &code, &space, &arg("root"), &arg("rel"));
        return made.map(|m| m.answer).map_err(|e| e.to_string());
    }
    if c["fn"] == "cloneModule" || c["fn"] == "createNode" {
        let code = format!("{}/../../../../..", env!("CARGO_MANIFEST_DIR"));
        let made = if c["fn"] == "cloneModule" {
            clone_module(&home, &code, &space, &arg("path"), &arg("newName"))
        } else {
            let catalogue = engine_catalogue(&home).unwrap();
            let mut n = 0;
            let mut mint = || {
                n += 1;
                format!("00000000-0000-4000-8000-{n:012}")
            };
            let node = c["args"]["node"].as_str();
            create_node_of(
                &home,
                &code,
                &space,
                &arg("path"),
                node,
                &catalogue,
                &mut mint,
            )
        };
        return made.map(|m| m.answer).map_err(|e| e.to_string());
    }
    if c["fn"] == "libraryMove" {
        let plan =
            plan_library_move(&space, &arg("path"), &arg("toDir")).map_err(|e| e.to_string())?;
        std::fs::rename(&plan.from_abs, &plan.dest).unwrap();
        let roots = [plan.root.clone(), root.clone()];
        let files = repoint(&roots, &plan.from, &plan.to, false)?;
        return Ok(move_answer(&space, &plan, &files));
    }
    let path = arg("path");
    let name = arg("name");
    let done = plan(
        &space,
        &Op::Rename {
            path: &path,
            name: &name,
        },
        &Disk,
    )
    .map_err(|e| e.to_string())?;
    let mut body = answer_of(&space, &done);
    if let Outcome::Rename { from, to, refs } = &done {
        let lib = library_of(&home, from).map(|l| l.root);
        std::fs::rename(from, to).unwrap();
        if let (Some((a, b, under)), Some(lib)) = (refs, lib) {
            let files = repoint(&[lib, root.clone()], a, b, *under)?;
            if !files.is_empty() {
                body["repointed"] = files
                    .iter()
                    .map(|f| to_rel(&space, f))
                    .collect::<Vec<_>>()
                    .into();
            }
        }
    }
    Ok(body)
}

fn replay(name: &str) {
    let all = cases(name);
    assert!(all.len() >= 8);
    for c in &all {
        let label = c["name"].as_str().unwrap();
        let s = Scratch::new(&format!(
            "em-{name}-{}",
            label.replace(|c: char| !c.is_ascii_alphanumeric(), "_")
        ));
        for (k, v) in c["tree"].as_object().unwrap() {
            s.file(k, v.as_str().unwrap());
        }
        std::fs::create_dir_all(s.path("ws")).unwrap();
        let got = run(c, &s).map(sorted);
        if let Some(msg) = c["thrown"].as_str() {
            assert_eq!(
                got.expect_err(label),
                msg.replace("$DIR", &s.path("")),
                "{label}"
            );
        } else {
            assert_eq!(
                got.unwrap_or_else(|e| panic!("{label}: {e}")),
                sorted(c["ret"].clone()),
                "{label}"
            );
        }
        let mut have = BTreeMap::new();
        files_under(&s.0, &s.0, &mut have);
        assert_eq!(have, expected(c), "{label}");
    }
}

#[test]
fn a_library_move_replays_the_typescript() {
    replay("libraryMove");
}

#[test]
fn a_rename_in_a_library_repoints_as_the_typescript_does() {
    replay("repoint");
}

#[test]
fn a_module_made_replays_the_typescript() {
    replay("modules");
}

#[test]
fn a_node_made_replays_the_typescript() {
    replay("nodes");
}

#[test]
fn a_module_cloned_replays_the_typescript() {
    replay("clones");
}

// A clone reads the workspace alone: a link to another library's file, or to `..`, is refused and nothing lands.
#[cfg(unix)]
#[test]
fn a_clone_refuses_links() {
    let code = format!("{}/../../../../..", env!("CARGO_MANIFEST_DIR"));
    for (label, target) in [("file", "libraries/private/x.wav"), ("up", "..")] {
        let s = Scratch::new(&format!("em-clone-link-{label}"));
        s.file("libraries/private/x.wav", "secret");
        s.file(
            "home/workspace/songs/_modules/audio/verb/module.k7x",
            "extends(\"@engine/reverb\")\n",
        );
        s.file(
            "home/workspace/songs/p.k7/project.k7x",
            "project({ name: \"p\" })\n",
        );
        s.file("home/workspace/.active-projects/songs", "p.k7");
        let module = s.path("home/workspace/songs/_modules/audio/verb");
        std::os::unix::fs::symlink(s.path(target), format!("{module}/ir")).unwrap();
        let home = Home {
            studio: s.path("home"),
            libraries: s.path("libraries"),
            admin: false,
        };
        let space = Space::open(&home, &s.path("home/workspace/songs"));
        let before = std::fs::read_dir(s.path("home/workspace/songs/_modules/audio"))
            .unwrap()
            .count();
        let made = clone_module(&home, &code, &space, "_modules/audio/verb", "copy");
        let err = made.map(|m| m.answer).expect_err(label).to_string();
        assert!(err.contains("a link"), "{label}: {err}");
        let after = std::fs::read_dir(s.path("home/workspace/songs/_modules/audio"))
            .unwrap()
            .count();
        assert_eq!(before, after, "{label}: a clone landed");
    }
}

#[test]
fn a_nodes_presets_replay_the_typescript() {
    use k7_file_service::edits::presets::node_presets;
    let all = cases("presets");
    assert!(all.len() >= 8);
    for c in &all {
        let label = c["name"].as_str().unwrap();
        let s = Scratch::new(&format!("pr-{}", label.len()));
        for (k, v) in c["tree"].as_object().unwrap() {
            s.file(k, v.as_str().unwrap());
        }
        let home = Home {
            studio: s.path("home"),
            libraries: s.path("libraries"),
            admin: false,
        };
        let code = format!("{}/../../../../..", env!("CARGO_MANIFEST_DIR"));
        let got = node_presets(
            &home,
            &code,
            &s.path("ws/song.k7"),
            c["args"]["node"].as_str().unwrap(),
        );
        match c["thrown"].as_str() {
            Some(msg) => assert_eq!(got.expect_err(label), msg, "{label}"),
            None => assert_eq!(
                got.unwrap_or_else(|e| panic!("{label}: {e}")),
                c["ret"],
                "{label}"
            ),
        }
    }
}

fn created(c: &Value, s: &Scratch) -> Result<Value, String> {
    use k7_file_service::edits::project_create::create_project;
    let home = Home {
        studio: s.path("home"),
        libraries: s.path("libraries"),
        admin: false,
    };
    let root = s.path(c["args"]["workspace"].as_str().unwrap());
    let space = Space::open(&home, &root);
    let code = k7_file_service::paths::resolve_all(&[env!("CARGO_MANIFEST_DIR"), "../../../../.."]);
    let mut n = 0;
    let mut mint = || {
        n += 1;
        format!("00000000-0000-4000-8000-{n:012}")
    };
    let a = |k: &str| c["args"][k].as_str().unwrap().to_owned();
    create_project(&home, &code, &space, &a("shape"), &a("name"), &mut mint)
        .map(|m| m.answer)
        .map_err(|e| e.to_string())
}

#[test]
fn a_project_created_replays_the_typescript() {
    let all = cases("projects");
    assert!(all.len() >= 12);
    for c in &all {
        let label = c["name"].as_str().unwrap();
        let s = Scratch::new(&format!("pc-{}", label.len()));
        for (k, v) in c["tree"].as_object().unwrap() {
            s.file(k, v.as_str().unwrap());
        }
        let got = created(c, &s);
        match c["thrown"].as_str() {
            Some(msg) => assert_eq!(
                got.expect_err(label),
                msg.replace("$DIR", &s.path("")),
                "{label}"
            ),
            None => assert_eq!(
                got.unwrap_or_else(|e| panic!("{label}: {e}")),
                c["ret"],
                "{label}"
            ),
        }
        let mut have = BTreeMap::new();
        files_under(&s.0, &s.0, &mut have);
        assert_eq!(have, expected(c), "{label}");
    }
}
