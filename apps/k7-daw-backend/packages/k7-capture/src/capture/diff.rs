//! diff.rs — what a scan makes of the disk: one scope diffed against the ledger into ops, and the ops closed into a batch
//! (`flushNow`, `diffScope`, `change`, `emit`). What a reader gets wrong: a file or folder that cannot be read now is
//! left as the ledger knew it, so nothing under it is read as removed (2132).

use std::collections::HashSet;
use std::io;
use std::sync::Arc;

use indexmap::{IndexMap, IndexSet};
use k7_replica::blob::parse_blob_ref;
use k7_replica::ops::{batch, file_op_for};
use serde_json::{Map, Value, json};

use super::{Baseline, LiveCapture};
use k7_js::number::text_hash;

use crate::ledger::{Known, SETTLE_MS, looks_binary, mtime_ms, stamp_of, text_ceiling};

/// Past this many dirty paths a flush is one full sweep: each scope costs a pass over the ledger.
pub const DIRTY_SWEEP: usize = 256;

/// What one scan found and has not applied: committed only if no window opened meanwhile (2121), else aborted.
pub struct Step {
    ops: Vec<Value>,
    delta: IndexMap<String, Option<Known>>,
    fresh: Vec<String>,
    scopes: Vec<String>,
    seen: HashSet<String>,
    from_dirty: bool,
    full: bool,
}

impl Step {
    fn new(scopes: Vec<String>, from_dirty: bool, full: bool) -> Step {
        Step {
            ops: vec![],
            delta: IndexMap::new(),
            fresh: vec![],
            scopes,
            seen: HashSet::new(),
            from_dirty,
            full,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    pub fn full(&self) -> bool {
        self.full
    }
}

impl LiveCapture {
    /// Learn the workspace (unless `prepare` did), resume interrupted fetches; against a baseline, what moved while down is a batch.
    pub fn start(&mut self, baseline: Option<Arc<dyn Baseline>>) -> io::Result<()> {
        if self.begin(baseline)? {
            self.flush_now(true)?;
            self.save_index();
        }
        Ok(())
    }

    /// `start` without the baseline's diff, which is a scan of its own: true when one is due, else the index is saved.
    pub fn begin(&mut self, baseline: Option<Arc<dyn Baseline>>) -> io::Result<bool> {
        if !self.prepared {
            self.prepare(baseline.clone())?;
        }
        // A fetch a restart interrupted: the file waits for its bytes again, and pending.json already holds it.
        let waiting = self
            .cache
            .as_ref()
            .map(|c| c.pending())
            .transpose()?
            .unwrap_or_default();
        for (rel, r#ref) in waiting {
            self.mark_pending(&rel, &r#ref);
        }
        let due = baseline.is_some();
        if !due {
            self.save_index();
        }
        Ok(due)
    }

    pub(super) fn save_index(&mut self) {
        let now = self.now();
        if let Some(i) = self.index.as_mut() {
            let _ = i.save(&self.ledger, now);
        }
    }

    /// Emit now whatever changed in `files`: a remote batch is about to write them, and would absorb it.
    pub fn capture_files(&mut self, files: &[String]) -> io::Result<Option<Value>> {
        let step = self.step_files(files)?;
        self.commit(step)
    }

    /// The files among `files` whose disk differs by a stat from what the ledger holds: a scan has not caught up with them.
    /// Reads no binary and hashes nothing opaque, so a write phase may ask; a text read unsettled is compared by its content.
    /// A file waiting for bytes or unreadable is the scan's to settle; a young one counts as uncaptured, or a write phase would hash it.
    pub fn uncaptured(&self, files: &[String]) -> Vec<String> {
        files
            .iter()
            .filter(|rel| self.young.contains(*rel) || self.differs(rel))
            .cloned()
            .collect()
    }

    fn differs(&self, rel: &str) -> bool {
        let known = self.ledger.get(rel);
        let waiting = known.is_some_and(|k| k.pending);
        if waiting || self.young.contains(rel) || self.deferred.holds(rel) {
            return false;
        }
        if rel.split('/').any(self.o.skip) {
            return false;
        }
        let Ok(abs) = self.path_of(rel) else {
            return false;
        };
        let md = match std::fs::symlink_metadata(&abs) {
            Ok(md) if md.is_file() => md,
            Ok(md) if md.file_type().is_symlink() => return false,
            Ok(_) => return known.is_some(),
            Err(e) if e.kind() == io::ErrorKind::NotFound => return known.is_some(),
            Err(_) => return false,
        };
        let Some(k) = known else { return true };
        let stamp = stamp_of(md.len(), mtime_ms(&md));
        if !k.stamp.is_empty() {
            return k.stamp != stamp;
        }
        // A text read unsettled keeps no stamp, since a same-size rewrite in its tick would keep it: its content says.
        if md.len() > text_ceiling(rel) {
            return true;
        }
        match std::fs::read(&abs) {
            Ok(buf) => looks_binary(&buf) || text_hash(&String::from_utf8_lossy(&buf)) != k.hash,
            Err(_) => false,
        }
    }

    /// Diff everything the watcher reported (or the whole tree) and emit it as one batch now.
    pub fn flush_now(&mut self, full: bool) -> io::Result<Option<Value>> {
        let step = self.step_flush(full)?;
        self.commit(step)
    }

    /// The scan of `flush_now`, changing neither ledger nor feed: `commit` applies it, `abort` gives the dirty paths back.
    pub fn step_flush(&mut self, full: bool) -> io::Result<Step> {
        self.quiet_armed = false;
        let full = full || self.dirty.len() > DIRTY_SWEEP;
        let scopes = if full {
            self.dirty.clear();
            vec![String::new()]
        } else {
            self.dirty.drain(..).collect()
        };
        self.scan(Step::new(scopes, !full, full))
    }

    /// The scan of `capture_files`: only the named paths.
    pub fn step_files(&mut self, files: &[String]) -> io::Result<Step> {
        let scopes: IndexSet<&String> = files.iter().collect();
        let scopes = scopes.into_iter().cloned().collect();
        self.scan(Step::new(scopes, false, false))
    }

    fn scan(&mut self, mut step: Step) -> io::Result<Step> {
        for scope in step.scopes.clone() {
            if let Err(e) = self.diff_scope(&scope, &mut step) {
                self.abort(step);
                return Err(e);
            }
        }
        self.recheck_young();
        Ok(step)
    }

    /// Apply a scan, and close its ops into one batch; None when it found nothing.
    pub fn commit(&mut self, step: Step) -> io::Result<Option<Value>> {
        // The id first: a failure leaves the ledger as it was, so the next scan finds the edits again.
        let id = if step.ops.is_empty() {
            String::new()
        } else {
            (self.o.new_id)()?
        };
        if step.full {
            self.window_closed = false;
            self.last_full = self.now();
        }
        let mut removed = HashSet::new();
        for (rel, known) in step.delta {
            match known {
                Some(k) => {
                    self.ledger.insert(rel, k);
                }
                None => {
                    removed.insert(rel);
                }
            }
        }
        if !removed.is_empty() {
            let mut visited = 0;
            self.ledger.retain(|rel, _| {
                visited += 1;
                !removed.contains(rel)
            });
            self.work += visited;
        }
        if step.ops.is_empty() {
            return Ok(None);
        }
        if step.from_dirty || step.full {
            self.last_active = self.now();
            self.save_index();
        }
        Ok(Some(self.emit(step.ops, &id)))
    }

    /// A scan dropped: the dirty paths it took come back, the snapshots it kept go, and the next flush sweeps whole.
    pub fn abort(&mut self, step: Step) {
        if step.from_dirty {
            self.dirty.extend(step.scopes);
        }
        if step.full {
            self.window_closed = true;
        }
        // A blob some file waits for may be the very download that landed during the scan: it stays.
        for hash in step.fresh.iter().filter(|h| !self.pending_on(h)) {
            if let Some(c) = &self.cache {
                c.evict(hash);
            }
        }
    }

    /// Binaries left waiting are reported again once they had time to settle.
    pub(super) fn recheck_young(&mut self) {
        if self.young.is_empty() || self.young_due.is_some() {
            return;
        }
        self.young_due = Some(self.now() + self.o.binary_settle_ms);
    }

    /// The ops closed into one batch, labelled by the windows closed since the last.
    fn emit(&mut self, ops: Vec<Value>, id: &str) -> Value {
        // A sweep finds what a dead or lossy watcher never reported, so its files are news too.
        for op in &ops {
            if let Some(f) = op.get("file").and_then(Value::as_str) {
                self.publish(f);
            }
        }
        self.local += 1;
        let label = if self.labels.is_empty() {
            "edit".to_string()
        } else {
            self.labels.join(" + ")
        };
        let ts = self.now() as u64;
        let b: Map<String, Value> = batch(id, &self.o.replica, self.local, ts, &label, ops);
        self.labels.clear();
        let b = Value::Object(b);
        self.emitted.push(b.clone());
        b
    }

    /// One reported path: a file, a folder (created, renamed or removed whole), or something gone.
    fn diff_scope(&mut self, scope: &str, step: &mut Step) -> io::Result<()> {
        let on_disk: HashSet<String> = match self.walk(scope) {
            Ok(files) => files.into_iter().collect(),
            Err(e) => {
                // A folder that cannot be listed now is no folder emptied: nothing under it is removed this round.
                self.deferred.skip(scope, &e);
                return Ok(());
            }
        };
        self.deferred.clear(scope);
        let prefix = if scope.is_empty() {
            String::new()
        } else {
            format!("{scope}/")
        };
        let mut visited = 0;
        let gone: Vec<String> = self
            .ledger
            .keys()
            .filter(|rel| {
                visited += 1;
                (*rel == scope || rel.starts_with(&prefix)) && !on_disk.contains(*rel)
            })
            .cloned()
            .collect();
        self.work += visited;
        for rel in gone {
            let known = self.ledger.get(&rel);
            // A file waiting for its bytes is absent on purpose.
            if known.is_some_and(|k| k.pending) {
                continue;
            }
            let was = known.map(|k| k.hash.clone()).filter(|h| !h.is_empty());
            if !step.seen.contains(&rel) {
                let mut op = json!({ "kind": "remove", "file": rel });
                if let Some(h) = was {
                    op["was"] = Value::String(h);
                }
                step.ops.push(op);
            }
            step.seen.insert(rel.clone());
            self.deferred.clear(&rel);
            step.delta.insert(rel, None);
        }
        let mut present: Vec<String> = on_disk.into_iter().collect();
        present.sort();
        for rel in present {
            if !step.seen.insert(rel.clone()) {
                continue;
            }
            if let Some(op) = self.change_or_defer(&rel, step)? {
                step.ops.push(op);
            }
        }
        Ok(())
    }

    /// `change`, a file that cannot be read now left as the ledger knew it, so the next scan reads it again.
    fn change_or_defer(&mut self, rel: &str, step: &mut Step) -> io::Result<Option<Value>> {
        match self.change(rel, step) {
            Ok(op) => {
                self.deferred.clear(rel);
                Ok(op)
            }
            Err(e) => {
                self.deferred.skip(rel, &e);
                Ok(None)
            }
        }
    }

    /// The op for one file present on disk, the scan's delta updated; None when unchanged, or still being written.
    fn change(&mut self, rel: &str, step: &mut Step) -> io::Result<Option<Value>> {
        self.work += 1;
        let known = match step.delta.get(rel) {
            Some(k) => k.clone(),
            None => self.ledger.get(rel).cloned(),
        };
        // A file waiting for its bytes belongs to the fetch; what is on disk is the version it replaces.
        if known.as_ref().is_some_and(|k| k.pending) {
            return Ok(None);
        }
        let md = std::fs::metadata(self.path_of(rel)?)?;
        let (size, mtime) = (md.len(), mtime_ms(&md));
        let stamp = stamp_of(size, mtime);
        let now = self.now();
        // A same-size rewrite inside one timestamp tick keeps the stamp, so an unsettled file is re-read.
        let settled = now - mtime > SETTLE_MS;
        let has_ref = known.as_ref().is_some_and(|k| k.r#ref.is_some());
        if known.as_ref().is_some_and(|k| k.stamp == stamp) && (settled || has_ref) {
            return Ok(None);
        }
        let opaque = has_ref || self.opaque_by_name(rel, size);
        if opaque && now - mtime < self.o.binary_settle_ms {
            self.young.insert(rel.to_string());
            return Ok(None);
        }
        let probed = self.probe(rel, true, None)?;
        step.delta.insert(rel.to_string(), Some(probed.now.clone()));
        let kept = parse_blob_ref(probed.now.r#ref.as_deref());
        if known.as_ref().is_some_and(|k| k.hash == probed.now.hash) {
            // The copy taken to send it is not needed: nothing changed.
            if let (Some(true), Some(blob), Some(c)) = (probed.fresh, kept, self.cache.as_ref()) {
                c.evict(&blob.hash);
            }
            return Ok(None);
        }
        if let (Some(true), Some(blob)) = (probed.fresh, kept) {
            step.fresh.push(blob.hash);
        }
        let was = known.as_ref().map(|k| k.hash.as_str());
        if probed.now.r#ref.is_some() {
            let was = was.map_or(Value::Null, |h| Value::String(h.to_string()));
            return Ok(Some(
                json!({ "kind": "text", "file": rel, "text": probed.text, "was": was }),
            ));
        }
        let before = match known.as_ref().and_then(|k| k.text.clone()) {
            Some(t) => Some(t),
            None => self.confirmed_text(rel, known.as_ref())?,
        };
        Ok(file_op_for(
            &*self.o.k7x,
            rel,
            before.as_deref(),
            Some(&probed.text),
            Some(was),
        ))
    }
}
