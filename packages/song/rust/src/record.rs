// record.rs — a Source that remembers what was read through it, as the bundle a MemSource is made from.
// A reader would get wrong NotFound: the bundle then needs the listing of the folder above, or the browser asks for more forever.
// Everything a call answered is kept (Outside, Io, not UTF-8); a bundle past the limits stops growing and `overflow` says so.

use crate::source::path::{is_within, normalize, parent, parent_str};
use crate::source::{
    BundleFile, BundleRequest, Entry, Kind, MAX_BUNDLE, MAX_ENTRIES, Source, SourceBundle, SourceError, Span, Stat,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Mutex, PoisonError};

#[derive(Default)]
struct Seen {
    texts: BTreeMap<String, String>,
    lens: BTreeMap<String, u64>,
    /// Whole reads that were not UTF-8.
    binary: BTreeSet<String>,
    /// Reads that failed after a good stat, with the message.
    ios: BTreeMap<String, String>,
    lists: BTreeMap<String, Vec<Entry>>,
    /// Paths that answered NotFound, which the bundle keeps unless a listing it holds proves them absent.
    absent: BTreeSet<String>,
    refused: BTreeSet<String>,
    failed_stats: BTreeMap<String, String>,
    failed_lists: BTreeMap<String, String>,
    /// The digests the cache computed, kept for the files the bundle holds no text of.
    digests: BTreeMap<String, String>,
    /// Bytes held, names included, and the paths named in the capped sets.
    weight: usize,
    entries: usize,
    overflow: Option<String>,
}

impl Seen {
    /// Whether `bytes` more (and one more capped path when `entry`) still fit; past the limits nothing is added.
    fn room(&mut self, bytes: usize, entry: bool) -> bool {
        if self.overflow.is_some() {
            return false;
        }
        if self.weight.saturating_add(bytes) > MAX_BUNDLE {
            self.overflow = Some(format!("the bundle would hold more than {MAX_BUNDLE} bytes"));
        } else if entry && self.entries >= MAX_ENTRIES {
            self.overflow = Some(format!(
                "the bundle would name more than {MAX_ENTRIES} refused or failed paths"
            ));
        }
        if self.overflow.is_some() {
            return false;
        }
        self.weight += bytes;
        self.entries += usize::from(entry);
        true
    }

    fn text(&mut self, path: String, text: String) {
        let (new, old) = (
            path.len() + text.len(),
            self.texts.get(&path).map_or(0, |t| path.len() + t.len()),
        );
        if self.room(new.saturating_sub(old), false) {
            self.texts.insert(path, text);
        }
    }

    fn len(&mut self, path: String, len: u64) {
        if !self.lens.contains_key(&path) && self.room(path.len() + 8, false) {
            self.lens.insert(path, len);
        }
    }

    /// The length of a file read whole, which is the one the bytes had and not the stat's of a moment before.
    fn read_len(&mut self, path: String, len: u64) {
        match self.lens.get_mut(&path) {
            Some(held) => *held = len,
            None => self.len(path, len),
        }
    }

    fn absent(&mut self, path: &str) {
        if !self.absent.contains(path) && self.room(path.len(), true) {
            self.absent.insert(path.to_owned());
        }
    }

    fn list(&mut self, dir: String, entries: Vec<Entry>) {
        let weigh = |d: &str, es: &[Entry]| d.len() + es.iter().map(|e| e.name.len() + 8).sum::<usize>();
        let old = self.lists.get(&dir).map_or(0, |es| weigh(&dir, es));
        if self.room(weigh(&dir, &entries).saturating_sub(old), false) {
            self.lists.insert(dir, entries);
        }
    }

    fn refuse(&mut self, path: String) {
        if !self.refused.contains(&path) && self.room(path.len(), true) {
            self.refused.insert(path);
        }
    }

    fn fail(&mut self, path: String, message: String, stat: bool) {
        let held = if stat { &self.failed_stats } else { &self.failed_lists };
        if !held.contains_key(&path) && self.room(path.len() + message.len(), true) {
            let into = if stat {
                &mut self.failed_stats
            } else {
                &mut self.failed_lists
            };
            into.insert(path, message);
        }
    }

    fn digest(&mut self, path: String, hex: &str) {
        if !self.digests.contains_key(&path) && self.room(path.len() + hex.len(), true) {
            self.digests.insert(path, hex.to_owned());
        }
    }

    fn io(&mut self, path: String, message: String) {
        if !self.ios.contains_key(&path) && self.room(path.len() + message.len(), false) {
            self.ios.insert(path, message);
        }
    }
}

/// What a host adds to a bundle beyond what the reads showed: the compile's own facts.
#[derive(Clone, Debug, Default)]
pub struct Extras {
    pub jsdsp_stale: BTreeSet<String>,
    pub lengths: BTreeMap<String, f64>,
    pub request: Option<BundleRequest>,
}

pub struct Recorder<'a> {
    inner: &'a dyn Source,
    seen: Mutex<Seen>,
}

impl<'a> Recorder<'a> {
    pub fn new(inner: &'a dyn Source) -> Recorder<'a> {
        Recorder {
            inner,
            seen: Mutex::new(Seen::default()),
        }
    }

    fn seen(&self) -> std::sync::MutexGuard<'_, Seen> {
        self.seen.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The sentence for a bundle that outgrew a limit, which must not be shipped; None while it fits.
    pub fn overflow(&self) -> Option<String> {
        self.seen().overflow.clone()
    }

    /// Keeps what a failed call answered: a refusal, or an `Io` with its message, for a `stat` or a `list`.
    fn note_error(&self, path: &str, e: &SourceError, stat: bool) {
        match e {
            SourceError::NotFound => self.note_absent(path),
            SourceError::Outside => self.seen().refuse(path.to_owned()),
            SourceError::Io(m) => self.seen().fail(path.to_owned(), m.clone(), stat),
            SourceError::Missing => {}
        }
    }

    /// Records a path that is not there and the nearest listing above it, which usually proves it.
    fn note_absent(&self, path: &str) {
        self.seen().absent(path);
        let mut dir = parent(path);
        while let Some(d) = dir {
            // A folder already listed or already absent is not asked twice: 8,000 absent files are not 8,000 listings.
            if self.seen().lists.contains_key(&d) {
                return;
            }
            if !self.seen().absent.contains(&d) {
                match self.inner.list(&d) {
                    Ok(entries) => {
                        self.seen().list(d, entries);
                        return;
                    }
                    Err(SourceError::NotFound) => self.seen().absent(&d),
                    // The Recorder's own probe: what it could not list is not something the compile asked.
                    Err(_) => {}
                }
            }
            dir = parent(&d);
        }
    }

    /// The bundle of everything read: texts as text, every other file by its length.
    pub fn into_bundle(self, mounts: &[String]) -> SourceBundle {
        self.into_bundle_with_files(mounts, &[])
    }

    /// The bundle of a Source that serves folder mounts and file mounts (the engine's scan).
    pub fn into_bundle_with_files(self, mounts: &[String], files_mounted: &[String]) -> SourceBundle {
        self.into_bundle_with(mounts, files_mounted, Extras::default())
    }

    /// The same, with the compile's facts: what was stale, the lengths it measured, and what a replay must ask.
    pub fn into_bundle_with(self, mounts: &[String], files_mounted: &[String], extras: Extras) -> SourceBundle {
        let seen = self.seen.into_inner().unwrap_or_else(PoisonError::into_inner);
        let mut files: BTreeMap<String, BundleFile> = BTreeMap::new();
        for (p, n) in seen.lens {
            let binary = seen.binary.contains(&p).then_some(true);
            files.insert(
                p,
                BundleFile {
                    len: Some(n),
                    binary,
                    ..BundleFile::default()
                },
            );
        }
        // A failed read keeps the length its stat gave.
        for (p, m) in seen.ios {
            if let Some(f) = files.get_mut(&p) {
                f.binary = None;
                f.io = Some(m);
            }
        }
        for (p, t) in seen.texts {
            files.insert(
                p,
                BundleFile {
                    text: Some(t),
                    ..BundleFile::default()
                },
            );
        }
        let lists = seen.lists;
        let mut digests = seen.digests;
        digests.retain(|p, _| !files.get(p).is_some_and(|f| f.text.is_some()));
        // A listing held above a path that omits it already proves it absent.
        let mut absent = seen.absent;
        absent.retain(|p| {
            let name = p.rsplit('/').next().unwrap_or(p);
            let listed = parent_str(p).and_then(|d| lists.get(d));
            !listed.is_some_and(|es| es.iter().all(|e| e.name != name))
        });
        // A refusal inside a mount stays, with a mount below it too: the replay answers it for itself alone.
        let dirs: Vec<String> = mounts
            .iter()
            .chain(files_mounted)
            .filter_map(|m| normalize(m).ok())
            .collect();
        let mut refused = seen.refused;
        refused.retain(|p| dirs.iter().any(|m| is_within(m, p)));
        SourceBundle {
            format: 1,
            mounts: mounts.iter().chain(files_mounted).cloned().collect(),
            files,
            lists,
            refused,
            failed_stats: seen.failed_stats,
            failed_lists: seen.failed_lists,
            absent,
            digests,
            jsdsp_stale: extras.jsdsp_stale,
            lengths: extras.lengths,
            measured: false,
            request: extras.request,
            overflow: seen.overflow,
        }
    }
}

impl Source for Recorder<'_> {
    fn stamps_are_times(&self) -> bool {
        self.inner.stamps_are_times()
    }

    fn folder_id(&self, path: &str) -> Option<String> {
        self.inner.folder_id(path)
    }

    fn note_digest(&self, path: &str, hex: &str) {
        if let Ok(norm) = normalize(path) {
            self.seen().digest(norm, hex);
        }
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

    fn note_text(&self, path: &str, text: &std::sync::Arc<str>) {
        if let Ok(norm) = normalize(path) {
            self.seen().text(norm, text.to_string());
        }
    }

    fn stat(&self, path: &str) -> Result<Stat, SourceError> {
        let r = self.inner.stat(path);
        let Ok(norm) = normalize(path) else { return r };
        match &r {
            Ok(st) if st.kind == Kind::File => self.seen().len(norm, st.len),
            Ok(_) => match self.inner.list(&norm) {
                Ok(entries) => self.seen().list(norm, entries),
                Err(e) => self.note_error(&norm, &e, false),
            },
            Err(e) => self.note_error(&norm, e, true),
        }
        r
    }

    fn list(&self, dir: &str) -> Result<Vec<Entry>, SourceError> {
        let r = self.inner.list(dir);
        let Ok(norm) = normalize(dir) else { return r };
        match &r {
            Ok(entries) => self.seen().list(norm, entries.clone()),
            Err(e) => self.note_error(&norm, e, false),
        }
        r
    }

    fn read(&self, path: &str, span: Span) -> Result<Vec<u8>, SourceError> {
        let r = self.inner.read(path, span);
        let Ok(norm) = normalize(path) else { return r };
        match (&r, span) {
            (Ok(bytes), Span::All) => match std::str::from_utf8(bytes) {
                Ok(text) => self.seen().text(norm, text.to_owned()),
                Err(_) => {
                    let mut seen = self.seen();
                    if seen.room(norm.len(), false) {
                        seen.binary.insert(norm.clone());
                    }
                    seen.read_len(norm, bytes.len() as u64);
                }
            },
            (Ok(_), Span::Range { .. }) => {
                if let Ok(st) = self.inner.stat(&norm) {
                    self.seen().len(norm, st.len);
                }
            }
            (Err(SourceError::Io(m)), _) => {
                // The stat says whether the file is there: a failed read of a good file keeps its length.
                if matches!(self.stat(&norm), Ok(st) if st.kind == Kind::File) {
                    self.seen().io(norm, m.clone());
                }
            }
            (Err(e), _) => self.note_error(&norm, e, true),
        }
        r
    }
}
