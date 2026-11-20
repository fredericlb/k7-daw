//! staged.rs — song-edit's `Fs` that reads through the disk and holds every write in memory until `planned` turns it into a set.
//! What a reader gets wrong: nothing here touches the disk, so a failed edit leaves it as it was; a folder is renamed first, elsewhere.

use std::collections::{BTreeMap, BTreeSet};

use song::{Entry, Kind, Source, SourceError, Span, Stamp, Stat};
use song_edit::Fs;
use song_edit::paths::dirname;

use super::edge::Edge;
use crate::changes::plan::{Act, Planned};

/// What the stage holds for a path the disk may also hold.
#[derive(Clone, Debug)]
enum Slot {
    Text(Vec<u8>),
    Gone,
    /// The file now at the key is the disk file `from`, which a move takes away.
    Moved(String),
    /// A copy of the disk file `from`.
    Copied(String),
}

pub struct StagedFs {
    disk: Edge,
    /// The folders a write may land in: the mounts, or the narrower set `reading` names.
    writes: Edge,
    slots: BTreeMap<String, Slot>,
    dirs: BTreeSet<String>,
    /// Folders whose files are all marked gone: a landing drops them once they are empty on the disk.
    removed: Vec<String>,
    tick: u64,
}

fn io(op: &str, path: &str) -> SourceError {
    SourceError::Io(format!("ENOENT: no such file or directory, {op} '{path}'"))
}

impl StagedFs {
    /// A stage over the folders `mounts`, which confine every read as `DiskSource` does.
    ///
    /// # Errors
    /// A mount that is not an absolute path.
    pub fn new<S: AsRef<str>>(mounts: &[S]) -> Result<StagedFs, SourceError> {
        Ok(StagedFs {
            disk: Edge::new(mounts)?,
            writes: Edge::new(mounts)?,
            slots: BTreeMap::new(),
            dirs: BTreeSet::new(),
            removed: Vec::new(),
            tick: 0,
        })
    }

    /// A stage that reads through `reads` (the project, the libraries, the meta modules) and writes only inside `writes`.
    ///
    /// # Errors
    /// A mount that is not an absolute path.
    pub fn reading<S: AsRef<str>, T: AsRef<str>>(
        writes: &[S],
        reads: &[T],
    ) -> Result<StagedFs, SourceError> {
        let mut all: Vec<&str> = writes.iter().map(AsRef::as_ref).collect();
        all.extend(reads.iter().map(AsRef::as_ref));
        let mut fs = StagedFs::new(&all)?;
        fs.writes = Edge::new(writes)?;
        Ok(fs)
    }

    /// A stage that writes only inside `writes` and reads `dirs` and the single `files` a compile's roots mount.
    ///
    /// # Errors
    /// A mount that is not an absolute path.
    pub fn over<S: AsRef<str>, T: AsRef<str>, F: AsRef<str>>(
        writes: &[S],
        dirs: &[T],
        files: &[F],
    ) -> Result<StagedFs, SourceError> {
        let mut fs = StagedFs::reading(writes, dirs)?;
        let all: Vec<&str> = writes
            .iter()
            .map(AsRef::as_ref)
            .chain(dirs.iter().map(AsRef::as_ref))
            .collect();
        fs.disk = Edge::with_files(&all, files)?;
        Ok(fs)
    }

    /// What the stage holds for `p`: the slot, or what a link-confined disk read says.
    fn disk_is_dir(&self, p: &str) -> bool {
        self.disk.stat(p).is_ok_and(|s| s.kind == Kind::Dir)
    }

    fn is_dir(&self, p: &str) -> bool {
        if self.dirs.contains(p) {
            return true;
        }
        let prefix = format!("{p}/");
        let staged = self
            .slots
            .range(prefix.clone()..)
            .take_while(|(k, _)| k.starts_with(&prefix))
            .any(|(_, v)| !matches!(v, Slot::Gone));
        staged || (!self.slots.contains_key(p) && self.disk_is_dir(p))
    }

    /// The path when it lies in a write mount, a mount not made yet included: a write elsewhere, or through a link out, is refused.
    fn confined(&self, path: &str) -> Result<String, SourceError> {
        let p = super::posix(path);
        match self.writes.stat(&p) {
            Err(SourceError::Outside) => Err(SourceError::Outside),
            _ => Ok(p),
        }
    }

    fn bytes_of(&self, p: &str) -> Result<Vec<u8>, SourceError> {
        match self.slots.get(p) {
            Some(Slot::Text(b)) => Ok(b.clone()),
            Some(Slot::Gone) => Err(SourceError::NotFound),
            Some(Slot::Moved(from) | Slot::Copied(from)) => self.disk.read(from, Span::All),
            None => self.disk.read(p, Span::All),
        }
    }

    /// Drop the folders `remove_tree` emptied, deepest first, where the landing left them empty; best-effort.
    pub fn drop_emptied_dirs(&self) {
        let mut all: Vec<&String> = self.removed.iter().collect();
        all.sort_by_key(|d| std::cmp::Reverse(d.len()));
        for d in all {
            let _ = std::fs::remove_dir(super::real(d));
        }
    }

    /// Every file under the folder `p`, as the stage sees them.
    fn files_under(&self, p: &str, out: &mut Vec<String>) {
        for e in self.list(p).unwrap_or_default() {
            let child = format!("{p}/{}", e.name);
            if e.kind == Kind::Dir {
                self.files_under(&child, out);
            } else {
                out.push(child);
            }
        }
    }

    /// Whether the stage has no write to land.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// The staged writes as a set for `commit`, in path order; a file that is not text, or a removal of nothing, is not carried.
    ///
    /// # Errors
    /// A staged file whose bytes are not UTF-8: a set carries text.
    pub fn planned(&self) -> Result<Planned, String> {
        let taken: BTreeSet<&str> = self
            .slots
            .values()
            .filter_map(|s| match s {
                Slot::Moved(from) => Some(from.as_str()),
                _ => None,
            })
            .collect();
        let mut acts = Vec::new();
        // A move or a copy lands before any write at its source's path, which would otherwise be what it takes.
        let mut ordered: Vec<(&String, &Slot)> = self.slots.iter().collect();
        ordered.sort_by_key(|(_, s)| !matches!(s, Slot::Moved(_) | Slot::Copied(_)));
        for (abs, slot) in ordered {
            // `rel` is the absolute path too: commit reads `abs`, and no consumer of `rel` exists yet.
            let rel = abs.clone();
            match slot {
                Slot::Text(b) => acts.push(Act::Text {
                    rel,
                    abs: super::real(abs),
                    text: String::from_utf8(b.clone())
                        .map_err(|_| format!("'{abs}' is not text"))?,
                    was: None,
                }),
                Slot::Gone if taken.contains(abs.as_str()) => {}
                Slot::Gone => acts.push(Act::Remove {
                    rel,
                    abs: super::real(abs),
                    was: None,
                }),
                Slot::Moved(from) => acts.push(Act::Move {
                    rel,
                    abs: super::real(abs),
                    from: super::real(from),
                    replace: true,
                }),
                Slot::Copied(from) => acts.push(Act::Copy {
                    rel,
                    abs: super::real(abs),
                    from: super::real(from),
                    replace: true,
                }),
            }
        }
        Ok(Planned { acts })
    }
}

impl Source for StagedFs {
    fn stamps_are_times(&self) -> bool {
        false
    }

    fn stat(&self, path: &str) -> Result<Stat, SourceError> {
        let p = super::posix(path);
        match self.slots.get(&p) {
            Some(Slot::Gone) => return Err(SourceError::NotFound),
            Some(Slot::Text(b)) => {
                let len = b.len() as u64;
                let tick = self.tick;
                return Ok(Stat {
                    kind: Kind::File,
                    len,
                    stamp: Stamp {
                        mtime_ns: tick,
                        len,
                        settled: true,
                    },
                });
            }
            Some(Slot::Moved(from) | Slot::Copied(from)) => return self.disk.stat(from),
            None => {}
        }
        if self.is_dir(&p) {
            return Ok(Stat {
                kind: Kind::Dir,
                len: 0,
                stamp: Stamp {
                    mtime_ns: 0,
                    len: 0,
                    settled: true,
                },
            });
        }
        self.disk.stat(&p)
    }

    fn list(&self, dir: &str) -> Result<Vec<Entry>, SourceError> {
        let d = super::posix(dir);
        let mut seen: BTreeMap<String, Kind> = BTreeMap::new();
        let on_disk = self.disk.list(&d);
        if let Ok(entries) = &on_disk {
            for e in entries {
                seen.insert(e.name.clone(), e.kind);
            }
        }
        let prefix = format!("{}/", d.trim_end_matches('/'));
        for (k, v) in &self.slots {
            let Some(rest) = k.strip_prefix(&prefix) else {
                continue;
            };
            let (name, kind) = match rest.split_once('/') {
                Some((n, _)) => (n, Kind::Dir),
                None => (rest, Kind::File),
            };
            if matches!(v, Slot::Gone) && !rest.contains('/') {
                seen.remove(name);
            } else if !matches!(v, Slot::Gone) {
                seen.insert(name.to_owned(), kind);
            }
        }
        for k in &self.dirs {
            if let Some(rest) = k.strip_prefix(&prefix) {
                let name = rest.split('/').next().unwrap_or(rest);
                seen.entry(name.to_owned()).or_insert(Kind::Dir);
            }
        }
        if on_disk.is_err() && seen.is_empty() && !self.is_dir(&d) {
            return on_disk;
        }
        Ok(seen
            .into_iter()
            .map(|(name, kind)| Entry { name, kind })
            .collect())
    }

    fn read(&self, path: &str, span: Span) -> Result<Vec<u8>, SourceError> {
        let p = super::posix(path);
        if !self.slots.contains_key(&p) {
            return self.disk.read(&p, span);
        }
        let bytes = self.bytes_of(&p)?;
        Ok(match span {
            Span::All => bytes,
            Span::Range { offset, len } => {
                let from = usize::try_from(offset)
                    .unwrap_or(usize::MAX)
                    .min(bytes.len());
                let to = from
                    .saturating_add(usize::try_from(len).unwrap_or(usize::MAX))
                    .min(bytes.len());
                bytes[from..to].to_vec()
            }
        })
    }
}

impl Fs for StagedFs {
    fn write_atomic(&mut self, path: &str, bytes: &[u8]) -> Result<(), SourceError> {
        let p = self.confined(path)?;
        if self.is_dir(&p) {
            return Err(SourceError::Io(format!(
                "EISDIR: illegal operation on a directory, open '{p}'"
            )));
        }
        if !self.is_dir(&dirname(&p)) {
            return Err(io("open", &p));
        }
        self.tick += 1;
        self.slots.insert(p, Slot::Text(bytes.to_vec()));
        Ok(())
    }

    fn remove(&mut self, path: &str) -> Result<(), SourceError> {
        let p = self.confined(path)?;
        if self.is_dir(&p) {
            return Err(SourceError::Io(format!(
                "Path is a directory: rm returned EISDIR (is a directory) {p}"
            )));
        }
        // A file only the stage holds is forgotten; one the disk holds is marked gone.
        if self.disk.stat(&p).is_ok() {
            self.slots.insert(p, Slot::Gone);
        } else {
            self.slots.remove(&p);
        }
        Ok(())
    }

    fn remove_tree(&mut self, path: &str) -> Result<(), SourceError> {
        let p = self.confined(path)?;
        if !self.is_dir(&p) {
            return self.remove(&p);
        }
        let mut files = Vec::new();
        self.files_under(&p, &mut files);
        for f in files {
            self.remove(&f)?;
        }
        let under = format!("{p}/");
        self.dirs.retain(|d| *d != p && !d.starts_with(&under));
        self.removed.push(p);
        Ok(())
    }

    fn rename(&mut self, from: &str, to: &str) -> Result<(), SourceError> {
        let (a, b) = (self.confined(from)?, self.confined(to)?);
        if self.is_dir(&a) {
            return Err(SourceError::Io(format!(
                "a folder is renamed on the disk before a stage is made: '{from}'"
            )));
        }
        if !self.is_dir(&dirname(&b)) {
            return Err(io("rename", from));
        }
        let slot = match self.slots.get(&a) {
            Some(Slot::Gone) => return Err(io("rename", from)),
            Some(Slot::Text(t)) => Slot::Text(t.clone()),
            Some(Slot::Moved(src)) => Slot::Moved(src.clone()),
            Some(Slot::Copied(src)) => Slot::Copied(src.clone()),
            None if self.disk.stat(&a).is_ok() => Slot::Moved(a.clone()),
            None => return Err(io("rename", from)),
        };
        self.tick += 1;
        // The old name goes whatever its slot, where the disk holds a file there; a copy's source is only read.
        if self.disk.stat(&a).is_ok() {
            self.slots.insert(a, Slot::Gone);
        } else {
            self.slots.remove(&a);
        }
        self.slots.insert(b, slot);
        Ok(())
    }

    fn copy(&mut self, from: &str, to: &str) -> Result<(), SourceError> {
        let (a, b) = (super::posix(from), self.confined(to)?);
        if !self.is_dir(&dirname(&b)) {
            return Err(io("copyfile", from));
        }
        let slot = match self.slots.get(&a) {
            Some(Slot::Gone) => return Err(io("copyfile", from)),
            Some(Slot::Text(t)) => Slot::Text(t.clone()),
            Some(Slot::Moved(src) | Slot::Copied(src)) => Slot::Copied(src.clone()),
            None if self.disk.stat(&a).is_ok_and(|s| s.kind == Kind::File) => {
                Slot::Copied(a.clone())
            }
            None => return Err(io("copyfile", from)),
        };
        self.tick += 1;
        self.slots.insert(b, slot);
        Ok(())
    }

    fn is_link(&self, path: &str) -> bool {
        let p = super::posix(path);
        !self.slots.contains_key(&p)
            && std::fs::symlink_metadata(super::real(&p)).is_ok_and(|m| m.file_type().is_symlink())
    }

    fn mkdir_all(&mut self, dir: &str) -> Result<(), SourceError> {
        let p = self.confined(dir)?;
        if self.slots.get(&p).is_some_and(|s| !matches!(s, Slot::Gone))
            || (!self.slots.contains_key(&p)
                && self.disk.stat(&p).is_ok_and(|s| s.kind == Kind::File))
        {
            return Err(SourceError::Io(format!(
                "EEXIST: file already exists, mkdir '{p}'"
            )));
        }
        self.dirs.insert(p);
        Ok(())
    }
}
