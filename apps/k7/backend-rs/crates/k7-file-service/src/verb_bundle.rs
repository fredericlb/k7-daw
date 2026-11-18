//! verb_bundle.rs — what the browser's verbs read, made once when a project opens (3d): the project's texts, the shared texts of its
//! workspace, binaries as a length and seconds, the listings, and each library named by a hash. What a reader gets wrong: a library is
//! never inside the source, it is fetched by its hash; and a binary's bytes are hashed by length alone.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use song::roots::Roots;
use song::source::{BundleFile, BundleRequest, Entry, Kind, MAX_BUNDLE, MAX_ENTRIES, SourceBundle};
use song::views::wav::{HEAD_BYTES, wav_seconds};

use crate::filter::{is_binary, skip};
use crate::libraries::{Home, Version, version_of_dir};
use crate::paths::{basename, join, resolve_all};
use crate::verb_paths::Virtual;

/// The shared texts of a workspace, below its root: its modules, its playlists and its library pins.
const SHARED: [&str; 3] = ["_modules", "_playlists", "_library-pins.k7x"];
/// The most folders one tree walk enters.
const MAX_DIRS: usize = 200_000;

/// Why a bundle is not made: its status and a sentence naming no folder of the server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refused {
    pub status: u16,
    pub error: String,
}

fn refused(status: u16, error: impl Into<String>) -> Refused {
    Refused {
        status,
        error: error.into(),
    }
}

/// What `build` reads: the open project, the workspace it stands in, the roots and the rig the compile would use.
pub struct Inputs<'a> {
    pub home: &'a Home,
    pub root: &'a str,
    pub project: &'a str,
    pub at: &'a str,
    pub rev: &'a str,
    pub roots: Roots,
    pub rig: Value,
    pub code_root: &'a str,
}

/// The bytes the walks have taken so far, against the bundle's own bound.
struct Budget {
    bytes: usize,
    names: usize,
    /// Whether binaries are hashed: the project's are, a library's (hashed whole elsewhere) are not.
    digests: bool,
}

impl Budget {
    fn take(&mut self, n: usize) -> Result<(), Refused> {
        self.bytes = self.bytes.saturating_add(n);
        self.names += 1;
        if self.bytes > MAX_BUNDLE || self.names > MAX_ENTRIES {
            return Err(refused(
                507,
                "the project is past the bundle's bound: its verbs run on the server's own file service instead",
            ));
        }
        Ok(())
    }
}

/// A file as a bundle holds it: its text; or its length alone, for a binary name or bytes that are no UTF-8.
fn held(abs: &str, len: u64) -> BundleFile {
    if is_binary(&basename(abs)) {
        return BundleFile {
            len: Some(len),
            binary: Some(true),
            ..BundleFile::default()
        };
    }
    match std::fs::read(abs).map(String::from_utf8) {
        Ok(Ok(text)) => BundleFile {
            text: Some(text),
            ..BundleFile::default()
        },
        Ok(Err(_)) => BundleFile {
            len: Some(len),
            binary: Some(true),
            ..BundleFile::default()
        },
        Err(e) => BundleFile {
            len: Some(len),
            io: Some(crate::filter::code_of(&e).to_string()),
            ..BundleFile::default()
        },
    }
}

/// A WAV's seconds from its header; `None` for any other file.
fn seconds(abs: &str, len: u64) -> Option<f64> {
    use std::io::Read;
    if !abs.to_ascii_lowercase().ends_with(".wav") {
        return None;
    }
    let mut head = Vec::new();
    std::fs::File::open(abs)
        .ok()?
        .take(HEAD_BYTES)
        .read_to_end(&mut head)
        .ok()?;
    wav_seconds(&head, len)
}

/// Every file of the tree at `dir` (a file is its own tree): texts, lengths and listings into the bundle, hidden names listed and never read.
fn add_tree(src: &mut SourceBundle, budget: &mut Budget, dir: &str) -> Result<(), Refused> {
    let Ok(meta) = std::fs::symlink_metadata(dir) else {
        return Ok(());
    };
    if meta.is_file() {
        let f = held(dir, meta.len());
        budget.take(dir.len() + f.text.as_ref().map_or(8, String::len))?;
        src.files.insert(dir.to_string(), f);
        return Ok(());
    }
    let mut stack = vec![dir.to_string()];
    let mut seen = 0usize;
    let mut hashed = 0u64;
    while let Some(d) = stack.pop() {
        seen += 1;
        if seen > MAX_DIRS {
            return Err(refused(507, "the project has too many folders to bundle"));
        }
        let Ok(read) = std::fs::read_dir(&d) else {
            continue;
        };
        let mut entries: Vec<Entry> = Vec::new();
        for e in read.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let Ok(kind) = e.file_type() else { continue };
            let hidden = skip(&name);
            let path = join(&[&d, &name]);
            if kind.is_dir() && !kind.is_symlink() {
                entries.push(Entry {
                    name,
                    kind: Kind::Dir,
                });
                if !hidden {
                    stack.push(path);
                }
            } else if kind.is_file() {
                entries.push(Entry {
                    name,
                    kind: Kind::File,
                });
                if hidden {
                    continue;
                }
                let len = e.metadata().map_or(0, |m| m.len());
                let f = held(&path, len);
                budget.take(path.len() + f.text.as_ref().map_or(8, String::len))?;
                if f.binary == Some(true)
                    && let Some(s) = seconds(&path, len)
                {
                    src.lengths.insert(path.clone(), s);
                }
                if f.binary == Some(true)
                    && budget.digests
                    && len <= DIGEST_MAX
                    && hashed.saturating_add(len) <= DIGEST_TOTAL
                    && let Some(d) = digest16(&path)
                {
                    hashed += len;
                    src.digests.insert(path.clone(), d);
                }
                src.files.insert(path, f);
            }
        }
        entries.sort_by(|a, b| a.name.as_bytes().cmp(b.name.as_bytes()));
        budget.take(d.len() + entries.iter().map(|e| e.name.len() + 8).sum::<usize>())?;
        src.lists.insert(d, entries);
    }
    Ok(())
}

/// The most bytes of one binary whose digest the bundle carries, and of all of them: the compile hashes a file it holds only by length.
const DIGEST_MAX: u64 = 64 << 20;
const DIGEST_TOTAL: u64 = 512 << 20;

/// A digest by path, length and last-change time.
type DigestMemo = std::sync::Mutex<BTreeMap<(String, u64, u128), String>>;

/// Digests already taken, by path with the length and the time the file was last changed: a reopen hashes nothing it has hashed.
fn digest_memo() -> &'static DigestMemo {
    static MEMO: std::sync::OnceLock<DigestMemo> = std::sync::OnceLock::new();
    MEMO.get_or_init(Default::default)
}

/// The first 16 hex of the SHA-256 of a file's bytes, as the song crate's `hex16` spells it.
fn digest16(abs: &str) -> Option<String> {
    let meta = std::fs::metadata(abs).ok()?;
    let at = meta
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_nanos();
    let key = (abs.to_string(), meta.len(), at);
    if let Some(d) = digest_memo()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&key)
    {
        return Some(d.clone());
    }
    let bytes = std::fs::read(abs).ok()?;
    let sum: String = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let d = sum[..16].to_string();
    let mut memo = digest_memo().lock().unwrap_or_else(|e| e.into_inner());
    if memo.len() > 50_000 {
        memo.clear();
    }
    memo.insert(key, d.clone());
    Some(d)
}

/// The published versions of a library this machine holds, each with the lowest level that runs it: what `library versions` and `upgrade` read.
fn installed_versions(home: &Home, name: &str) -> Vec<Value> {
    let dir = home.library_home(name);
    let Ok(read) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out: Vec<(u32, u32)> = read
        .flatten()
        .filter_map(|e| {
            let Some(Version::Number(n)) = version_of_dir(&e.file_name().to_string_lossy()) else {
                return None;
            };
            let manifest =
                std::fs::read_to_string(join(&[&dir, &n.to_string(), "library.k7x"])).ok();
            // The text after `requires` up to its number: a manifest is a few lines, and one without it asks for level 1.
            let requires = manifest
                .as_deref()
                .and_then(|t| t.split_once("requires"))
                .and_then(|(_, rest)| {
                    let digits: String = rest
                        .chars()
                        .skip_while(|c| !c.is_ascii_digit())
                        .take_while(char::is_ascii_digit)
                        .collect();
                    digits.parse().ok()
                })
                .unwrap_or(1);
            Some((n, requires))
        })
        .collect();
    out.sort_unstable();
    out.into_iter()
        .map(|(version, requires)| json!({ "version": version, "requires": requires }))
        .collect()
}

/// One folder's names and kinds, nothing below it and no bytes.
fn add_listing(src: &mut SourceBundle, dir: &str) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<Entry> = read
        .flatten()
        .filter_map(|e| {
            let kind = e.file_type().ok()?;
            Some(Entry {
                name: e.file_name().to_string_lossy().into_owned(),
                kind: if kind.is_dir() { Kind::Dir } else { Kind::File },
            })
        })
        .collect();
    entries.sort_by(|a, b| a.name.as_bytes().cmp(b.name.as_bytes()));
    src.lists.entry(dir.to_string()).or_insert(entries);
}

/// The most entries the models listing holds; past it the listing is dropped, never the bundle refused.
const MAX_MODEL_NAMES: usize = 20_000;

/// A folder's names and the lengths of its files, never their bytes: the models folder (3f-9), whose files are hundreds of MB.
/// Hidden names are skipped, a link counts when it resolves to a file below the folder, and a folder too large to list is left out whole.
fn add_names(src: &mut SourceBundle, dir: &str) {
    let Ok(root) = std::fs::canonicalize(dir) else {
        return;
    };
    let mut files: BTreeMap<String, BundleFile> = BTreeMap::new();
    let mut lists: BTreeMap<String, Vec<Entry>> = BTreeMap::new();
    let mut stack = vec![dir.to_string()];
    let mut seen = 0usize;
    while let Some(d) = stack.pop() {
        let Ok(read) = std::fs::read_dir(&d) else {
            continue;
        };
        let mut entries: Vec<Entry> = Vec::new();
        for e in read.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if skip(&name) {
                continue;
            }
            let Ok(kind) = e.file_type() else { continue };
            let path = join(&[&d, &name]);
            seen += 1;
            if seen > MAX_MODEL_NAMES {
                return;
            }
            if kind.is_dir() && !kind.is_symlink() {
                entries.push(Entry {
                    name,
                    kind: Kind::Dir,
                });
                stack.push(path);
                continue;
            }
            // A link is followed once, and listed only as a file that stays below the folder.
            let target = if kind.is_symlink() {
                match std::fs::canonicalize(&path) {
                    Ok(t) if t.starts_with(&root) => std::fs::metadata(&t).ok(),
                    _ => None,
                }
            } else {
                e.metadata().ok()
            };
            let Some(meta) = target.filter(|m| m.is_file()) else {
                continue;
            };
            entries.push(Entry {
                name,
                kind: Kind::File,
            });
            files.insert(
                path,
                BundleFile {
                    len: Some(meta.len()),
                    binary: Some(true),
                    ..BundleFile::default()
                },
            );
        }
        entries.sort_by(|a, b| a.name.as_bytes().cmp(b.name.as_bytes()));
        lists.insert(d, entries);
    }
    src.files.extend(files);
    src.lists.extend(lists);
}

/// A library read once: its files by path below its root, and the seconds of its WAVs; the hash and the answer come from this one read.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Library {
    pub files: BTreeMap<String, BundleFile>,
    pub lengths: BTreeMap<String, f64>,
}

impl Library {
    /// The sha-256 of the files, their paths, lengths, seconds and (for a text) bytes in path order; and how many and how big.
    pub fn hash(&self) -> (String, usize, u64) {
        let mut h = Sha256::new();
        let mut bytes = 0u64;
        for (path, f) in &self.files {
            h.update(path.as_bytes());
            let len = f
                .text
                .as_ref()
                .map_or(f.len.unwrap_or(0), |t| t.len() as u64);
            h.update(len.to_le_bytes());
            if let Some(t) = &f.text {
                h.update(t.as_bytes());
            }
            if let Some(s) = self.lengths.get(path) {
                h.update(s.to_bits().to_le_bytes());
            }
            bytes += len;
        }
        let sum: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
        (sum, self.files.len(), bytes)
    }
}

/// The files of a library, by their path below its root: one walk, what `/verb-bundle/library` answers and what its hash covers.
pub fn library_files(dir: &str) -> Result<Library, Refused> {
    let mut src = SourceBundle::default();
    let root = resolve_all(&[dir]);
    add_tree(
        &mut src,
        &mut Budget {
            bytes: 0,
            names: 0,
            digests: false,
        },
        &root,
    )?;
    let below = |abs: String| {
        abs.strip_prefix(&format!("{root}/"))
            .map_or_else(|| abs.clone(), str::to_string)
    };
    Ok(Library {
        files: src.files.into_iter().map(|(a, f)| (below(a), f)).collect(),
        lengths: src
            .lengths
            .into_iter()
            .map(|(a, s)| (below(a), s))
            .collect(),
    })
}

/// The libraries a compile of the project reads, by name and folder: the account's, the published ones, k7's own modules as `@meta`.
pub fn library_roots(roots: &Roots) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = roots
        .libraries
        .iter()
        .map(|l| (l.name.clone(), resolve_all(&[&l.root])))
        .collect();
    if let Some(m) = &roots.meta {
        out.push(("@meta".to_string(), resolve_all(&[m])));
    }
    out
}

/// The bundle of the open project: see the page for the shape. Libraries are named, never included; every path is virtual.
pub fn build(i: &Inputs<'_>) -> Result<Value, Refused> {
    let libs = library_roots(&i.roots);
    let virt = Virtual::new(i.root, &libs, i.code_root, &i.home.studio, &i.roots);
    let roots = virt.roots(&i.roots);
    let (mounts, _) = roots
        .mounts()
        .map_err(|e| refused(503, format!("the roots cannot be spelled: {e:?}")))?;
    let mut real = SourceBundle::default();
    let mut budget = Budget {
        bytes: 0,
        names: 0,
        digests: true,
    };
    add_tree(&mut real, &mut budget, &resolve_all(&[i.project]))?;
    for name in SHARED {
        add_tree(&mut real, &mut budget, &join(&[i.root, name]))?;
    }
    // The workspace's own names, so a file a verb makes beside the project (the pins, a playlist) is a new name, not an unknown one.
    add_listing(&mut real, &resolve_all(&[i.root]));
    if let Some(models) = &i.roots.models {
        let dir = resolve_all(&[models]);
        add_names(&mut real, &dir);
        // A machine with no models yet: listed empty, or the compile asks for the one it wants and finds the page unable to say.
        real.lists.entry(dir).or_default();
    }
    // The renders an automatic freeze stats, one path per lane: listed, or listed empty, never asked for one at a time.
    let renders = join(&[&i.home.studio, ".renders"]);
    if std::fs::metadata(&renders).is_ok_and(|m| m.is_dir()) {
        add_names(&mut real, &resolve_all(&[&renders]));
    } else {
        // An empty listing answers "not there" for every name below it, which an absent folder alone does not.
        real.lists.insert(resolve_all(&[&renders]), Vec::new());
    }
    let source = SourceBundle {
        format: 1,
        mounts,
        files: real
            .files
            .into_iter()
            .map(|(p, f)| (virt.virt(&p), f))
            .collect(),
        lists: real
            .lists
            .into_iter()
            .map(|(p, l)| (virt.virt(&p), l))
            .collect(),
        lengths: real
            .lengths
            .into_iter()
            .map(|(p, s)| (virt.virt(&p), s))
            .collect(),
        // Every wav was measured above: a file the table lacks is no wav, and its header is not asked for.
        measured: true,
        digests: real
            .digests
            .into_iter()
            .map(|(p, d)| (virt.virt(&p), d))
            .collect(),
        request: Some(BundleRequest {
            roots: roots.clone(),
            rig: i.rig.clone(),
            vsts: "scan".into(),
            thawed: Vec::new(),
        }),
        ..SourceBundle::default()
    };
    source
        .check()
        .map_err(|e| refused(507, format!("the bundle is past its bound: {e}")))?;
    let mut libraries: Vec<Value> = Vec::new();
    for (name, dir) in &libs {
        let (hash, files, bytes) = library_files(dir)?.hash();
        let root = virt.virt(dir);
        libraries.push(
            json!({ "name": name, "root": root, "hash": hash, "files": files, "bytes": bytes }),
        );
    }
    let preferences = std::fs::read(join(&[&i.home.studio, "preferences.k7x"]))
        .ok()
        .and_then(|b| String::from_utf8(b).ok());
    let mut installed = serde_json::Map::new();
    for (name, _) in &libs {
        if name != "@meta" && name != "account" {
            installed.insert(name.clone(), Value::Array(installed_versions(i.home, name)));
        }
    }
    Ok(json!({
        "format": 1,
        "at": i.at,
        "installed": installed,
        "rev": i.rev,
        "workspaceRoot": virt.virt(i.root),
        "source": serde_json::to_value(&source).map_err(|e| refused(500, e.to_string()))?,
        "preferences": preferences,
        "libraries": libraries,
    }))
}
