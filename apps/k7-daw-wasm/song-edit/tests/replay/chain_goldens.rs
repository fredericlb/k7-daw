// chain_goldens.rs — replays rust/tests/goldens/chain.json: the chain project, tap chain, lane strip and preset verbs over folders the 3e tap never built.
// A reader would get wrong what is compared: the answer, the error's message and every file of the temp folder after, byte for byte.

use super::dispatch;
use super::dispatch::Outcome;
use super::harness::{canon, concrete, map_strings, root, scrub, tmp};
use serde_json::Value as Json;
use song_edit::{Fs, MemFs};
use std::collections::BTreeMap;

fn module_of(f: &str) -> &'static str {
    match f {
        "addTapChainNode" | "removeTapChainNode" | "moveTapChainNode" => "song-edit/tapChain",
        "stripNewLane" | "createPictureLane" => "song/laneStrip",
        "writeNodePreset" | "applyPresetToNode" | "listNodePresets" | "presetRefOf" | "presetFile" => {
            "song-edit/presets"
        }
        _ => "song-edit/graphEdit",
    }
}

fn folder(files: &Json) -> MemFs {
    let rows: Vec<(String, Vec<u8>)> = files
        .as_object()
        .map(|o| {
            o.iter()
                .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").as_bytes().to_vec()))
                .collect()
        })
        .unwrap_or_default();
    let mut fs = MemFs::from_tree(&tmp(), rows);
    let _ = fs.mkdir_all(&root());
    fs
}

fn files_of(fs: &MemFs, meta: &str) -> BTreeMap<String, String> {
    let prefix = format!("{}/", tmp());
    fs.files()
        .filter(|(p, _)| !p.starts_with(meta))
        .map(|(p, b)| {
            (
                p.strip_prefix(&prefix).unwrap_or(p).to_owned(),
                String::from_utf8_lossy(b).into_owned(),
            )
        })
        .collect()
}

#[test]
fn the_chain_goldens_replay() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/goldens/chain.json");
    let cases: Json = serde_json::from_str(&std::fs::read_to_string(path).unwrap_or_default()).unwrap_or(Json::Null);
    let cases = cases.as_array().cloned().unwrap_or_default();
    assert!(cases.len() >= 70, "the goldens are missing");
    let meta = super::dispatch::env().meta_root;
    let mut bad = vec![];
    for c in &cases {
        let name = c["name"].as_str().unwrap_or("");
        let mut fs = folder(&c["files"]);
        let args: Vec<Json> = c["args"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|a| map_strings(a, &|s| if s == "$DIR" { root() } else { concrete(s) }))
            .collect();
        let mut env = super::dispatch::env();
        env.overlays = c["overlays"]
            .as_array()
            .map(|o| {
                o.iter()
                    .filter_map(|n| n.as_str())
                    .map(|n| (n.to_owned(), format!("{}/home/libraries/{n}", tmp())))
                    .collect()
            })
            .unwrap_or_default();
        let f = c["fn"].as_str().unwrap_or("");
        let got = dispatch::call_with(module_of(f), f, &args, &mut fs, None, &env);
        let mut why = vec![];
        match got {
            None => why.push("not ported".to_owned()),
            Some(Outcome::Skip(w)) => why.push(format!("skipped: {w}")),
            Some(Outcome::Thrown(m)) => {
                let want = c["e"].as_str().unwrap_or("<no error>");
                if scrub(&m) != want {
                    why.push(format!("threw {m:?}, wanted {want:?}"));
                }
            }
            Some(Outcome::Ret(r)) => {
                if c.get("e").is_some() {
                    why.push(format!("returned {r}, wanted an error {}", c["e"]));
                } else if canon(&map_strings(&r, &scrub))
                    != canon(&map_strings(c.get("ret").unwrap_or(&Json::Null), &scrub))
                {
                    why.push(format!("returned {r}, wanted {}", c["ret"]));
                }
            }
        }
        let want: BTreeMap<String, String> = c["after"]
            .as_object()
            .map(|o| {
                o.iter()
                    .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_owned()))
                    .collect()
            })
            .unwrap_or_default();
        let have = files_of(&fs, &meta);
        if have != want {
            let diff: Vec<String> = want
                .keys()
                .chain(have.keys())
                .filter(|k| want.get(*k) != have.get(*k))
                .cloned()
                .collect();
            why.push(format!("files differ at {diff:?}"));
            if let Some(k) = diff.first() {
                why.push(format!("have {:?}\n want {:?}", have.get(k), want.get(k)));
            }
        }
        if !why.is_empty() {
            bad.push(format!("{name}: {}", why.join("; ")));
        }
    }
    assert!(bad.is_empty(), "{} goldens differ:\n{}", bad.len(), bad.join("\n"));
}
