//! bench/slot.rs — `slotOf`: a live chain compiled once into the document an engine keeps in its `slot` session.
//! What a reader gets wrong: the rig is staged in memory beside the bench and never landed, so the bench being tuned and the disk are left as they are (3j-5).

use std::collections::HashSet;

use serde_json::{Map, Value, json};
use song::doc::Vsts;
use song::graph::Folder;
use song::session::{Ask, Session, View};
use song::{Kind, Source};
use song_edit::Fs;
use song_edit::scaffold_live::scaffold_live_graph;
use unicode_normalization::UnicodeNormalization;

use super::splice::splice_chain;
use super::{BenchError, Libs, WORKSPACE, bench_dir, with_chains, workspace_root};
use crate::chains::refusal_at;
use crate::confine::Disk;
use crate::edits::StagedFs;
use crate::engine::docs::fold_json;
use crate::listings::families::library_names;
use crate::paths::{dirname, join};
use crate::song_ask::{Input, song_roots};
use crate::space::Declined;

/// The engine wire's version a slot is stored with (`ENGINE_PROTOCOL`).
const ENGINE_PROTOCOL: u32 = 1;
/// The most files a slot names besides its document: a sample library is not a chain's.
const MAX_ASSETS: usize = 10_000;

/// What the supervisor stores as one slot, as the route answers it.
pub type SlotFile = Value;

/// A slot's id: the chain's last segment, folded to what the supervisor takes as a folder name.
#[must_use]
pub fn slot_id_of(r: &str) -> String {
    let last = r.split('/').rfind(|s| !s.is_empty()).unwrap_or("");
    let mut id = String::new();
    let mut gap = true;
    let folded: String = last
        .nfd()
        .filter(|c| !('\u{300}'..='\u{36f}').contains(c))
        .collect::<String>()
        .to_lowercase();
    for c in folded.chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            id.push(c);
            gap = false;
        } else if !gap {
            id.push('-');
            gap = true;
        }
    }
    let id = id.trim_matches('-');
    let id: String = id.chars().take(64).collect();
    if id.is_empty() {
        "chain".to_owned()
    } else {
        id
    }
}

/// A whole number printed as JavaScript prints it: `120`, where a float prints `120.0`.
fn js_numbers(v: &mut Value) {
    match v {
        Value::Number(n) => {
            if let Some(f) = n
                .as_f64()
                .filter(|f| n.is_f64() && f.fract() == 0.0 && f.abs() < 9e15)
            {
                *n = serde_json::Number::from(f as i64);
            }
        }
        Value::Array(a) => a.iter_mut().for_each(js_numbers),
        Value::Object(o) => o.values_mut().for_each(js_numbers),
        _ => {}
    }
}

fn list(v: Option<&Value>) -> Vec<&Value> {
    v.and_then(Value::as_array)
        .map(|a| a.iter().collect())
        .unwrap_or_default()
}

/// The plugins of every track, then the master's; `master_first` flips it for the sample walk.
fn plugins_of(doc: &Value, master_first: bool) -> Vec<&Value> {
    let tracks: Vec<&Value> = list(doc.get("tracks"))
        .into_iter()
        .flat_map(|t| list(t.get("plugins")))
        .collect();
    let master = list(doc.get("masterPlugins"));
    if master_first {
        master.into_iter().chain(tracks).collect()
    } else {
        tracks.into_iter().chain(master).collect()
    }
}

/// The ids of the document's plugins whose node is one of `nodes`, an id being `<track>:<node>`.
fn chain_plugins(doc: &Value, nodes: &HashSet<&str>) -> Vec<String> {
    plugins_of(doc, false)
        .into_iter()
        .map(|p| p.get("id").and_then(Value::as_str).unwrap_or(""))
        .filter(|id| nodes.contains(id.find(':').map_or(*id, |at| &id[at + 1..])))
        .map(str::to_owned)
        .collect()
}

/// Every regular file beside each plain `.sfz` the document plays, which sfizz reads by the names inside it.
fn sfz_assets(fs: &StagedFs, doc: &Value) -> Result<Vec<String>, BenchError> {
    let mut dirs: Vec<String> = Vec::new();
    for p in plugins_of(doc, true) {
        let f = p.get("sfzFile").and_then(Value::as_str).unwrap_or("");
        if !f.is_empty() && !f.starts_with("@instruments/") {
            let d = dirname(f);
            if !dirs.contains(&d) {
                dirs.push(d);
            }
        }
    }
    let mut out = Vec::new();
    for d in dirs {
        let mut queue = vec![d.clone()];
        let mut at = 0;
        while at < queue.len() {
            let here = queue[at].clone();
            at += 1;
            // Listed through the stage: confined to its mounts, so a folder outside them is refused.
            let mut entries = fs.list(&here).map_err(|e| {
                BenchError::Refused(song_edit::error::io_error(&e, "scandir", &here).to_string())
            })?;
            entries.sort_by(|a, b| a.name.cmp(&b.name));
            for e in entries {
                let path = join(&[&here, &e.name]);
                // A link is neither followed nor shipped, as Node's `isFile` and `isDirectory` skip it.
                if fs.is_link(&path) {
                    continue;
                }
                if e.kind == Kind::Dir {
                    queue.push(path);
                } else {
                    if out.len() >= MAX_ASSETS {
                        return Err(BenchError::Refused(format!(
                            "the slot's samples pass {MAX_ASSETS} files"
                        )));
                    }
                    out.push(path);
                }
            }
        }
    }
    Ok(out)
}

/// The slot `ref_` compiles to: refused, saying why, for a chain that cannot be played live.
///
/// `rig` is the machine's preferences as the compile reads them, `saved_at` the ISO instant, `scratch` the staged folder's unique name.
///
/// # Errors
/// `Declined` where the route is refused, `Refused` with the sentence the route sends.
pub fn slot_of(
    libs: &Libs<'_>,
    rig: &Value,
    ref_: &str,
    saved_at: &str,
    scratch: &str,
) -> Result<SlotFile, BenchError> {
    let home = libs.home;
    // A machine with no library is refused with a sentence (4-7).
    if library_names(home).is_empty() {
        return Err(Declined::WouldThrow.into());
    }
    let root = workspace_root(home);
    std::fs::create_dir_all(&root).map_err(|e| BenchError::Refused(e.to_string()))?;
    // The bench's workspace is pinned afresh once, as `pinBench` does before any view of it.
    crate::pins::ensure_fresh(home, &root)?;
    let bench = bench_dir(home);
    // The bench's own rule of what plays live: a cold step needs the whole file first.
    with_chains(libs, &bench, |chains, _| {
        let Some(i) = (0..chains.dirs.len()).find(|&i| chains.dirs[i].ref_ == ref_) else {
            return Err(BenchError::Refused(format!(
                "no chain \"{ref_}\" in the libraries"
            )));
        };
        match refusal_at(chains, i)? {
            Some(why) => Err(BenchError::Refused(format!(
                "\"{ref_}\" cannot play live: {why}"
            ))),
            None => Ok(()),
        }
    })?;
    let dir = join(&[&root, &format!("slot-{scratch}")]);
    let input = Input {
        home,
        project: &dir,
        code_root: libs.code_root,
        env: libs.env,
    };
    let roots = song_roots(&input, &Disk)?;
    let (mounts, files) = roots.mounts().map_err(|_| Declined::WouldThrow)?;
    // The stage writes only into its own folder and reads the bench workspace besides the mounts.
    let mut reads: Vec<&str> = vec![root.as_str()];
    reads.extend(mounts.iter().map(String::as_str));
    let mut fs =
        StagedFs::over(&[dir.as_str()], &reads, &files).map_err(|_| Declined::WouldThrow)?;
    scaffold_live_graph(&mut fs, &dir, WORKSPACE)?;
    let ids = with_chains(libs, &bench, |chains, _| {
        splice_chain(&mut fs, &dir, chains, ref_)
    })?
    .ids;
    let rule = roots.paths;
    let ask = Ask {
        roots,
        rig: rig.clone(),
        vsts: Vsts::Scan,
        thawed: Vec::new(),
        rev: None,
        folder: Folder::Open,
    };
    let reply = Session::new(rule)
        .answer(&fs, &ask, &[View::Engine])
        .map_err(|e| BenchError::Refused(e.to_string()))?;
    let text = reply
        .views
        .into_iter()
        .find(|(v, _)| *v == View::Engine)
        .map(|(_, t)| t);
    let view: Value = text
        .and_then(|t| serde_json::from_str(&t).ok())
        .ok_or_else(|| BenchError::Refused("the slot's chain did not compile".to_owned()))?;
    let Some(Value::Object(doc)) = view.get("doc").cloned() else {
        return Err(BenchError::Refused(
            "the slot's chain did not compile".to_owned(),
        ));
    };
    let last = ref_.split('/').rfind(|s| !s.is_empty()).unwrap_or(ref_);
    let mut doc: Map<String, Value> = doc;
    doc.insert("name".into(), json!(format!("slot {last}")));
    let mut played = fold_json(&Value::Object(doc));
    js_numbers(&mut played);
    let nodes: HashSet<&str> = ids.keys().map(String::as_str).collect();
    let (assets, plugins) = (sfz_assets(&fs, &played)?, chain_plugins(&played, &nodes));
    Ok(json!({
        "id": slot_id_of(ref_),
        "name": last,
        "chain": ref_,
        "savedAt": saved_at,
        "protocol": ENGINE_PROTOCOL,
        "doc": played,
        "assets": assets,
        "chainPlugins": plugins,
    }))
}

// Links need no privilege on Unix only.
#[cfg(all(test, unix))]
mod tests {
    use super::*;

    // The sample walk lists a folder breadth first, skips links, and refuses a folder outside the stage's mounts.
    #[test]
    fn the_samples_are_the_regular_files_below_the_sfz_and_no_link() {
        let base = std::env::temp_dir().join(format!("k7-sfz-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (lib, other) = (base.join("lib/kit"), base.join("private"));
        std::fs::create_dir_all(lib.join("sub")).unwrap();
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(lib.join("kit.sfz"), "").unwrap();
        std::fs::write(lib.join("b.wav"), "").unwrap();
        std::fs::write(lib.join("sub/c.wav"), "").unwrap();
        std::fs::write(other.join("secret.wav"), "").unwrap();
        std::os::unix::fs::symlink(other.join("secret.wav"), lib.join("a.wav")).unwrap();
        std::os::unix::fs::symlink(&other, lib.join("up")).unwrap();
        let mount = base.join("lib").display().to_string();
        let fs = StagedFs::over(
            &[base.join("none").display().to_string()],
            &[mount],
            &[] as &[&str],
        )
        .unwrap();
        let sfz = lib.join("kit.sfz").display().to_string();
        let doc = json!({ "tracks": [{ "plugins": [{ "sfzFile": sfz }] }] });
        let got = sfz_assets(&fs, &doc).unwrap();
        let names: Vec<&str> = got.iter().map(|p| p.rsplit('/').next().unwrap()).collect();
        assert_eq!(names, ["b.wav", "kit.sfz", "c.wav"], "{got:?}");
        let out = json!({ "tracks": [{ "plugins": [{ "sfzFile": other.join("x.sfz").display().to_string() }] }] });
        assert!(sfz_assets(&fs, &out).is_err());
        let _ = std::fs::remove_dir_all(&base);
    }
}

#[cfg(test)]
mod slot_id_tests {
    use super::slot_id_of;

    // Each expectation is what `slotIdOf` answered in the TypeScript this module replaced.
    #[test]
    fn the_id_folds_accents_as_the_typescripts_nfd_does() {
        let cases = [
            ("lib/Việt-ễ x", "viet-e-x"),
            ("x/Łódź Ł-x", "odz-x"),
            ("a/Άλφα ΐ-ab", "ab"),
            ("a/한글 가각-ab", "ab"),
            ("a/Éclair-Ça/", "eclair-ca"),
            ("a/ǅ ﬁ Ⅻ-ab", "ab"),
            ("a/İstanbul-\u{212A}elvin", "istanbul-kelvin"),
            ("a/ａｂｃ-d", "d"),
            ("//", "chain"),
            ("a/ŁŁ", "chain"),
        ];
        for (r, want) in cases {
            assert_eq!(slot_id_of(r), want, "{r}");
        }
        assert_eq!(
            slot_id_of(&format!("a/{}-x", "ễ".repeat(70))),
            "e".repeat(64)
        );
    }
}
