//! segments.rs — the log's segments and snapshots read and written (`rollLog`, the segment index, a page of
//! entries, the files at a point), byte for byte as node wrote them.
//! What a reader gets wrong: `log/index.json` is a cache; one whose version or byte counts moved is rebuilt.

use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use serde_json::{Map, Value};

use super::{ENTRIES, LogError, Parsed, SEGMENTS, SNAPSHOTS, read_lines, seq_name, write_snapshot};
use crate::fsx;
use crate::json::{js_keys, stringify};
use crate::protocol::LOG_V;
use crate::settle::Js;

const SEGMENT_INDEX: &str = "index.json";

/// `Number(name)` for a file name's stem, as the log's folders are listed.
fn number_of(stem: &str) -> f64 {
    let t = stem.trim();
    if t.is_empty() {
        return 0.0;
    }
    let plain = t
        .bytes()
        .all(|b| b.is_ascii_digit() || b"+-.eE".contains(&b));
    if !plain {
        return f64::NAN;
    }
    t.parse().unwrap_or(f64::NAN)
}

/// The seqs a folder's files are named by, ascending.
fn seqs_in(dir: &Path, ext: &str) -> Vec<f64> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<f64> = read
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter_map(|f| f.strip_suffix(ext).map(number_of))
        .filter(|n| n.fract() == 0.0 && *n >= 0.0)
        .collect();
    out.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    out
}

fn segment_seqs(dir: &Path) -> Vec<f64> {
    seqs_in(&dir.join(SEGMENTS), ".jsonl")
}

fn snapshot_seqs(dir: &Path) -> Vec<f64> {
    seqs_in(&dir.join(SNAPSHOTS), ".json")
}

/// One segment as the index knows it, and the object the index keeps for it.
struct SegmentInfo {
    start: Value,
    first: f64,
    last: f64,
    raw: Value,
}

fn segment_file(dir: &Path, start: &Value) -> PathBuf {
    dir.join(SEGMENTS)
        .join(format!("{}.jsonl", crate::protocol::js_string(Some(start))))
}

/// The segments, oldest first; one the index lacks or whose length moved is read, and the index rewritten.
fn segment_index(dir: &Path) -> Result<Vec<SegmentInfo>, LogError> {
    let log_dir = dir.join(SEGMENTS);
    let mut saved = Map::new();
    if let Ok(text) = fsx::read_text(&log_dir.join(SEGMENT_INDEX))
        && let Ok(Value::Object(raw)) = serde_json::from_str::<Value>(&text)
        && raw.get("v").and_then(Value::as_f64) == Some(LOG_V)
        && let Some(Value::Object(s)) = raw.get("segments")
    {
        saved = s.clone();
    }
    let mut changed = false;
    let mut out = Vec::new();
    for start in segment_seqs(dir) {
        let file = log_dir.join(format!("{}.jsonl", seq_name(start)));
        let bytes = std::fs::metadata(&file)?.len() as f64;
        let known = saved
            .get(&seq_name(start))
            .filter(|i| i.get("bytes").and_then(Value::as_f64) == Some(bytes));
        let info = match known {
            Some(raw) => SegmentInfo {
                start: raw.get("start").cloned().unwrap_or(Value::Null),
                first: raw.get("first").and_then(Value::as_f64).unwrap_or(f64::NAN),
                last: raw.get("last").and_then(Value::as_f64).unwrap_or(f64::NAN),
                raw: raw.clone(),
            },
            None => {
                let (mut first, mut last, mut count) = (start, start - 1.0, 0.0);
                for e in read_lines(&file)? {
                    let seq = e.seq();
                    if count == 0.0 || seq < first {
                        first = seq;
                    }
                    count += 1.0;
                    last = last.max(seq);
                }
                changed = true;
                let raw = serde_json::json!({ "start": start, "first": first, "last": last, "count": count, "bytes": bytes });
                SegmentInfo {
                    start: Value::from(start),
                    first,
                    last,
                    raw,
                }
            }
        };
        out.push(info);
    }
    if changed || saved.len() != out.len() {
        let mut segments = Map::new();
        for i in &out {
            segments.insert(crate::protocol::js_string(Some(&i.start)), i.raw.clone());
        }
        let index = serde_json::json!({ "v": LOG_V, "segments": Value::Object(segments) });
        fsx::write_atomic(&log_dir.join(SEGMENT_INDEX), &stringify(&index))?;
    }
    Ok(out)
}

/// After the room's state is saved: move `entries.jsonl` into a segment, and snapshot the files when `every` entries passed.
pub fn roll_log(
    dir: &Path,
    head: f64,
    files_json: &dyn Fn() -> String,
    every: f64,
) -> Result<(), LogError> {
    let entries = dir.join(ENTRIES);
    if let Some(first) = read_lines(&entries)?.first() {
        std::fs::create_dir_all(dir.join(SEGMENTS))?;
        let segment = dir
            .join(SEGMENTS)
            .join(format!("{}.jsonl", seq_name(first.seq())));
        // A name already taken can only hold the same entries' start: appending keeps both, and readers skip a seq seen twice.
        if segment.exists() {
            fsx::append(&segment, &fsx::read_text(&entries)?)?;
        } else {
            fsx::rename_retrying(&entries, &segment)?;
        }
    }
    std::fs::write(&entries, "")?;
    segment_index(dir)?;
    let last = snapshot_seqs(dir).last().copied().unwrap_or(0.0);
    if head - last >= every {
        write_snapshot(dir, head, &files_json())?;
    }
    Ok(())
}

/// Entries with `after < seq <= upto`, oldest first, read only from the files that can hold them.
fn entries_between(dir: &Path, after: f64, upto: f64) -> Result<Vec<Parsed>, LogError> {
    let segs = segment_index(dir)?;
    let mut files: Vec<PathBuf> = segs
        .iter()
        .filter(|s| s.last > after && s.first <= upto)
        .map(|s| segment_file(dir, &s.start))
        .collect();
    if upto > segs.last().map_or(0.0, |s| s.last) {
        files.push(dir.join(ENTRIES));
    }
    let mut out = Vec::new();
    let mut seen = after;
    for f in files {
        for e in read_lines(&f)? {
            let seq = e.seq();
            if seq > seen && seq <= upto {
                seen = seq;
                out.push(e);
            }
        }
    }
    Ok(out)
}

/// Up to `limit` entries before `before`, newest first, reading files from the last back until the page is full.
pub fn entries_before(dir: &Path, before: f64, limit: usize) -> Result<Vec<Value>, LogError> {
    let segs = segment_index(dir)?;
    let mut files: Vec<PathBuf> = segs
        .iter()
        .filter(|s| s.first < before)
        .rev()
        .map(|s| segment_file(dir, &s.start))
        .collect();
    if before > segs.last().map_or(0.0, |s| s.last) + 1.0 {
        files.insert(0, dir.join(ENTRIES));
    }
    let mut by_seq: IndexMap<u64, (f64, Value)> = IndexMap::new();
    for f in files {
        for e in read_lines(&f)? {
            let seq = e.seq();
            if seq < before {
                by_seq.insert(seq.to_bits(), (seq, e.entry));
            }
        }
        if by_seq.len() >= limit {
            break;
        }
    }
    let mut all: Vec<(f64, Value)> = by_seq.into_values().collect();
    all.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    Ok(all.into_iter().take(limit).map(|(_, e)| e).collect())
}

/// The last seq the disk holds, without reading the room's saved state.
pub fn head_on_disk(dir: &Path) -> Result<f64, LogError> {
    let tail = read_lines(&dir.join(ENTRIES))?
        .last()
        .map_or(0.0, Parsed::seq);
    let seg = segment_index(dir)?.last().map_or(0.0, |s| s.last);
    let snap = snapshot_seqs(dir).last().copied().unwrap_or(0.0);
    Ok(tail.max(seg).max(snap))
}

/// `/\.ya?ml$/i`: a point holding one is from before the workspace was k7x.
fn yaml_name(f: &str) -> bool {
    let l = f.to_ascii_lowercase();
    l.ends_with(".yaml") || l.ends_with(".yml")
}

/// Every file's text at `seq`: the nearest snapshot at or before it and the entries after it replayed; a Gap when that would be a guess.
pub fn files_at(js: &dyn Js, dir: &Path, seq: f64) -> Result<IndexMap<String, String>, LogError> {
    let snap = snapshot_seqs(dir).into_iter().rfind(|s| *s <= seq);
    let mut files: IndexMap<String, String> = IndexMap::new();
    if let Some(s) = snap {
        let text = fsx::read_text(&dir.join(SNAPSHOTS).join(format!("{}.json", seq_name(s))))?;
        let raw: Value = serde_json::from_str(&text).map_err(|e| LogError::Other(e.to_string()))?;
        if let Value::Object(m) = &raw {
            for f in js_keys(m) {
                if let Some(t) = m[f].as_str() {
                    files.insert(f.clone(), t.to_string());
                }
            }
        }
    }
    // With no snapshot at or before it, the empty room at 0 is the base: only valid when the log reaches back to 1.
    let base = snap.unwrap_or(0.0);
    let entries = entries_between(dir, base, seq)?;
    for (i, e) in entries.iter().enumerate() {
        let want = base + 1.0 + i as f64;
        if e.seq() != want {
            return Err(LogError::Gap(format!("seq {} is not kept", seq_name(want))));
        }
    }
    if base + entries.len() as f64 != seq {
        return Err(LogError::Gap(format!("seq {} is not kept", seq_name(seq))));
    }
    // A file a skipped op changed is a guess until a whole text or a removal replaces it.
    let mut guessed: IndexMap<String, ()> = IndexMap::new();
    for e in &entries {
        let ops = e
            .entry
            .pointer("/batch/ops")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for op in &ops {
            let file = crate::protocol::js_string(op.get("file"));
            match crate::settle::apply(js, files.get(&file).map(String::as_str), op)? {
                None => {
                    files.shift_remove(&file);
                }
                Some(t) => {
                    files.insert(file.clone(), t);
                }
            }
            if op.get("kind").and_then(Value::as_str) != Some("k7x") {
                guessed.shift_remove(&file);
            }
        }
        for f in &e.lost {
            guessed.insert(f.clone(), ());
        }
    }
    if !guessed.is_empty() {
        let names: Vec<&str> = guessed.keys().map(String::as_str).collect();
        return Err(LogError::Gap(format!(
            "seq {}: {} changed by an edit this k7 no longer replays",
            seq_name(seq),
            names.join(", ")
        )));
    }
    let yaml: Vec<&str> = files
        .keys()
        .map(String::as_str)
        .filter(|f| yaml_name(f))
        .collect();
    if !yaml.is_empty() {
        return Err(LogError::Gap(format!(
            "seq {} is from before this workspace was k7x ({})",
            seq_name(seq),
            yaml.join(", ")
        )));
    }
    Ok(files)
}
