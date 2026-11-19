//! The module reads replayed from the TypeScript's goldens (a real Workspace over a fixture folder): the answer or the sentence, the tree untouched.
//! What a reader gets wrong: the open project is the marker file the tree carries, found by `Space::active` as the route's runtime finds it.

use k7_file_service::libraries::Home;
use k7_file_service::module_reads::{
    Failure, Reading, consoles, module_bases, module_engines, module_ui,
};
use k7_file_service::space::Space;
use serde_json::{Value, json};

use crate::support::Scratch;

pub fn cases(name: &str) -> Vec<Value> {
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

/// The fixture folder of a case, and the home over it.
pub fn mounted(c: &Value, tag: &str) -> (Scratch, Home) {
    let label = c["name"].as_str().unwrap();
    let s = Scratch::new(&format!(
        "rg-{tag}-{}",
        label.replace(|c: char| !c.is_ascii_alphanumeric(), "_")
    ));
    for (k, v) in c["tree"].as_object().unwrap() {
        s.file(k, v.as_str().unwrap());
    }
    std::fs::create_dir_all(s.path("ws")).unwrap();
    let home = Home {
        studio: s.path("home"),
        libraries: s.path("libraries"),
    };
    (s, home)
}

pub fn code() -> String {
    format!("{}/../../../../..", env!("CARGO_MANIFEST_DIR"))
}

fn sentence(f: Failure) -> String {
    match f {
        Failure::Say(s) => s,
        Failure::Declined(d) => format!("declined: {}", d.as_str()),
    }
}

/// Compares a case's answer with the golden's `ret` or `thrown`.
pub fn check(c: &Value, s: &Scratch, got: Result<Value, String>) {
    let label = c["name"].as_str().unwrap();
    if let Some(msg) = c["thrown"].as_str() {
        assert_eq!(
            got.expect_err(label),
            msg.replace("$DIR", &s.path("")),
            "{label}"
        );
    } else {
        assert_eq!(
            got.unwrap_or_else(|e| panic!("{label}: {e}")),
            c["ret"],
            "{label}"
        );
    }
}

#[test]
fn the_module_reads_replay_the_typescript() {
    let all = cases("moduleReads");
    assert!(all.len() >= 30);
    for c in &all {
        let (s, home) = mounted(c, "mod");
        let root = s.path("ws");
        let active = Space::open(&home, &root).active().unwrap();
        let r = Reading {
            home: &home,
            code_root: &code(),
            root: &root,
            active: active.as_deref(),
        };
        let path = c["args"]["path"].as_str().unwrap_or("");
        let got = match c["fn"].as_str().unwrap() {
            "moduleUi" => module_ui(&r, path),
            "moduleBases" => module_bases(&r, path).map(|b| json!({ "bases": b })),
            "moduleEngines" => module_engines(&r, path),
            _ => consoles(&r),
        };
        check(c, &s, got.map_err(sentence));
    }
}

fn files_under(
    dir: &std::path::Path,
    base: &std::path::Path,
    out: &mut std::collections::BTreeMap<String, String>,
) {
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

/// The tree a case ends with: what it began with, then the files it changed or removed.
pub fn expected(c: &Value) -> std::collections::BTreeMap<String, String> {
    let mut out: std::collections::BTreeMap<String, String> = c["tree"]
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

pub fn tree_of(s: &Scratch) -> std::collections::BTreeMap<String, String> {
    let mut have = std::collections::BTreeMap::new();
    files_under(&s.0, &s.0, &mut have);
    have
}

#[test]
fn the_preferences_replay_the_typescript_files_included() {
    use k7_file_service::preferences::{update, view};
    let all = cases("preferences");
    assert!(all.len() >= 11);
    for c in &all {
        let label = c["name"].as_str().unwrap();
        let (s, home) = mounted(c, "pref");
        let arg = &c["args"]["arg"];
        let got = match c["fn"].as_str().unwrap() {
            "view" => Ok(view(&home)),
            "setVim" => update(&home, |p| p.editor_vim = arg.as_bool().unwrap())
                .map(|p| json!({ "vim": p.editor_vim })),
            "setStems" => update(&home, |p| {
                let t = arg.as_str().unwrap().trim();
                p.stems_chain = (!t.is_empty()).then(|| t.to_owned());
            })
            .map(|_| Value::Null),
            _ => update(&home, |p| {
                let t = arg.as_str().unwrap().trim();
                p.instrumental_chain = (!t.is_empty()).then(|| t.to_owned());
            })
            .map(|_| Value::Null),
        };
        if c["fn"].as_str().unwrap().starts_with("set") && c["fn"] != "setVim" {
            assert_eq!(got.is_err(), c["thrown"].is_string(), "{label}");
        } else {
            check(c, &s, got);
        }
        assert_eq!(tree_of(&s), expected(c), "{label}");
    }
}

#[test]
fn the_project_reads_replay_the_typescript() {
    use k7_file_service::confine::Disk;
    use k7_file_service::listings::engine::engine_scan;
    use k7_file_service::project_reads::{compat_of, plugin_needs, project_dir_of, resources};
    use k7_file_service::rig::rig;
    use k7_file_service::song_ask::{Input, song_roots};
    let all = cases("projectReads");
    assert!(all.len() >= 16);
    for c in &all {
        let label = c["name"].as_str().unwrap();
        let (s, home) = mounted(c, "proj");
        let root = s.path("ws");
        let space = Space::open(&home, &root);
        let env = std::collections::BTreeMap::new();
        let project = c["args"]["project"].as_str().unwrap_or("");
        let got: Result<Value, String> = (|| {
            let (dir, fresh) = if c["fn"] == "resources" {
                (
                    space
                        .active()
                        .unwrap()
                        .ok_or(())
                        .map_err(|()| String::new()),
                    true,
                )
            } else {
                (project_dir_of(&space, project).map_err(sentence), false)
            };
            let _ = fresh;
            let dir = match dir {
                Ok(d) => d,
                Err(e) if c["fn"] == "resources" && e.is_empty() => return Ok(json!([])),
                Err(e) => return Err(e),
            };
            let input = Input {
                home: &home,
                project: &dir,
                code_root: &code(),
                env: &env,
            };
            let roots = song_roots(&input, &Disk).map_err(|d| d.as_str().to_owned())?;
            let (dirs, files) = roots.mounts().map_err(|e| e.to_string())?;
            let disk = song::DiskSource::with_files(&dirs, &files).map_err(|e| e.to_string())?;
            let mut cache = song::Cache::new();
            if c["fn"] == "resources" {
                return Ok(Value::Array(resources(&disk, &mut cache, &space, &dir)));
            }
            let rig =
                song::lanes::rig::Rig::from_json(&rig(&home).map_err(|d| d.as_str().to_owned())?);
            let needs = plugin_needs(&disk, &mut cache, &roots, rig)?;
            if c["fn"] == "pluginNeeds" {
                return Ok(Value::Array(needs.iter().map(|n| n.json()).collect()));
            }
            let scan = engine_scan(&home).map_err(|d| d.as_str().to_owned())?;
            Ok(compat_of(&needs, &scan.plugins).unwrap_or(Value::Null))
        })();
        check(c, &s, got);
        assert_eq!(tree_of(&s), expected(c), "{label}");
    }
}
