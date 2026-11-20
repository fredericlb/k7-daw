// harness.rs — loads the 3e vectors, builds each one's folder in a MemFs, and compares what a call returned and changed.
// A reader would get wrong the placeholders: `$DIR`, `$TMP`, `$REPO`, `$UUID<n>` and `$STAMP` are rewritten on both sides alike.

use serde_json::Value as Json;
use song_edit::MemFs;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

static SIBLING: AtomicBool = AtomicBool::new(false);

/// Chooses where the replay's `$DIR` stands: under `$TMP` (a project in a temp folder) or beside it.
pub fn sibling(on: bool) {
    SIBLING.store(on, Ordering::Relaxed);
}

pub fn tmp() -> String {
    "/tmp/k7r-tmp".to_owned()
}

pub fn root() -> String {
    if SIBLING.load(Ordering::Relaxed) {
        "/tmp/k7r-dir".to_owned()
    } else {
        "/tmp/k7r-tmp/dir".to_owned()
    }
}
pub const STAMP: &str = "260101T000000";

pub fn repo() -> String {
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    here.join("../../..")
        .canonicalize()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default()
}

pub fn vectors_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../apps/k7/backend-rs/tests/vectors/song-edit")
}

pub fn uuid(n: usize) -> String {
    format!("00000000-0000-4000-8000-{n:012}")
}

/// A string with every placeholder made concrete.
pub fn concrete(s: &str) -> String {
    let mut out = s
        .replace("$DIR", &root())
        .replace("$TMP", &tmp())
        .replace("$REPO", &repo())
        .replace("$STAMP", STAMP);
    for n in (1..=64).rev() {
        out = out.replace(&format!("$UUID{n}"), &uuid(n));
    }
    out
}

/// `/tmp` and the segments after it, as the harvest's regex found a temp path.
fn scrub_tmp(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(at) = rest.find("/tmp") {
        out.push_str(&rest[..at]);
        let mut tail = &rest[at + 4..];
        while let Some(t) = tail.strip_prefix('/') {
            let n = t
                .find(|c: char| c == '/' || c == '"' || c.is_whitespace())
                .unwrap_or(t.len());
            tail = &t[n..];
        }
        out.push_str("$TMP");
        rest = tail;
    }
    out + rest
}

/// What a call produced, with its temp paths, ids and stamps given back their placeholder.
pub fn scrub(s: &str) -> String {
    let mut out = scrub_tmp(&s.replace(&root(), "$DIR")).replace(&repo(), "$REPO");
    for n in (1..=64).rev() {
        out = out.replace(&uuid(n), &format!("$UUID{n}"));
    }
    out.replace(STAMP, "$STAMP")
}

pub fn map_strings(v: &Json, f: &dyn Fn(&str) -> String) -> Json {
    match v {
        Json::String(s) => Json::String(f(s)),
        Json::Array(a) => Json::Array(a.iter().map(|x| map_strings(x, f)).collect()),
        Json::Object(o) => Json::Object(o.iter().map(|(k, x)| (f(k), map_strings(x, f))).collect()),
        other => other.clone(),
    }
}

/// JSON with sorted keys and every number as an f64, so equal values print equal.
pub fn canon(v: &Json) -> String {
    match v {
        Json::Object(o) => {
            let sorted: BTreeMap<&String, &Json> = o.iter().collect();
            let rows: Vec<String> = sorted
                .iter()
                .map(|(k, x)| format!("{}:{}", Json::String((*k).clone()), canon(x)))
                .collect();
            format!("{{{}}}", rows.join(","))
        }
        Json::Array(a) => format!("[{}]", a.iter().map(canon).collect::<Vec<_>>().join(",")),
        Json::Number(n) => format!("{}", n.as_f64().unwrap_or(f64::NAN)),
        other => other.to_string(),
    }
}

pub struct File {
    pub module: String,
    pub blobs: BTreeMap<String, Json>,
    pub trees: BTreeMap<String, Json>,
    pub vectors: Vec<Json>,
}

pub fn load(name: &str) -> Option<File> {
    let text = std::fs::read_to_string(vectors_dir().join(format!("{name}.json"))).ok()?;
    let j: Json = serde_json::from_str(&text).ok()?;
    let obj = |k: &str| -> BTreeMap<String, Json> {
        j[k].as_object()
            .map(|o| o.iter().map(|(a, b)| (a.clone(), b.clone())).collect())
            .unwrap_or_default()
    };
    Some(File {
        module: j["module"].as_str().unwrap_or("").to_owned(),
        blobs: obj("blobs"),
        trees: obj("trees"),
        vectors: j["vectors"].as_array().cloned().unwrap_or_default(),
    })
}

/// A blob's bytes: its text with placeholders made concrete, or that many zero bytes.
fn bytes_of(blob: &Json) -> Vec<u8> {
    match blob {
        Json::String(s) => concrete(s).into_bytes(),
        other => vec![0; other["size"].as_u64().unwrap_or(0) as usize],
    }
}

impl File {
    pub fn fs_of(&self, v: &Json) -> MemFs {
        let Some(tree) = v["tree"].as_str().and_then(|h| self.trees.get(h)) else {
            return MemFs::new();
        };
        let rows = tree.as_object().map(|o| {
            o.iter()
                .map(|(rel, h)| {
                    (
                        concrete(rel),
                        self.blobs
                            .get(h.as_str().unwrap_or(""))
                            .map(bytes_of)
                            .unwrap_or_default(),
                    )
                })
                .collect::<Vec<_>>()
        });
        let mut fs = MemFs::from_tree(&root(), rows.unwrap_or_default());
        let _ = song_edit::Fs::mkdir_all(&mut fs, &root());
        fs
    }

    /// The files the call changed, as the harvest records them: text, a size, or null for a removal.
    pub fn changes(&self, before: &MemFs, after: &MemFs, only: Option<&str>) -> BTreeMap<String, Json> {
        let text = |b: &[u8]| -> Json {
            match std::str::from_utf8(b) {
                Ok(t) if b.len() <= 65_536 && !b.contains(&0) => Json::String(scrub(t)),
                _ => serde_json::json!({ "size": b.len() }),
            }
        };
        let mut out = BTreeMap::new();
        let old: BTreeMap<&str, &[u8]> = before.files().collect();
        let new: BTreeMap<&str, &[u8]> = after.files().collect();
        let rel = |p: &str| scrub(p).strip_prefix("$DIR/").unwrap_or(p).to_owned();
        // A call on one file is recorded for that file alone: the harvest keeps its name and drops the folder's others.
        let kept = |p: &str| only.is_none_or(|o| rel(p) == o);
        for (p, b) in new.iter().filter(|(p, _)| kept(p)) {
            if old.get(p) != Some(b) {
                out.insert(rel(p), text(b));
            }
        }
        for p in old.keys().filter(|p| !new.contains_key(*p) && kept(p)) {
            out.insert(rel(p), Json::Null);
        }
        out
    }

    pub fn expected_changes(&self, v: &Json) -> Option<BTreeMap<String, Json>> {
        let c = v["changed"].as_object()?;
        Some(
            c.iter()
                .map(|(rel, h)| {
                    (
                        rel.clone(),
                        h.as_str()
                            .and_then(|h| self.blobs.get(h))
                            .cloned()
                            .unwrap_or(Json::Null),
                    )
                })
                .collect(),
        )
    }
}

/// What a module's replay tallied.
#[derive(Default)]
pub struct Tally {
    pub replayed: usize,
    pub skipped: BTreeMap<String, usize>,
    pub failures: Vec<String>,
}

pub fn report(module: &str, vectors: usize, t: &Tally) {
    eprintln!(
        "{module}: {vectors} vectors, {} replayed, skipped {:?}, {} failed",
        t.replayed,
        t.skipped,
        t.failures.len()
    );
    for f in t.failures.iter().take(40) {
        eprintln!("  FAIL {}", f.chars().take(420).collect::<String>());
    }
}
