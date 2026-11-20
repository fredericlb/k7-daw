// readset.rs — what a read touched: every file's stamp, every listing, every absent path, re-checked before a result is reused.
// A reader would get wrong that a listing is compared by its ENTRIES: a same-tick rename moves no stamp, but it moves a name.

use crate::source::path::normalize;
use crate::source::{Entry, Kind, Source, SourceError, Span, Stamp, Stat};
use indexmap::{IndexMap, IndexSet};
use std::sync::{Arc, Mutex, PoisonError};

/// Why a path was refused, which a re-check must see again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Refusal {
    Outside,
    Io,
}

/// A dot name is no module's: the walk skips it, so the read set never looks at one.
fn is_dot(e: &Entry) -> bool {
    e.name.starts_with('.')
}

/// Everything a read asked of a Source, and what it was answered.
#[derive(Clone, Debug, Default)]
pub struct ReadSet {
    files: IndexMap<String, Stamp>,
    dirs: IndexSet<String>,
    lists: IndexMap<String, Vec<Entry>>,
    absent: IndexSet<String>,
    refused: IndexMap<String, Refusal>,
    texts: IndexMap<String, Arc<str>>,
    /// Whole reads that were not UTF-8: a reuse reads them again for a Recorder, which keeps no text of them.
    binaries: IndexSet<String>,
}

impl ReadSet {
    /// How many things were touched.
    pub fn len(&self) -> usize {
        self.files.len() + self.dirs.len() + self.lists.len() + self.absent.len() + self.refused.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Whether every file stamp is one no later write could reproduce: only then may the set key a cache.
    pub fn settled(&self) -> bool {
        self.files.values().all(|s| s.settled)
    }

    /// The paths of the files touched.
    pub fn files(&self) -> impl Iterator<Item = &str> {
        self.files.keys().map(String::as_str)
    }

    /// Whether `source` still answers as it did: files by stamp, listings by entries, absent paths still absent.
    /// A path the source cannot answer (`Missing`) is a change: the rebuild decides what that means.
    pub fn valid(&self, source: &dyn Source) -> bool {
        for (path, stamp) in &self.files {
            match source.stat(path) {
                Ok(Stat {
                    kind: Kind::File,
                    stamp: now,
                    ..
                }) if now == *stamp => {}
                _ => return false,
            }
        }
        for path in &self.dirs {
            if !matches!(source.stat(path), Ok(Stat { kind: Kind::Dir, .. })) {
                return false;
            }
        }
        for (dir, entries) in &self.lists {
            match source.list(dir) {
                Ok(now) if now.iter().filter(|e| !is_dot(e)).eq(entries.iter()) => {}
                _ => return false,
            }
        }
        for path in &self.absent {
            if !matches!(source.stat(path), Err(SourceError::NotFound)) {
                return false;
            }
        }
        for (path, why) in &self.refused {
            let now = match source.stat(path) {
                Err(SourceError::Outside) => Some(Refusal::Outside),
                Err(SourceError::Io(_)) => Some(Refusal::Io),
                _ => None,
            };
            if now != Some(*why) {
                return false;
            }
        }
        true
    }

    /// Whether anything this set read now has an mtime later than `since_ns`, or appeared, or went (decision 717); a hash stamp orders nothing.
    pub fn moved_after(&self, source: &dyn Source, since_ns: u64) -> bool {
        if !source.stamps_are_times() {
            return false;
        }
        let later = |p: &String| source.stat(p).map_or(true, |st| st.stamp.mtime_ns > since_ns);
        self.files.keys().any(later)
            || self.dirs.iter().any(later)
            || self.lists.keys().any(later)
            || self.absent.iter().any(|p| source.stat(p).is_ok())
    }

    /// Whether anything this set asked was refused by the Source.
    pub fn has_refused(&self) -> bool {
        !self.refused.is_empty()
    }

    /// The bytes of the texts this set holds.
    pub fn text_bytes(&self) -> usize {
        self.texts.values().map(|t| t.len()).sum()
    }

    /// Hands every text this set read to `source`, so a Recorder holds what a reused result was made of.
    pub fn replay(&self, source: &dyn Source) {
        for (path, text) in &self.texts {
            source.note_text(path, text);
        }
        for path in &self.binaries {
            let _ = source.read(path, Span::All);
        }
    }
}

/// A Source that records what is asked of it into a ReadSet.
pub struct Watch<'a> {
    inner: &'a dyn Source,
    set: Mutex<ReadSet>,
}

impl<'a> Watch<'a> {
    pub fn new(inner: &'a dyn Source) -> Watch<'a> {
        Watch {
            inner,
            set: Mutex::new(ReadSet::default()),
        }
    }

    fn set(&self) -> std::sync::MutexGuard<'_, ReadSet> {
        self.set.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn into_set(self) -> ReadSet {
        self.set.into_inner().unwrap_or_else(PoisonError::into_inner)
    }

    fn note_error(&self, path: &str, e: &SourceError) {
        let mut set = self.set();
        match e {
            SourceError::NotFound => {
                set.absent.insert(path.to_owned());
            }
            SourceError::Outside => {
                set.refused.insert(path.to_owned(), Refusal::Outside);
            }
            SourceError::Io(_) => {
                set.refused.insert(path.to_owned(), Refusal::Io);
            }
            SourceError::Missing => {}
        }
    }
}

impl Source for Watch<'_> {
    fn stamps_are_times(&self) -> bool {
        self.inner.stamps_are_times()
    }

    fn folder_id(&self, path: &str) -> Option<String> {
        self.inner.folder_id(path)
    }

    fn note_digest(&self, path: &str, hex: &str) {
        self.inner.note_digest(path, hex);
    }

    fn known_digest(&self, path: &str) -> Option<String> {
        self.inner.known_digest(path)
    }

    fn known_stale(&self, source: &str) -> Option<bool> {
        self.inner.known_stale(source)
    }

    fn known_seconds(&self, path: &str) -> Option<f64> {
        self.inner.known_seconds(path)
    }

    fn measured_by_table(&self) -> bool {
        self.inner.measured_by_table()
    }

    fn stat(&self, path: &str) -> Result<Stat, SourceError> {
        let r = self.inner.stat(path);
        let Ok(norm) = normalize(path) else { return r };
        match &r {
            Ok(st) if st.kind == Kind::File => {
                self.set().files.insert(norm, st.stamp);
            }
            Ok(_) => {
                self.set().dirs.insert(norm);
            }
            Err(e) => self.note_error(&norm, e),
        }
        r
    }

    fn list(&self, dir: &str) -> Result<Vec<Entry>, SourceError> {
        let r = self.inner.list(dir);
        let Ok(norm) = normalize(dir) else { return r };
        match &r {
            Ok(entries) => {
                let kept: Vec<Entry> = entries.iter().filter(|e| !is_dot(e)).cloned().collect();
                self.set().lists.insert(norm, kept);
            }
            Err(e) => self.note_error(&norm, e),
        }
        r
    }

    fn read(&self, path: &str, span: Span) -> Result<Vec<u8>, SourceError> {
        let r = self.inner.read(path, span);
        let Ok(norm) = normalize(path) else { return r };
        match (&r, span) {
            (Ok(bytes), Span::All) => match std::str::from_utf8(bytes) {
                Ok(text) => {
                    self.set().texts.insert(norm, Arc::from(text));
                }
                Err(_) => {
                    self.set().binaries.insert(norm);
                }
            },
            (Err(e), _) => self.note_error(&norm, e),
            _ => {}
        }
        r
    }

    fn note_text(&self, path: &str, text: &Arc<str>) {
        if let Ok(norm) = normalize(path) {
            self.set().texts.insert(norm, text.clone());
        }
        self.inner.note_text(path, text);
    }
}
