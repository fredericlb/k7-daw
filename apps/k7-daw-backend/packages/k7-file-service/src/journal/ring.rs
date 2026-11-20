//! ring.rs — the journal's pure half, `ProjectHistory` without its disk: a ring of up to 1,000 entries with a cursor, a
//! fresh push dropping the redo tail, a join folding into the newest step, and the views the History panel and the
//! shortcuts read. What a reader gets wrong: it names the blobs to delete and keep, and deletes none; the caller does,
//! after the journal is persisted.

use serde_json::{Value, json};

use super::format::{BlobChange, Change, EngineRef, Entry, Journal, Kind};

pub const MAX_ENTRIES: usize = 1000;

/// A binary file a record changed; its bytes are in files, not here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewBlob {
    pub path: String,
    pub before: bool,
    pub after: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NewEntry {
    pub label: String,
    pub changes: Vec<Change>,
    pub blobs: Vec<NewBlob>,
}

/// `<id>-b<n>` for a pre-image, `<id>-a<n>` for an after-image.
pub fn blob_name(id: u64, before: bool, n: usize) -> String {
    format!("{id}-{}{n}", if before { 'b' } else { 'a' })
}

/// What a push decided for the files.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Pushed {
    pub id: u64,
    /// The new entry's blob names, `(before, after)` per blob, to store before the journal is persisted.
    pub stored: Vec<(Option<String>, Option<String>)>,
    /// Blob names whose copies here go once the journal is persisted: the redo tail's and the evicted's.
    pub dropped: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Joined {
    Pushed(Pushed),
    Folded,
    /// The gesture ended where it began: no step is left.
    Dropped,
    /// Nothing to record, nothing changed.
    Nothing,
}

fn names_of(entries: &[Entry]) -> Vec<String> {
    entries
        .iter()
        .flat_map(|e| e.blobs.iter())
        .flat_map(|b| [b.before.clone(), b.after.clone()])
        .flatten()
        .collect()
}

/// Record one change on top of the stack, dropping any redo tail it lands on; `None` when it holds nothing.
pub fn push(
    j: &mut Journal,
    e: NewEntry,
    ts_ms: u64,
    max: usize,
    engine: Option<EngineRef>,
) -> Option<Pushed> {
    if e.changes.is_empty() && e.blobs.is_empty() {
        return None;
    }
    let mut dropped = names_of(&j.entries[j.cursor..]);
    j.entries.truncate(j.cursor);
    let id = j.next_id;
    j.next_id += 1;
    let blobs: Vec<BlobChange> = e
        .blobs
        .iter()
        .enumerate()
        .map(|(n, b)| {
            Change::new(
                &b.path,
                b.before.then(|| blob_name(id, true, n)),
                b.after.then(|| blob_name(id, false, n)),
            )
        })
        .collect();
    let stored = blobs
        .iter()
        .map(|b| (b.before.clone(), b.after.clone()))
        .collect();
    let process = !blobs.is_empty();
    j.entries.push(Entry {
        id,
        ts: ts_ms.into(),
        kind: if process { Kind::Process } else { Kind::Text },
        label: e.label,
        changes: e.changes,
        blobs,
        kept_on: process.then_some(engine),
        order: vec![],
        rest: Default::default(),
    });
    j.cursor = j.entries.len();
    if j.entries.len() > max {
        let drop = j.entries.len() - max;
        dropped.extend(names_of(&j.entries[..drop]));
        j.entries.drain(..drop);
        j.cursor -= drop;
    }
    Some(Pushed {
        id,
        stored,
        dropped,
    })
}

/// Fold a change into the step on top, so one gesture stays one step; a fresh push when there is no such step.
pub fn join(
    j: &mut Journal,
    e: NewEntry,
    ts_ms: u64,
    max: usize,
    engine: Option<EngineRef>,
) -> Joined {
    let top_is_plain =
        j.cursor > 0 && j.cursor == j.entries.len() && j.entries[j.cursor - 1].blobs.is_empty();
    if !top_is_plain || !e.blobs.is_empty() {
        return push(j, e, ts_ms, max, engine).map_or(Joined::Nothing, Joined::Pushed);
    }
    if e.changes.is_empty() {
        return Joined::Nothing;
    }
    let top = &mut j.entries[j.cursor - 1];
    for c in e.changes {
        match top.changes.iter().position(|x| x.path == c.path) {
            Some(i) => {
                let before = top.changes[i].before.clone();
                top.changes[i] = Change { before, ..c };
            }
            None => top.changes.push(c),
        }
    }
    top.changes.retain(|c| c.before != c.after);
    top.ts = ts_ms.into();
    if top.changes.is_empty() {
        j.entries.pop();
        j.cursor = j.entries.len();
        return Joined::Dropped;
    }
    Joined::Folded
}

fn files_of(e: &Entry) -> Vec<&str> {
    let mut out: Vec<&str> = Vec::new();
    for p in e.changes.iter().chain(&e.blobs).map(|c| c.path.as_str()) {
        if !out.contains(&p) {
            out.push(p);
        }
    }
    out
}

/// `HistoryPeek`: what a step touches, never its contents.
pub fn peek(e: &Entry) -> Value {
    json!({ "kind": e.kind.as_str(), "label": e.label, "files": files_of(e) })
}

pub fn peek_undo(j: &Journal) -> Option<Value> {
    (j.cursor > 0).then(|| peek(&j.entries[j.cursor - 1]))
}

pub fn peek_redo(j: &Journal) -> Option<Value> {
    (j.cursor < j.entries.len()).then(|| peek(&j.entries[j.cursor]))
}

/// `HistoryState`: the shortcuts' and the toast's peek.
pub fn state(j: &Journal) -> Value {
    let mut m = serde_json::Map::new();
    m.insert("canUndo".into(), json!(j.cursor > 0));
    m.insert("canRedo".into(), json!(j.cursor < j.entries.len()));
    if let Some(v) = peek_undo(j) {
        m.insert("nextUndo".into(), v);
    }
    if let Some(v) = peek_redo(j) {
        m.insert("nextRedo".into(), v);
    }
    Value::Object(m)
}

/// `HistoryListEntry[]`, oldest first, the paths as recorded (workspace-relative), `undone` past the cursor.
pub fn list(j: &Journal) -> Vec<Value> {
    j.entries
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let paths: Vec<&str> = e.changes.iter().chain(&e.blobs).map(|c| c.path.as_str()).collect();
            json!({ "id": e.id, "ts": e.ts, "kind": e.kind.as_str(), "label": e.label, "paths": paths, "undone": i >= j.cursor })
        })
        .collect()
}
