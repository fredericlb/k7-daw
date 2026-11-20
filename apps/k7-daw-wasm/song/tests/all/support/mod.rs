// support/mod.rs — what the integration tests share: a temporary folder, a bundle made from files, a project helper.
// A reader would get wrong that the folder is spelled as the Source spells it: real path, `/` separators.

#![allow(dead_code)]

mod world;
#[allow(unused_imports)]
pub use world::*;

use song::{BundleFile, DiskSource, Entry, Kind, Library, MemSource, Roots, SourceBundle};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

static COUNTER: AtomicU32 = AtomicU32::new(0);
static SKIPS: AtomicUsize = AtomicUsize::new(0);

/// A check this platform cannot make: named on stderr and counted, never a silent return.
pub fn skip(test: &str, why: &str) {
    SKIPS.fetch_add(1, Ordering::SeqCst);
    eprintln!("SKIPPED {test}: {why}");
}

/// How many checks were skipped so far in this test binary.
pub fn skipped() -> usize {
    SKIPS.load(Ordering::SeqCst)
}

pub struct Tmp(pub PathBuf);

/// A path in the Source's spelling.
pub fn spell(p: &Path) -> String {
    let s = p.to_string_lossy().into_owned();
    let s = s.strip_prefix(r"\\?\").unwrap_or(&s).to_owned();
    if cfg!(windows) { s.replace('\\', "/") } else { s }
}

impl Tmp {
    pub fn new(tag: &str) -> Tmp {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("song-{tag}-{}-{n}", std::process::id()));
        fs::create_dir_all(&dir).expect("create the temporary folder");
        Tmp(fs::canonicalize(&dir).expect("canonicalize the temporary folder"))
    }

    /// An absolute path under the folder.
    pub fn at(&self, rel: &str) -> String {
        format!("{}/{rel}", spell(&self.0))
    }

    pub fn write(&self, rel: &str, text: &str) {
        let path = self.0.join(rel);
        fs::create_dir_all(path.parent().expect("a parent")).expect("create the folder");
        fs::write(path, text).expect("write the file");
    }

    pub fn mkdir(&self, rel: &str) {
        fs::create_dir_all(self.0.join(rel)).expect("create the folder");
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A memory source over `/p`, holding `files` (path under `/p`, text) and a listing for every folder on their way.
pub fn mem(files: &[(&str, &str)]) -> MemSource {
    mem_at("/p", files)
}

/// `mem` with no root file added: a folder that is no project (811).
pub fn mem_bare(files: &[(&str, &str)]) -> MemSource {
    mem_unrooted("/p", files)
}

/// The same over another root, `C:/p` for a Windows world.
pub fn mem_at(root: &str, files: &[(&str, &str)]) -> MemSource {
    // A project folder holds its root file (811): a world that names none gets an empty one, which says nothing.
    let rooted = files.iter().any(|(rel, _)| rel.starts_with("ws/proj/"))
        && !files.iter().any(|(rel, _)| *rel == "ws/proj/project.k7x");
    let added = [("ws/proj/project.k7x", "")];
    let files: Vec<(&str, &str)> = files
        .iter()
        .copied()
        .chain(added.iter().copied().filter(|_| rooted))
        .collect();
    mem_unrooted(root, &files)
}

fn mem_unrooted(root: &str, files: &[(&str, &str)]) -> MemSource {
    let mut held = BTreeMap::new();
    let mut lists: BTreeMap<String, BTreeMap<String, Kind>> = BTreeMap::new();
    lists.entry(root.to_owned()).or_default();
    for (rel, text) in files {
        held.insert(
            format!("{root}/{rel}"),
            BundleFile {
                text: Some((*text).to_owned()),
                len: None,
                ..Default::default()
            },
        );
        let parts: Vec<&str> = rel.split('/').collect();
        let mut dir = root.to_owned();
        for (i, part) in parts.iter().enumerate() {
            let kind = if i + 1 == parts.len() { Kind::File } else { Kind::Dir };
            lists.entry(dir.clone()).or_default().insert((*part).to_owned(), kind);
            dir = format!("{dir}/{part}");
            if kind == Kind::Dir {
                lists.entry(dir.clone()).or_default();
            }
        }
    }
    let lists = lists
        .into_iter()
        .map(|(d, es)| (d, es.into_iter().map(|(name, kind)| Entry { name, kind }).collect()))
        .collect();
    let bundle = SourceBundle {
        format: 1,
        mounts: vec![root.to_owned()],
        files: held,
        lists,
        ..Default::default()
    };
    MemSource::from_bundle(bundle).expect("a valid bundle")
}

use serde_json::{Map, Value, json};
use song::{Cache, Clip, FileValue, Sequence, Source, SourceError, Timeline, read_family, read_head};

/// What the readers answer for one project, in the shape `scripts/stageDump.ts` writes for the TypeScript.
pub struct Dump {
    pub json: Value,
    pub compile_findings: usize,
}

fn family<T: FileValue + serde::Serialize>(
    source: &dyn Source,
    cache: &mut Cache,
    project: &str,
) -> Result<Value, SourceError> {
    let fam = read_family::<T>(source, cache, project)?;
    let values: Map<String, Value> = fam
        .values
        .iter()
        .map(|(id, v)| (id.clone(), serde_json::to_value(&**v).expect("json")))
        .collect();
    let mut files: Vec<String> = fam.findings.iter().map(|f| f.file.clone()).collect();
    files.sort();
    files.dedup();
    Ok(json!({ "values": values, "files": files }))
}

/// Every family and the head of `project` read through `source`, with a cache of its own.
pub fn dump(source: &dyn Source, project: &str) -> Result<Dump, SourceError> {
    let mut cache = Cache::new();
    let mut families = Map::new();
    families.insert("clips".into(), family::<Clip>(source, &mut cache, project)?);
    families.insert("sequences".into(), family::<Sequence>(source, &mut cache, project)?);
    families.insert("timelines".into(), family::<Timeline>(source, &mut cache, project)?);
    let h = read_head(source, &mut cache, project)?;
    let in_file = |name: &str| h.findings.iter().any(|f| f.file == name);
    let root = h.root.as_ref().and_then(|r| r.project.as_ref());
    let head = json!({
        "format_too_new": h.format_too_new,
        "root_ok": h.format_too_new || !in_file("project.k7x"),
        "root": root.map_or_else(|| json!({}), |p| serde_json::to_value(p).expect("json")),
        "markers": h.markers,
        "settings_ok": !in_file("settings.k7x"),
        "settings": h.settings,
    });
    Ok(Dump {
        json: json!({ "families": families, "head": head }),
        compile_findings: h.compile_findings.len(),
    })
}

/// One known-bad (or known-good) project of tests/negative.json and the findings it must give.
pub struct Case {
    pub name: String,
    pub category: String,
    pub ts: bool,
    pub mem: bool,
    pub module: bool,
    pub resolve: bool,
    pub routing: bool,
    pub workspace_namespace: Option<String>,
    pub files: Vec<(String, Vec<u8>)>,
    pub expect: Value,
}

pub fn cases() -> Vec<Case> {
    let all: Vec<Value> = serde_json::from_str(include_str!("../negative.json")).expect("negative.json");
    all.iter()
        .map(|c| Case {
            name: c["name"].as_str().expect("name").to_owned(),
            category: c["category"].as_str().expect("category").to_owned(),
            ts: c["ts"].as_bool().expect("ts"),
            mem: c["mem"].as_bool().expect("mem"),
            module: c["module"].as_bool().unwrap_or(false),
            resolve: c["resolve"].as_bool().unwrap_or(false),
            routing: c["routing"].as_bool().unwrap_or(false),
            workspace_namespace: c["workspace_namespace"].as_str().map(str::to_owned),
            files: c["files"]
                .as_object()
                .expect("files")
                .iter()
                .map(|(rel, v)| {
                    let repeat = v.get("repeat").and_then(Value::as_array);
                    let bytes = match v.get("hex").and_then(Value::as_str) {
                        _ if repeat.is_some() => {
                            let r = repeat.expect("repeat");
                            let n = usize::try_from(r[1].as_u64().expect("count")).expect("count");
                            r[0].as_str().expect("text").repeat(n).into_bytes()
                        }
                        Some(hex) => (0..hex.len())
                            .step_by(2)
                            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("hex"))
                            .collect(),
                        None => v.as_str().expect("text").as_bytes().to_vec(),
                    };
                    (rel.clone(), bytes)
                })
                .collect(),
            expect: c["expect"].clone(),
        })
        .collect()
}

impl Tmp {
    /// Writes a case's files under `rel` and returns the project folder.
    pub fn project(&self, rel: &str, files: &[(String, Vec<u8>)]) -> String {
        for (name, bytes) in files {
            let path = self.0.join(rel).join(name);
            fs::create_dir_all(path.parent().expect("a parent")).expect("create the folder");
            fs::write(path, bytes).expect("write the file");
        }
        fs::create_dir_all(self.0.join(rel)).expect("create the folder");
        self.at(rel)
    }
}

/// The finding files of each family in a dump, sorted.
pub fn finding_files(dump: &Value, family: &str) -> Vec<String> {
    dump["families"][family]["files"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect())
        .unwrap_or_default()
}

/// Whether a dump records a failed read of project.k7x, settings.k7x, a newer format.
pub fn head_flags(dump: &Value, compile: usize) -> Value {
    json!({
        "format_too_new": dump["head"]["format_too_new"],
        "root": !dump["head"]["root_ok"].as_bool().unwrap_or(true),
        "settings": !dump["head"]["settings_ok"].as_bool().unwrap_or(true),
        "compile": compile > 0,
    })
}

impl Tmp {
    /// Ages every file under the folder past the two seconds a stamp needs to be settled.
    pub fn age(&self) {
        fn walk(dir: &Path, at: std::time::SystemTime) {
            for e in fs::read_dir(dir).expect("list").flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, at);
                } else if let Ok(f) = fs::File::options().write(true).open(&p) {
                    let _ = f.set_modified(at);
                }
            }
        }
        walk(
            &self.0,
            std::time::SystemTime::now() - std::time::Duration::from_secs(3600),
        );
    }
}

/// The roots a module test reads: a workspace with a project, two libraries (the account's first), meta and a scan.
pub fn roots(t: &Tmp) -> Roots {
    Roots {
        project: t.at("ws/proj"),
        workspace: Some(t.at("ws")),
        libraries: vec![
            Library {
                name: "account".into(),
                root: t.at("lib/account"),
                editable: true,
            },
            Library {
                name: "k7-core".into(),
                root: t.at("lib/k7-core"),
                editable: false,
            },
        ],
        meta: Some(t.at("meta")),
        scan: Some(t.at("home/exts.json")),
        models: Some(t.at("home/models")),
        confine: Some(t.at("ws")),
        ..Roots::default()
    }
}

/// A disk Source serving every mount of `roots`, the scan as a file mount.
pub fn disk(roots: &Roots) -> DiskSource {
    let (dirs, files) = roots.mounts().expect("mounts");
    DiskSource::with_files(&dirs, &files).expect("a source")
}

/// A module file as a plain plugin of a name.
pub fn plugin_text(name: &str) -> String {
    format!("plugin({{ type: \"internal\", name: \"{name}\" }})\n")
}

/// The roots of a module negative case, laid out as `negative.json` places its files under `base`.
pub fn case_roots(base: &Path, workspace_namespace: Option<&str>) -> Roots {
    let at = |rel: &str| spell(&base.join(rel));
    let mut names: Vec<String> = fs::read_dir(base.join("lib"))
        .map(|d| {
            d.flatten()
                .filter(|e| e.path().is_dir())
                .filter_map(|e| e.file_name().into_string().ok())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    // The account's own library comes first, as the host orders them.
    names.sort_by_key(|n| n != "account");
    Roots {
        project: at("ws/proj"),
        workspace: Some(at("ws")),
        workspace_namespace: workspace_namespace.map(str::to_owned),
        libraries: names
            .iter()
            .map(|n| Library {
                name: n.clone(),
                root: at(&format!("lib/{n}")),
                editable: false,
            })
            .collect(),
        meta: Some(at("meta")),
        scan: Some(at("home/exts.json")),
        models: Some(at("home/models")),
        confine: Some(at("ws")),
        ..Roots::default()
    }
}

/// Where a module case's file goes under `base`: the project, the workspace, a library, `@meta` or the scan.
pub fn placed(rel: &str) -> String {
    if let Some(r) = rel.strip_prefix("../_modules/") {
        format!("ws/_modules/{r}")
    } else if let Some(r) = rel.strip_prefix("$lib/") {
        format!("lib/{r}")
    } else if let Some(r) = rel.strip_prefix("$meta/") {
        format!("meta/{r}")
    } else if rel == "$scan" {
        "home/exts.json".to_owned()
    } else if let Some(r) = rel.strip_prefix("$home/") {
        format!("home/{r}")
    } else {
        format!("ws/proj/{rel}")
    }
}

/// Chain files by name, decoded: what a chain project holds once read.
pub fn chain_files(files: &[(&str, &str)]) -> indexmap::IndexMap<String, std::sync::Arc<song::routing::ChainFile>> {
    files
        .iter()
        .map(|(name, text)| {
            let c = song::routing::decode_chain(&format!("chains/{name}.k7x"), text, song::PathRule::HOST)
                .unwrap_or_else(|f| panic!("{name}: {}:{} {}", f.line, f.col, f.message));
            ((*name).to_owned(), std::sync::Arc::new(c))
        })
        .collect()
}
