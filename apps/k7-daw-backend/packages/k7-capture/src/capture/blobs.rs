//! blobs.rs — the capture's binaries: a ref written for another replica, the files waiting for bytes, and the cache's copies
//! (`writeBlob`, `await`, `place`, `fetched`, `blobFile`). What a reader gets wrong: `pending.json` is saved BEFORE the
//! ledger marks a file pending (2134), since a restart reads a pending file absent from it as removed.

use std::io;
use std::path::Path;

use indexmap::IndexMap;
use k7_js::number::text_hash;
use k7_replica::blob::{BlobId, parse_blob_ref};

use super::LiveCapture;
use crate::cache::place_file;
use crate::ledger::{Known, mtime_ms, stamp_of};

/// What `fetched` wrote, and the failure that stopped it: the files left stay pending, and the bytes stay cached to try again.
#[derive(Default)]
pub struct Fetched {
    pub placed: Vec<String>,
    pub failed: Option<io::Error>,
}

impl LiveCapture {
    /// A path holding blob `hash`'s bytes, copied into the cache from a file that has them when needed.
    pub fn blob_file(&mut self, hash: &str) -> io::Result<Option<String>> {
        let Some(cache) = self.cache.as_ref() else {
            return Ok(None);
        };
        let src = if cache.has(hash) {
            None
        } else {
            self.holder(hash)
        };
        let copy = src.map(|s| cache.snapshot(&s)).transpose()?;
        // The file moved on between its stamp and the copy: those bytes are not the blob asked for.
        if let Some(c) = copy.filter(|c| c.id.hash != hash && c.fresh) {
            cache.evict(&c.id.hash);
        }
        Ok(cache.has(hash).then(|| cache.path(hash)))
    }

    /// The cache's copy of `hash` reached the store: it no longer has to be kept.
    pub fn uploaded(&self, hash: &str) {
        if !self.pending_on(hash)
            && let Some(c) = &self.cache
        {
            c.evict(hash);
        }
    }

    /// True while some file waits for blob `hash`.
    pub fn pending_on(&self, hash: &str) -> bool {
        self.ledger.values().any(|k| {
            k.pending && parse_blob_ref(k.r#ref.as_deref()).is_some_and(|b| b.hash == hash)
        })
    }

    /// The cache holds blob `hash` now: every file waiting for it is written, and the copy dropped; answers those files.
    pub fn fetched(&mut self, hash: &str) -> Fetched {
        let mut out = Fetched::default();
        let Some(cache) = self.cache.as_ref().filter(|c| c.has(hash)) else {
            return out;
        };
        let src = cache.path(hash);
        let waiting: Vec<(String, String)> = self
            .ledger
            .iter()
            .filter(|(_, k)| k.pending)
            .filter_map(|(rel, k)| {
                let r = k.r#ref.clone()?;
                (parse_blob_ref(Some(&r))?.hash == hash).then(|| (rel.clone(), r))
            })
            .collect();
        for (rel, r) in waiting {
            match self.place(&rel, &src, &r) {
                Ok(()) => out.placed.push(rel),
                Err(e) => {
                    out.failed = Some(e);
                    break;
                }
            }
        }
        // What was placed is saved even when a later file failed, so a restart does not read it as pending.
        if !out.placed.is_empty()
            && let Err(e) = self.save_pending()
        {
            out.failed.get_or_insert(e);
        }
        if out.failed.is_none()
            && let Some(c) = &self.cache
        {
            c.evict(hash);
        }
        out
    }

    /// A ref written for another replica: the bytes from the cache or a local file, else the file waits for a fetch.
    pub(super) fn write_blob(&mut self, file: &str, r#ref: &str, blob: &BlobId) -> io::Result<()> {
        let k = self.ledger.get(file);
        if k.is_some_and(|k| k.r#ref.as_deref() == Some(r#ref) && !k.pending)
            && Path::new(&self.path_of(file)?).exists()
        {
            return Ok(());
        }
        let cached = self
            .cache
            .as_ref()
            .filter(|c| c.has(&blob.hash))
            .map(|c| c.path(&blob.hash));
        match cached.or_else(|| self.holder(&blob.hash)) {
            Some(src) => {
                self.place(file, &src, r#ref)?;
                self.save_pending()
            }
            None => self.await_bytes(file, r#ref),
        }
    }

    /// `file` is known by `ref` and waits for its bytes; whatever is on disk there stays until they arrive.
    pub(super) fn await_bytes(&mut self, file: &str, r#ref: &str) -> io::Result<()> {
        if parse_blob_ref(Some(r#ref)).is_none() {
            return Ok(());
        }
        let mut files = self.pending_files();
        files.insert(file.to_string(), r#ref.to_string());
        if let Some(c) = &self.cache {
            c.save_pending(&files)?;
        }
        self.mark_pending(file, r#ref);
        Ok(())
    }

    /// The ledger marks `file` as waiting for `ref`'s bytes, saving nothing: the caller saved `pending.json` or read it.
    pub(super) fn mark_pending(&mut self, file: &str, r#ref: &str) {
        let Some(blob) = parse_blob_ref(Some(r#ref)) else {
            return;
        };
        let known = Known {
            stamp: String::new(),
            hash: text_hash(r#ref),
            text: None,
            r#ref: Some(r#ref.to_string()),
            pending: true,
        };
        self.ledger.insert(file.to_string(), known);
        self.missing.push(blob);
    }

    /// Copy `src` into place as `file`, and record it as `ref` before the watcher reports it.
    fn place(&mut self, file: &str, src: &str, r#ref: &str) -> io::Result<()> {
        let abs = self.write_path_of(file)?;
        if let Some(dir) = Path::new(&abs).parent() {
            std::fs::create_dir_all(dir)?;
        }
        place_file(src, &abs)?;
        let md = std::fs::metadata(&abs)?;
        let known = Known {
            stamp: stamp_of(md.len(), mtime_ms(&md)),
            hash: text_hash(r#ref),
            text: None,
            r#ref: Some(r#ref.to_string()),
            pending: false,
        };
        self.ledger.insert(file.to_string(), known);
        self.publish(file);
        Ok(())
    }

    /// A workspace file whose ledger ref names `hash` and whose stamp has not moved since.
    fn holder(&self, hash: &str) -> Option<String> {
        self.ledger.iter().find_map(|(rel, k)| {
            if k.pending || parse_blob_ref(k.r#ref.as_deref())?.hash != hash {
                return None;
            }
            let abs = self.path_of(rel).ok()?;
            let md = std::fs::metadata(&abs).ok()?;
            (k.stamp == stamp_of(md.len(), mtime_ms(&md))).then_some(abs)
        })
    }

    /// The files waiting for bytes, by path, for the worker to publish.
    pub fn pending_paths(&self) -> Vec<String> {
        let waiting = self.ledger.iter().filter(|(_, k)| k.pending);
        waiting.map(|(rel, _)| rel.clone()).collect()
    }

    fn pending_files(&self) -> IndexMap<String, String> {
        self.ledger
            .iter()
            .filter(|(_, k)| k.pending)
            .filter_map(|(rel, k)| Some((rel.clone(), k.r#ref.clone()?)))
            .collect()
    }

    pub(super) fn save_pending(&self) -> io::Result<()> {
        match &self.cache {
            Some(c) => c.save_pending(&self.pending_files()),
            None => Ok(()),
        }
    }
}
