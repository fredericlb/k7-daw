// source/mem.rs — the Source over a bundle of files the server already read, plus drafts laid over it.
// A reader would get wrong that an absent path is Missing and not NotFound, unless a listing it must be in is held and omits it.
// A binary is held as its length alone, so a range of its bytes is Missing too; the refused paths answer Outside, as the disk did.

use super::bundle::{BundleFile, BundleRequest, MAX_BUNDLE, SourceBundle};
use super::path::{Mounts, any_head, depth, head, join, normalize, parent, parent_str};
use super::{Entry, Kind, Source, SourceError, Span, Stamp, Stat, sort_entries};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

enum Held {
    Text(String),
    Len(u64),
    Binary(u64),
    Io(u64, String),
}

/// What the source knows of one path.
enum Know<'a> {
    Text(&'a str),
    Len(u64),
    Binary(u64),
    Io(u64, &'a str),
    Gone,
    Unknown,
}

pub struct MemSource {
    mounts: Mounts,
    files: HashMap<String, Held>,
    lists: HashMap<String, Vec<Entry>>,
    /// Every proper ancestor of a held file or listing, so a folder question is one lookup.
    ancestors: HashSet<String>,
    /// How many names the deepest held file or listing has: nothing deeper is a folder with a listing.
    held_depth: usize,
    drafts: HashMap<String, Option<String>>,
    generations: HashMap<String, u64>,
    tick: u64,
    /// Refused paths, and everything below them.
    refused: HashSet<String>,
    /// The deepest refused entry's names and bytes, which bound how much of a path is looked up.
    refused_depth: usize,
    refused_len: usize,
    /// Refused paths with a mount below them: the path itself and nothing under it.
    refused_exact: HashSet<String>,
    /// Paths the disk answered NotFound for.
    absent: HashSet<String>,
    failed_stats: HashMap<String, String>,
    failed_lists: HashMap<String, String>,
    digests: BTreeMap<String, String>,
    jsdsp_stale: BTreeSet<String>,
    lengths: BTreeMap<String, f64>,
    measured: bool,
    request: Option<BundleRequest>,
}

/// A bundle file's kind, or why it is not one.
fn held_of(path: &str, f: BundleFile) -> Result<Held, SourceError> {
    let bad = || {
        SourceError::Io(format!(
            "{path}: a bundle file holds its text, or its length with at most one of binary and io"
        ))
    };
    match f {
        BundleFile {
            text: Some(t),
            len: None,
            binary: None,
            io: None,
        } => Ok(Held::Text(t)),
        BundleFile {
            text: None,
            len: Some(n),
            binary,
            io,
        } => match (binary.unwrap_or(false), io) {
            (false, None) => Ok(Held::Len(n)),
            // An empty file is UTF-8, so it cannot be a binary.
            (true, None) if n > 0 => Ok(Held::Binary(n)),
            (false, Some(m)) => Ok(Held::Io(n, m)),
            _ => Err(bad()),
        },
        _ => Err(bad()),
    }
}

/// The keys of a bundle's path map, normalised; one that does not normalise refuses the bundle.
fn norm_keys<V>(map: BTreeMap<String, V>) -> Result<BTreeMap<String, V>, SourceError> {
    map.into_iter().map(|(k, v)| Ok((normalize(&k)?, v))).collect()
}

impl MemSource {
    pub fn from_bundle(bundle: SourceBundle) -> Result<MemSource, SourceError> {
        bundle.check()?;
        if let Some(r) = &bundle.request
            && r.vsts != "scan"
            && r.vsts != "none"
        {
            return Err(SourceError::Io(format!(
                "bundle request vsts {:?} is not scan or none",
                r.vsts
            )));
        }
        let mut files = HashMap::new();
        for (path, f) in bundle.files {
            let held = held_of(&path, f)?;
            files.insert(normalize(&path)?, held);
        }
        let mut lists = HashMap::new();
        for (dir, mut entries) in bundle.lists {
            sort_entries(&mut entries);
            lists.insert(normalize(&dir)?, entries);
        }
        let mut ancestors = HashSet::new();
        let mut ancestor_bytes = 0usize;
        let mut held_depth = 0;
        for held in files.keys().chain(lists.keys()) {
            held_depth = held_depth.max(depth(held));
            let mut up = parent_str(held);
            // An ancestor already held has all of its own above it.
            while let Some(dir) = up {
                if ancestors.contains(dir) {
                    break;
                }
                ancestor_bytes = ancestor_bytes.saturating_add(dir.len());
                if ancestor_bytes > MAX_BUNDLE {
                    return Err(SourceError::Io(format!(
                        "the folders above a bundle's paths weigh more than {MAX_BUNDLE} bytes"
                    )));
                }
                ancestors.insert(dir.to_owned());
                up = parent_str(dir);
            }
        }
        let mut refused = bundle
            .refused
            .iter()
            .map(|p| normalize(p))
            .collect::<Result<HashSet<_>, _>>()?;
        let mounts = Mounts::new(&bundle.mounts)?;
        // A refused path with a mount below it says nothing of what lies under it, so it answers for itself alone.
        let below: BTreeSet<&str> = mounts
            .roots()
            .iter()
            .chain(mounts.file_roots())
            .map(String::as_str)
            .collect();
        let has_mount_below = |p: &str| {
            let prefix = if p.ends_with('/') {
                p.to_owned()
            } else {
                format!("{p}/")
            };
            below
                .range(prefix.as_str()..)
                .next()
                .is_some_and(|m| m.starts_with(&prefix))
        };
        let refused_exact: HashSet<String> = refused.iter().filter(|p| has_mount_below(p)).cloned().collect();
        refused.retain(|p| !refused_exact.contains(p));
        let strings = |map: BTreeMap<String, String>| -> Result<HashMap<String, String>, SourceError> {
            Ok(norm_keys(map)?.into_iter().collect())
        };
        Ok(MemSource {
            mounts,
            files,
            lists,
            ancestors,
            held_depth,
            drafts: HashMap::new(),
            generations: HashMap::new(),
            tick: 0,
            refused_depth: refused.iter().map(|p| depth(p)).max().unwrap_or(0),
            refused_len: refused.iter().map(String::len).max().unwrap_or(0),
            refused,
            refused_exact,
            absent: bundle
                .absent
                .iter()
                .map(|p| normalize(p))
                .collect::<Result<HashSet<_>, _>>()?,
            failed_stats: strings(bundle.failed_stats)?,
            failed_lists: strings(bundle.failed_lists)?,
            digests: norm_keys(bundle.digests)?,
            jsdsp_stale: bundle
                .jsdsp_stale
                .iter()
                .map(|p| normalize(p))
                .collect::<Result<BTreeSet<_>, _>>()?,
            lengths: norm_keys(bundle.lengths)?,
            measured: bundle.measured,
            request: bundle.request,
        })
    }

    /// The lengths the bundle carries, by normalised path.
    pub fn lengths(&self) -> &BTreeMap<String, f64> {
        &self.lengths
    }

    /// What a replay must ask, when the bundle says.
    pub fn request(&self) -> Option<&BundleRequest> {
        self.request.as_ref()
    }

    /// The digests the bundle carries, by normalised path.
    pub fn digests(&self) -> &BTreeMap<String, String> {
        &self.digests
    }

    /// The jsdsp sources the bundle says were stale, normalised.
    pub fn jsdsp_stale(&self) -> &BTreeSet<String> {
        &self.jsdsp_stale
    }

    /// Lays a draft text over `path`, or a deletion with None, and bumps that path's generation.
    pub fn set(&mut self, path: &str, text: Option<String>) -> Result<(), SourceError> {
        let path = normalize(path)?;
        self.tick += 1;
        self.generations.insert(path.clone(), self.tick);
        // Every folder on the way up to the mount moves, so a stamp keyed on a listing above sees a new folder two levels down.
        let mut up = parent(&path);
        while let Some(dir) = up {
            if self.mounts.containing(&dir).is_empty() {
                break;
            }
            self.generations.insert(dir.clone(), self.tick);
            up = parent(&dir);
        }
        self.drafts.insert(path, text);
        Ok(())
    }

    fn know(&self, path: &str) -> Know<'_> {
        match self.drafts.get(path) {
            Some(Some(t)) => return Know::Text(t),
            Some(None) => return Know::Gone,
            None => {}
        }
        match self.files.get(path) {
            Some(Held::Text(t)) => Know::Text(t),
            Some(Held::Len(n)) => Know::Len(*n),
            Some(Held::Binary(n)) => Know::Binary(*n),
            Some(Held::Io(n, m)) => Know::Io(*n, m),
            None => Know::Unknown,
        }
    }

    fn checked(&self, path: &str) -> Result<String, SourceError> {
        let path = normalize(path)?;
        if self.mounts.containing(&path).is_empty() || self.is_refused(&path) {
            Err(SourceError::Outside)
        } else {
            Ok(path)
        }
    }

    /// The `Io` a failed call kept.
    fn failed(&self, map: &HashMap<String, String>, path: &str) -> SourceError {
        SourceError::Io(map.get(path).cloned().unwrap_or_default())
    }

    /// Whether the path, or a folder above it, answered `Outside` when the bundle was made.
    fn is_refused(&self, path: &str) -> bool {
        self.refused_exact.contains(path)
            || (!self.refused.is_empty()
                && any_head(path, self.refused_depth, self.refused_len, |h| self.refused.contains(h)))
    }

    /// Whether anything held lies below `dir`; drafts count only when asked.
    fn holds_below(&self, dir: &str, drafts: bool) -> bool {
        let below = |p: &String| p.len() > dir.len() && super::path::is_within(dir, p);
        self.ancestors.contains(dir) || (drafts && self.drafts.iter().any(|(p, d)| d.is_some() && below(p)))
    }

    /// Whether the listing of the path's own folder is held, omits it, and no draft lies below it.
    fn omitted_by_listing(&self, path: &str) -> bool {
        let Some(dir) = parent(path) else { return false };
        let Some(entries) = self.lists.get(&dir) else {
            return false;
        };
        let name = path.rsplit('/').next().unwrap_or(path);
        let below = |p: &String| p.len() > path.len() && super::path::is_within(path, p);
        entries
            .binary_search_by(|e| e.name.as_bytes().cmp(name.as_bytes()))
            .is_err()
            && !self.drafts.iter().any(|(p, d)| d.is_some() && below(p))
    }

    /// The error for a path nothing holds: NotFound when the disk said so or the nearest listing held above it omits its branch.
    fn absent(&self, path: &str) -> SourceError {
        if self.absent.contains(path) {
            return SourceError::NotFound;
        }
        // Nothing deeper than the deepest held path has a listing, so a path a million names deep starts at its shallow end.
        let mut child = head(path, self.held_depth + 1);
        while let Some(dir) = parent_str(child) {
            if let Some(entries) = self.lists.get(dir) {
                let name = child.rsplit('/').next().unwrap_or(child);
                let named = entries
                    .binary_search_by(|e| e.name.as_bytes().cmp(name.as_bytes()))
                    .is_ok();
                // A folder only drafts lie in is new and wholly known, so its other paths are absent, not missing.
                return if named || self.holds_below(child, false) {
                    SourceError::Missing
                } else {
                    SourceError::NotFound
                };
            }
            child = dir;
        }
        SourceError::Missing
    }

    /// A file's stamp: the hash of its text, so two bundles holding different text never share one.
    fn stamp(&self, path: &str, len: u64, text: Option<&str>) -> Stamp {
        let body = text.map_or_else(|| fnv1a(format!("len:{len}").as_bytes()), |t| fnv1a(t.as_bytes()));
        Stamp {
            mtime_ns: body ^ self.generations.get(path).copied().unwrap_or(0).rotate_left(32),
            len,
            settled: true,
        }
    }

    /// A folder's stamp: its listing and the generation `set` bumped.
    fn dir_stamp(&self, dir: &str) -> Stamp {
        let mut bytes = Vec::new();
        for e in self.list(dir).unwrap_or_default() {
            bytes.extend_from_slice(e.name.as_bytes());
            bytes.push(if e.kind == Kind::Dir { 1 } else { 0 });
            bytes.push(0);
        }
        let gen_bump = self.generations.get(dir).copied().unwrap_or(0).rotate_left(32);
        Stamp {
            mtime_ns: fnv1a(&bytes) ^ gen_bump,
            len: 0,
            settled: true,
        }
    }
}

/// FNV-1a, 64 bits.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3)
    })
}

impl Source for MemSource {
    fn known_digest(&self, path: &str) -> Option<String> {
        self.digests.get(&normalize(path).ok()?).cloned()
    }

    fn known_stale(&self, source: &str) -> Option<bool> {
        Some(self.jsdsp_stale.contains(&normalize(source).ok()?))
    }

    fn known_seconds(&self, path: &str) -> Option<f64> {
        self.lengths.get(&normalize(path).ok()?).copied()
    }

    fn measured_by_table(&self) -> bool {
        self.measured
    }

    fn stat(&self, path: &str) -> Result<Stat, SourceError> {
        let path = self.checked(path)?;
        let file = |len: u64, text: Option<&str>| {
            Ok(Stat {
                kind: Kind::File,
                len,
                stamp: self.stamp(&path, len, text),
            })
        };
        match self.know(&path) {
            Know::Text(t) => file(t.len() as u64, Some(t)),
            Know::Len(n) | Know::Binary(n) | Know::Io(n, _) => file(n, None),
            Know::Gone => Err(SourceError::NotFound),
            Know::Unknown if self.failed_stats.contains_key(&path) => Err(self.failed(&self.failed_stats, &path)),
            // A folder with a listing of its own, or one that would not list, is there, whatever the listing above spells.
            Know::Unknown if self.lists.contains_key(&path) || self.failed_lists.contains_key(&path) => Ok(Stat {
                kind: Kind::Dir,
                len: 0,
                stamp: self.dir_stamp(&path),
            }),
            // A held listing above that omits the name settles an absent path without a scan of everything held.
            Know::Unknown if self.omitted_by_listing(&path) => Err(SourceError::NotFound),
            Know::Unknown if self.holds_below(&path, true) => Ok(Stat {
                kind: Kind::Dir,
                len: 0,
                stamp: self.dir_stamp(&path),
            }),
            Know::Unknown => Err(self.absent(&path)),
        }
    }

    fn list(&self, dir: &str) -> Result<Vec<Entry>, SourceError> {
        let dir = self.checked(dir)?;
        if !self.lists.contains_key(&dir) && self.failed_lists.contains_key(&dir) {
            return Err(self.failed(&self.failed_lists, &dir));
        }
        let mut out: Vec<Entry> = match self.lists.get(&dir) {
            Some(held) => held
                .iter()
                .filter(|e| !matches!(self.drafts.get(&join(&dir, &e.name)), Some(None)))
                .cloned()
                .collect(),
            // A folder known to be absent can still be given drafts; one nothing is known about cannot.
            None => match self.absent(&dir) {
                SourceError::NotFound => Vec::new(),
                other => return Err(other),
            },
        };
        let mut drafted = false;
        for (path, draft) in &self.drafts {
            if draft.is_none() || path.len() <= dir.len() || !super::path::is_within(&dir, path) {
                continue;
            }
            // A draft below this folder names its first step: a file when it is the last, else a folder no listing held.
            let rest = path.get(dir.len()..).unwrap_or("").trim_start_matches('/');
            let (name, kind) = match rest.split_once('/') {
                Some((first, _)) => (first, Kind::Dir),
                None => (rest, Kind::File),
            };
            drafted = true;
            if !out.iter().any(|e| e.name == name) {
                out.push(Entry {
                    name: name.to_owned(),
                    kind,
                });
            }
        }
        if out.is_empty() && !drafted && !self.lists.contains_key(&dir) {
            return Err(SourceError::NotFound);
        }
        sort_entries(&mut out);
        Ok(out)
    }

    fn read(&self, path: &str, span: Span) -> Result<Vec<u8>, SourceError> {
        let path = self.checked(path)?;
        match self.know(&path) {
            Know::Text(t) => Ok(match span {
                Span::All => t.as_bytes().to_vec(),
                Span::Range { offset, len } => {
                    let from = usize::try_from(offset).unwrap_or(usize::MAX).min(t.len());
                    let to = usize::try_from(len)
                        .map_or(t.len(), |n| from.saturating_add(n))
                        .min(t.len());
                    t.as_bytes().get(from..to).unwrap_or_default().to_vec()
                }
            }),
            // A file that was not UTF-8 reads as one byte no text starts with, so each caller takes its own branch.
            Know::Binary(_) if span == Span::All => Ok(vec![0xFF]),
            Know::Len(_) | Know::Binary(_) => Err(SourceError::Missing),
            Know::Io(_, m) => Err(SourceError::Io(m.to_owned())),
            Know::Gone => Err(SourceError::NotFound),
            Know::Unknown if self.failed_stats.contains_key(&path) => Err(self.failed(&self.failed_stats, &path)),
            Know::Unknown if self.lists.contains_key(&path) || self.failed_lists.contains_key(&path) => {
                Err(SourceError::Io(format!("{path}: is a directory")))
            }
            Know::Unknown => Err(self.absent(&path)),
        }
    }
}
