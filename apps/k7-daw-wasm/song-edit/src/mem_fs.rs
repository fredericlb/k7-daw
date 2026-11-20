// mem_fs.rs — an `Fs` over a map of paths to bytes: the browser's vfs, and what every replay of a vector writes into.
// A reader would get wrong that a folder is its files' ancestors plus those `mkdir_all` named: an empty folder is not remembered.

use crate::fs::Fs;
use crate::paths::{dirname, resolve};
use song::{Entry, Kind, Source, SourceError, Span, Stamp, Stat};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default, Clone)]
pub struct MemFs {
    files: BTreeMap<String, (Vec<u8>, u64)>,
    dirs: BTreeSet<String>,
    tick: u64,
}

impl MemFs {
    #[must_use]
    pub fn new() -> MemFs {
        MemFs::default()
    }

    /// A file system holding `files`, each under `root`.
    #[must_use]
    pub fn from_tree(root: &str, files: impl IntoIterator<Item = (String, Vec<u8>)>) -> MemFs {
        let mut fs = MemFs::new();
        for (rel, bytes) in files {
            let path = resolve(&format!("{root}/{rel}"));
            fs.tick += 1;
            fs.files.insert(path, (bytes, fs.tick));
        }
        fs
    }

    /// Every file, path and bytes, in path order.
    pub fn files(&self) -> impl Iterator<Item = (&str, &[u8])> {
        self.files.iter().map(|(p, (b, _))| (p.as_str(), b.as_slice()))
    }

    /// The folders `mkdir_all` named, which no file implies.
    pub fn dirs(&self) -> impl Iterator<Item = &str> {
        self.dirs.iter().map(String::as_str)
    }

    /// A file's text, lossy where it is not UTF-8.
    #[must_use]
    pub fn text(&self, path: &str) -> Option<String> {
        self.files
            .get(&resolve(path))
            .map(|(b, _)| String::from_utf8_lossy(b).into_owned())
    }

    fn is_dir(&self, p: &str) -> bool {
        if p == "/" || self.dirs.contains(p) {
            return true;
        }
        let prefix = format!("{p}/");
        self.files
            .range(prefix.clone()..)
            .next()
            .is_some_and(|(k, _)| k.starts_with(&prefix))
            || self
                .dirs
                .range(prefix.clone()..)
                .next()
                .is_some_and(|k| k.starts_with(&prefix))
    }

    fn stamp(&self, len: u64, tick: u64) -> Stamp {
        Stamp {
            mtime_ns: tick,
            len,
            settled: true,
        }
    }

    fn enoent(op: &str, path: &str) -> SourceError {
        SourceError::Io(format!("ENOENT: no such file or directory, {op} '{path}'"))
    }
}

impl Source for MemFs {
    fn stat(&self, path: &str) -> Result<Stat, SourceError> {
        let p = resolve(path);
        if let Some((b, tick)) = self.files.get(&p) {
            let len = b.len() as u64;
            return Ok(Stat {
                kind: Kind::File,
                len,
                stamp: self.stamp(len, *tick),
            });
        }
        if self.is_dir(&p) {
            return Ok(Stat {
                kind: Kind::Dir,
                len: 0,
                stamp: self.stamp(0, 0),
            });
        }
        Err(SourceError::NotFound)
    }

    fn list(&self, dir: &str) -> Result<Vec<Entry>, SourceError> {
        let d = resolve(dir);
        if !self.is_dir(&d) {
            return Err(SourceError::NotFound);
        }
        let prefix = if d == "/" { "/".to_owned() } else { format!("{d}/") };
        let mut seen: BTreeMap<String, Kind> = BTreeMap::new();
        let below = self.files.keys().chain(self.dirs.iter());
        for k in below {
            let Some(rest) = k.strip_prefix(&prefix) else { continue };
            if rest.is_empty() {
                continue;
            }
            let (name, kind) = match rest.split_once('/') {
                Some((n, _)) => (n, Kind::Dir),
                None => (
                    rest,
                    if self.files.contains_key(k) {
                        Kind::File
                    } else {
                        Kind::Dir
                    },
                ),
            };
            seen.entry(name.to_owned()).or_insert(kind);
        }
        Ok(seen.into_iter().map(|(name, kind)| Entry { name, kind }).collect())
    }

    fn read(&self, path: &str, span: Span) -> Result<Vec<u8>, SourceError> {
        let (bytes, _) = self.files.get(&resolve(path)).ok_or(SourceError::NotFound)?;
        Ok(match span {
            Span::All => bytes.clone(),
            Span::Range { offset, len } => {
                let from = usize::try_from(offset).unwrap_or(usize::MAX).min(bytes.len());
                let to = from
                    .saturating_add(usize::try_from(len).unwrap_or(usize::MAX))
                    .min(bytes.len());
                bytes[from..to].to_vec()
            }
        })
    }
}

impl Fs for MemFs {
    fn write_atomic(&mut self, path: &str, bytes: &[u8]) -> Result<(), SourceError> {
        let p = resolve(path);
        if self.is_dir(&p) {
            return Err(SourceError::Io(format!(
                "EISDIR: illegal operation on a directory, open '{p}'"
            )));
        }
        if !self.is_dir(&dirname(&p)) {
            return Err(MemFs::enoent("open", &p));
        }
        self.tick += 1;
        self.files.insert(p, (bytes.to_vec(), self.tick));
        Ok(())
    }

    fn remove(&mut self, path: &str) -> Result<(), SourceError> {
        let p = resolve(path);
        if self.files.remove(&p).is_none() && self.is_dir(&p) {
            return Err(SourceError::Io(format!(
                "Path is a directory: rm returned EISDIR (is a directory) {p}"
            )));
        }
        Ok(())
    }

    fn remove_tree(&mut self, path: &str) -> Result<(), SourceError> {
        let p = resolve(path);
        let prefix = format!("{p}/");
        self.files.retain(|k, _| *k != p && !k.starts_with(&prefix));
        self.dirs.retain(|d| *d != p && !d.starts_with(&prefix));
        Ok(())
    }

    fn rename(&mut self, from: &str, to: &str) -> Result<(), SourceError> {
        let (a, b) = (resolve(from), resolve(to));
        let parent_ok = self.is_dir(&dirname(&b));
        if let Some(entry) = self.files.remove(&a) {
            if !parent_ok {
                self.files.insert(a, entry);
                return Err(SourceError::Io(format!(
                    "ENOENT: no such file or directory, rename '{from}' -> '{to}'"
                )));
            }
            self.tick += 1;
            self.files.insert(b, (entry.0, self.tick));
            return Ok(());
        }
        if self.is_dir(&a) {
            let prefix = format!("{a}/");
            let moved: Vec<String> = self.files.keys().filter(|k| k.starts_with(&prefix)).cloned().collect();
            for k in moved {
                if let Some(v) = self.files.remove(&k) {
                    self.files.insert(format!("{b}/{}", &k[prefix.len()..]), v);
                }
            }
            self.dirs.insert(b);
            return Ok(());
        }
        Err(SourceError::Io(format!(
            "ENOENT: no such file or directory, rename '{from}' -> '{to}'"
        )))
    }

    fn copy(&mut self, from: &str, to: &str) -> Result<(), SourceError> {
        let (a, b) = (resolve(from), resolve(to));
        let Some((bytes, _)) = self.files.get(&a).cloned() else {
            return Err(SourceError::Io(format!(
                "ENOENT: no such file or directory, copyfile '{from}' -> '{to}'"
            )));
        };
        if !self.is_dir(&dirname(&b)) {
            return Err(SourceError::Io(format!(
                "ENOENT: no such file or directory, copyfile '{from}' -> '{to}'"
            )));
        }
        self.tick += 1;
        self.files.insert(b, (bytes, self.tick));
        Ok(())
    }

    fn mkdir_all(&mut self, dir: &str) -> Result<(), SourceError> {
        let p = resolve(dir);
        if self.files.contains_key(&p) {
            return Err(SourceError::Io(format!("EEXIST: file already exists, mkdir '{p}'")));
        }
        self.dirs.insert(p);
        Ok(())
    }
}
