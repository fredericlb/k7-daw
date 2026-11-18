//! learn.rs — the ledger built from the disk or from the room's confirmed text, and one file's entry as it is now
//! (`learning`, `look`, `probe`, `learnBaseline`). What a reader gets wrong: a stamp read unsettled is kept empty, so the
//! next look compares by hash and a same-size rewrite inside one timestamp tick is not missed.

use std::io::{self, ErrorKind};
use std::sync::Arc;

use indexmap::IndexMap;
use k7_js::number::text_hash;
use k7_replica::blob::{BlobId, blob_ref, parse_blob_ref};

use super::{Baseline, LiveCapture, files_under};
use crate::cache::{sha256_bytes, sha256_file};
use crate::disk::confined_path;
use crate::ledger::{Known, SETTLE_MS, keeps_text, looks_binary, mtime_ms, stamp_of, text_ceiling};

/// A file as it is: its ledger entry, its text (or its blob ref when it is opaque), and whether a copy was newly kept.
pub struct Probed {
    pub now: Known,
    pub text: String,
    pub fresh: Option<bool>,
}

/// What `look` found: a file that cannot be read now is neither present nor absent.
pub(super) enum Look {
    Deferred,
    Absent,
    Present(Known),
}

impl LiveCapture {
    /// Read the workspace into the ledger WITHOUT emitting; `baseline` makes the ledger what the room last confirmed.
    pub fn prepare(&mut self, baseline: Option<Arc<dyn Baseline>>) -> io::Result<()> {
        self.baseline = baseline.clone();
        let learned = match baseline {
            Some(b) => self.learn_baseline(&*b),
            None => self.walk("").map(|files| {
                for rel in files {
                    self.learn(&rel);
                }
            }),
        };
        // A half-built ledger would read what it missed as removed: a failed prepare leaves none.
        if learned.is_err() {
            self.ledger.clear();
            self.baseline = None;
        }
        self.prepared = learned.is_ok();
        learned
    }

    /// The ledger as the baseline says the files were; one the disk still holds as it was keeps the walk's entry.
    fn learn_baseline(&mut self, baseline: &dyn Baseline) -> io::Result<()> {
        let mut walked: IndexMap<String, Known> = IndexMap::new();
        for rel in self.walk("")? {
            if let Look::Present(k) = self.look_or_defer(&rel) {
                walked.insert(rel, k);
            }
        }
        for rel in baseline.files()? {
            let Some(text) = baseline.get(&rel)? else {
                continue;
            };
            if rel.split('/').any(self.o.skip) {
                continue;
            }
            // Node followed links and this walk never does: what the room holds through one is forgotten, never removed.
            if self.reached_by_link(&rel)? {
                continue;
            }
            let hash = text_hash(&text);
            let r#ref = parse_blob_ref(Some(&text)).map(|_| text.clone());
            // The same content as on disk: the walk's entry, with its real stamp, spares the first sweep reading it again.
            if let Some(w) = walked
                .get(&rel)
                .filter(|w| w.hash == hash && w.r#ref == r#ref)
            {
                self.ledger.insert(rel, w.clone());
                continue;
            }
            // An empty stamp never matches the disk's, so the first sweep compares by hash and text.
            let kept = (keeps_text(&rel) && r#ref.is_none()).then_some(text);
            self.ledger.insert(
                rel,
                Known {
                    stamp: String::new(),
                    hash,
                    text: kept,
                    r#ref,
                    pending: false,
                },
            );
        }
        Ok(())
    }

    /// Every file as the room would hold it now (text, or a blob ref when opaque), read from the disk: a history's first entry.
    pub fn texts(&mut self) -> io::Result<Vec<(String, String)>> {
        let mut out = Vec::new();
        for rel in self.walk("")? {
            if self.reached_by_link(&rel)? {
                continue;
            }
            match self.probe(&rel, false, None) {
                Ok(p) => out.push((rel, p.text)),
                Err(e) if e.kind() == ErrorKind::NotFound => {}
                Err(e) => return Err(e),
            }
        }
        Ok(out)
    }

    fn reached_by_link(&self, rel: &str) -> io::Result<bool> {
        if super::through_link(&self.o.root, rel)? {
            return Ok(true);
        }
        let Ok(abs) = confined_path(&self.o.root, rel) else {
            return Ok(false);
        };
        Ok(std::fs::symlink_metadata(abs).is_ok_and(|m| m.file_type().is_symlink()))
    }

    pub(super) fn learn(&mut self, rel: &str) {
        match self.look_or_defer(rel) {
            Look::Present(k) => {
                self.ledger.insert(rel.to_string(), k);
            }
            Look::Absent => {
                self.ledger.shift_remove(rel);
            }
            Look::Deferred => {}
        }
    }

    /// `look`, or `Deferred` for a file that cannot be read now: the ledger keeps what it knew, and a scan reads it again.
    fn look_or_defer(&mut self, rel: &str) -> Look {
        match self.look(rel) {
            Ok(Some(k)) => Look::Present(k),
            Ok(None) => Look::Absent,
            Err(e) => {
                self.deferred.skip(rel, &e);
                Look::Deferred
            }
        }
    }

    /// The text of a k7x file the ledger knows by hash alone, from the baseline when it holds those very bytes.
    pub(super) fn confirmed_text(
        &self,
        rel: &str,
        known: Option<&Known>,
    ) -> io::Result<Option<String>> {
        let (Some(k), Some(b)) = (known, &self.baseline) else {
            return Ok(None);
        };
        if k.r#ref.is_some() || !keeps_text(rel) {
            return Ok(None);
        }
        Ok(b.get(rel)?.filter(|t| text_hash(t) == k.hash))
    }

    /// The entry for `rel` as it is, or None when absent; a text file whose stamp the index knows is not read.
    fn look(&mut self, rel: &str) -> io::Result<Option<Known>> {
        let abs = self.path_of(rel)?;
        let md = match std::fs::metadata(&abs) {
            Ok(m) => m,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        let stamp = stamp_of(md.len(), mtime_ms(&md));
        // A k7x file's text is needed to diff its next change: known by hash only when the baseline can give it back.
        let by_hash = !keeps_text(rel) || self.baseline.is_some();
        let hash = if !by_hash || self.opaque_by_name(rel, md.len()) {
            None
        } else {
            self.index
                .as_ref()
                .and_then(|i| i.text_hash(rel, &stamp))
                .map(str::to_string)
        };
        if let Some(hash) = hash {
            return Ok(Some(Known {
                stamp,
                hash,
                text: None,
                r#ref: None,
                pending: false,
            }));
        }
        Ok(Some(self.probe(rel, false, Some((&abs, &md)))?.now))
    }

    pub(super) fn walk(&self, scope: &str) -> io::Result<Vec<String>> {
        files_under(&self.o.root, scope, self.o.skip)
    }

    pub(super) fn opaque_by_name(&self, rel: &str, size: u64) -> bool {
        (self.o.is_binary)(rel) || size > text_ceiling(rel)
    }

    fn read_file(&mut self, rel: &str, abs: &str) -> io::Result<Vec<u8>> {
        self.count_read(rel);
        std::fs::read(abs)
    }

    /// One workspace read, counted when `track_reads` is on; `&self`, so `peek` counts too.
    pub(super) fn count_read(&self, rel: &str) {
        if let Some(reads) = &self.reads {
            reads
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(rel.to_string());
        }
    }

    /// The file as it is: its entry, and its text or its blob ref when opaque; with `keep` the COPY in the cache is hashed.
    pub(super) fn probe(
        &mut self,
        rel: &str,
        keep: bool,
        seen: Option<(&str, &std::fs::Metadata)>,
    ) -> io::Result<Probed> {
        let (abs, md) = match seen {
            Some((a, m)) => (a.to_string(), m.clone()),
            None => {
                let abs = self.path_of(rel)?;
                let md = std::fs::metadata(&abs)?;
                (abs, md)
            }
        };
        let stamp = stamp_of(md.len(), mtime_ms(&md));
        let opaque = |id: BlobId, fresh: Option<bool>| -> io::Result<Probed> {
            let r = blob_ref(&id.hash, id.size as u64)
                .map_err(|e| io::Error::new(ErrorKind::InvalidData, e))?;
            let now = Known {
                stamp: stamp.clone(),
                hash: text_hash(&r),
                text: None,
                r#ref: Some(r.clone()),
                pending: false,
            };
            Ok(Probed {
                now,
                text: r,
                fresh,
            })
        };
        // An unchanged binary is known by the hash it had: only one about to be sent is read.
        if !keep && let Some(id) = self.index.as_ref().and_then(|i| i.get(rel, &stamp)) {
            return opaque(id, None);
        }
        if self.opaque_by_name(rel, md.len()) {
            let (id, fresh) = self.identify(&abs, keep, None)?;
            return opaque(id, fresh);
        }
        let buf = self.read_file(rel, &abs)?;
        if looks_binary(&buf) {
            let (id, fresh) = self.identify(&abs, keep, Some(&buf))?;
            return opaque(id, fresh);
        }
        let text = String::from_utf8_lossy(&buf).into_owned();
        let hash = text_hash(&text);
        // A stamp read unsettled may be reused by a same-size rewrite in its tick: empty, the next look compares by hash.
        let sure = if self.now() - mtime_ms(&md) > SETTLE_MS {
            stamp
        } else {
            String::new()
        };
        let kept = keeps_text(rel).then(|| text.clone());
        Ok(Probed {
            now: Known {
                stamp: sure,
                hash,
                text: kept,
                r#ref: None,
                pending: false,
            },
            text,
            fresh: None,
        })
    }

    fn identify(
        &self,
        abs: &str,
        keep: bool,
        buf: Option<&[u8]>,
    ) -> io::Result<(BlobId, Option<bool>)> {
        match (&self.cache, keep, buf) {
            (Some(c), true, _) => c.snapshot(abs).map(|s| (s.id, Some(s.fresh))),
            (_, _, Some(b)) => Ok((sha256_bytes(b), None)),
            _ => sha256_file(abs).map(|id| (id, None)),
        }
    }
}
