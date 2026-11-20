// server_goldens_modules.rs — replays the goldens of moduleGoldens.mts: node and clone writers over the same tree, the same files after.
// A reader would get wrong that a failure compares the message only: the TypeScript leaves what it wrote before it threw.

use serde_json::Value as Json;
use song::modules::{Scan, ScannedPlugin, engine_modules};
use song_edit::MemFs;
use song_edit::confine::Env;
use song_edit::instance_name::{instance_name, name_part};
use song_edit::project_files::{clone_module_file, create_node, free_node_name};
use std::collections::BTreeMap;

const ROOT: &str = "/w";

fn cases() -> Vec<Json> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/goldens/server/projectFiles.json");
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

fn scan_of(c: &Json) -> Scan {
    let plugins = c["args"]["scan"].as_array().map_or_else(Vec::new, |a| {
        a.iter()
            .map(|p| ScannedPlugin {
                name: p["name"].as_str().unwrap().to_owned(),
                format: p["format"].as_str().unwrap().to_owned(),
                id: None,
                manufacturer: None,
                role: p["role"].as_str().map(str::to_owned),
                instrument: p["instrument"] == Json::Bool(true),
                category: None,
            })
            .collect()
    });
    Scan::from_plugins(plugins)
}

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

fn str_arg<'a>(c: &'a Json, k: &str) -> &'a str {
    c["args"][k].as_str().unwrap()
}

#[test]
fn project_file_writers_replay_the_typescript() {
    let all = cases();
    assert!(all.len() >= 50);
    let env = Env::at(&format!("{ROOT}/home"), &format!("{ROOT}/meta"));
    for c in &all {
        let name = c["name"].as_str().unwrap();
        let mut fs = mem(c);
        let dir = format!("{ROOT}/{}", if name.contains("template") { "t.k7" } else { "p.k7" });
        let mut n = 0;
        let mut new_id = || {
            n += 1;
            format!("00000000-0000-4000-8000-{n:012}")
        };
        let got: Result<Json, String> = match c["fn"].as_str().unwrap() {
            "createNode" => {
                let cat = engine_modules(&scan_of(c));
                create_node(
                    &mut fs,
                    &env,
                    &dir,
                    str_arg(c, "category"),
                    str_arg(c, "module"),
                    str_arg(c, "node"),
                    &cat,
                    &mut new_id,
                )
                .map(Json::from)
                .map_err(|e| e.to_string())
            }
            "freeNodeName" => {
                Ok(free_node_name(&fs, &env, &dir, str_arg(c, "module"), c["args"]["lane"].as_str()).into())
            }
            "cloneModuleFile" => clone_module_file(
                &mut fs,
                &dir,
                str_arg(c, "category"),
                str_arg(c, "name"),
                str_arg(c, "newName"),
            )
            .map(Json::from)
            .map_err(|e| e.to_string()),
            "namePart" => Ok(name_part(str_arg(c, "text")).into()),
            "instanceName" => {
                let taken: Vec<&str> = c["args"]["taken"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|t| t.as_str().unwrap())
                    .collect();
                Ok(instance_name(str_arg(c, "what"), c["args"]["owner"].as_str(), &|n| taken.contains(&n)).into())
            }
            other => panic!("{name}: {other}"),
        };
        if let Some(msg) = c["thrown"].as_str() {
            assert_eq!(got.expect_err(name), msg.replace("$DIR", ROOT), "{name}");
            continue;
        }
        assert_eq!(got.unwrap_or_else(|e| panic!("{name}: {e}")), c["ret"], "{name}");
        let have: BTreeMap<String, String> = fs
            .files()
            .map(|(p, b)| (p.to_owned(), String::from_utf8_lossy(b).into_owned()))
            .collect();
        assert_eq!(have, expected_tree(c), "{name}");
    }
}
