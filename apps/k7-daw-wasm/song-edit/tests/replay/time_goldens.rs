// time_goldens.rs — replays rust/tests/goldens/time.json: the time-side writers over folders the 3e tap never built, from the TypeScript.
// A reader would get wrong what is compared: the answer, the error's message and every file of the folder after, byte for byte.

use super::dispatch::{self, Outcome};
use super::harness::{canon, concrete, map_strings, root, scrub};
use super::recfs::RecFs;
use super::shape::outcome;
use serde_json::Value as Json;
use song_edit::facade::{self, Ctx};
use song_edit::{Fs, MemFs};
use std::collections::BTreeMap;

fn folder(files: &Json) -> MemFs {
    let rows: Vec<(String, Vec<u8>)> = files
        .as_object()
        .map(|o| {
            o.iter()
                .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").as_bytes().to_vec()))
                .collect()
        })
        .unwrap_or_default();
    let mut fs = MemFs::from_tree(&root(), rows);
    let _ = fs.mkdir_all(&root());
    fs
}

fn after(fs: &MemFs) -> BTreeMap<String, String> {
    let prefix = format!("{}/", root());
    fs.files()
        .map(|(p, b)| {
            (
                p.strip_prefix(&prefix).unwrap_or(p).to_owned(),
                String::from_utf8_lossy(b).into_owned(),
            )
        })
        .collect()
}

#[test]
fn the_time_goldens_replay() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/goldens/time.json");
    let cases: Json = serde_json::from_str(&std::fs::read_to_string(path).unwrap_or_default()).unwrap_or(Json::Null);
    let cases = cases.as_array().cloned().unwrap_or_default();
    assert!(cases.len() >= 80, "the goldens are missing");
    let mut bad = vec![];
    for c in &cases {
        let name = c["name"].as_str().unwrap_or("");
        let mut fs = folder(&c["files"]);
        let args: Vec<Json> = c["args"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter(|a| a.as_str() != Some("$snap"))
            .map(|a| map_strings(a, &|s| if s == "$DIR" { root() } else { concrete(s) }))
            .collect();
        let f = c["fn"].as_str().unwrap_or("");
        let env = dispatch::env();
        let mut ids = || String::new();
        let mut rec = RecFs {
            inner: &mut fs,
            writes: vec![],
        };
        let got = dispatch::time_name(f).map(|name| {
            let mut cx = Ctx {
                fs: &mut rec,
                env: &env,
                new_id: &mut ids,
            };
            outcome(facade::run_named(&mut cx, &name, &args))
        });
        let snap = rec.writes;
        let mut why = vec![];
        match got {
            None => why.push("not ported".to_owned()),
            Some(Outcome::Thrown(m)) => {
                let want = c["e"].as_str().unwrap_or("<no error>");
                if scrub(&m) != want && !(c["approx"] == true && scrub(&m).lines().next() == want.lines().next()) {
                    why.push(format!("threw {m:?}, wanted {want:?}"));
                }
            }
            Some(Outcome::Ret(r)) => {
                if c.get("e").is_some() {
                    why.push(format!("returned {r}, wanted an error {}", c["e"]));
                } else if canon(&map_strings(&r, &scrub)) != canon(&c["ret"]) {
                    why.push(format!("returned {r}, wanted {}", c["ret"]));
                }
            }
            Some(Outcome::Skip(w)) => why.push(format!("skipped: {w}")),
        }
        let want: BTreeMap<String, String> = c["after"]
            .as_object()
            .map(|o| {
                o.iter()
                    .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_owned()))
                    .collect()
            })
            .unwrap_or_default();
        let have = after(&fs);
        for (k, v) in &want {
            if have.get(k) != Some(v) {
                why.push(format!("{k} is {:?}, wanted {v:?}", have.get(k)));
            }
        }
        for k in have.keys().filter(|k| !want.contains_key(*k)) {
            why.push(format!("{k} should not exist"));
        }
        if let Some(want_snap) = c["snap"].as_array() {
            let want_snap: Vec<String> = want_snap.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect();
            let have_snap: Vec<String> = snap
                .iter()
                .map(|p| p.strip_prefix(&format!("{}/", root())).unwrap_or(p).to_owned())
                .collect();
            if have_snap != want_snap {
                why.push(format!("snapshot saw {have_snap:?}, wanted {want_snap:?}"));
            }
        }
        if !why.is_empty() {
            bad.push(format!("{name}: {}", why.join("; ")));
        }
    }
    eprintln!("time goldens: {} cases, {} differ", cases.len(), bad.len());
    for b in bad.iter().take(30) {
        eprintln!("  DIFF {b}");
    }
    assert!(bad.is_empty(), "{} goldens differ", bad.len());
}
