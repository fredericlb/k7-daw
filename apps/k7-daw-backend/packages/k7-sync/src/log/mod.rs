//! log — a room's whole log on disk (`docs/files/history.md`): `entries.jsonl` since the
//! last save, segments `log/<firstSeq>.jsonl` with `log/index.json`, full snapshots `snapshots/<seq>.json`, and
//! `checkpoints.json`. Every line and saved state carries the LOG_V that wrote it; a higher one is refused.
//! What a reader gets wrong: a save ROLLS `entries.jsonl` into a segment, it never just empties it.

mod segments;

use std::path::Path;

use serde_json::{Map, Value};

use crate::fsx;
use crate::json::{stringify, stringify_indented};
use crate::protocol::{LOG_V, is_known_op_kind, is_retired, js_string};

pub use segments::{entries_before, files_at, head_on_disk, roll_log};

pub const ENTRIES: &str = "entries.jsonl";
pub const SEGMENTS: &str = "log";
pub const SNAPSHOTS: &str = "snapshots";
const CHECKPOINTS: &str = "checkpoints.json";

/// Why a log could not be read or written.
#[derive(Debug)]
pub enum LogError {
    /// Written by a newer k7: refused, never guessed at.
    NeedsUpdate(String),
    /// The entries that would rebuild a point were never kept.
    Gap(String),
    Other(String),
}

impl LogError {
    pub fn message(&self) -> &str {
        match self {
            LogError::NeedsUpdate(m) | LogError::Gap(m) | LogError::Other(m) => m,
        }
    }
}

impl From<std::io::Error> for LogError {
    fn from(e: std::io::Error) -> Self {
        LogError::Other(e.to_string())
    }
}

impl From<k7_js::JsError> for LogError {
    fn from(e: k7_js::JsError) -> Self {
        if e.name == "NeedsUpdate" {
            LogError::NeedsUpdate(e.message)
        } else {
            LogError::Other(e.message)
        }
    }
}

/// One entry read back, and the files its skipped retired ops changed, which a replay can no longer rebuild.
#[derive(Debug, Clone)]
pub struct Parsed {
    pub entry: Value,
    pub lost: Vec<String>,
}

impl Parsed {
    pub fn seq(&self) -> f64 {
        crate::room::seq_of(&self.entry)
    }
}

/// A version stamp read from disk: absent is 1, the only version written before the stamp.
fn check_version(v: Option<&Value>, what: &str) -> Result<(), LogError> {
    let ver = match v {
        None | Some(Value::Null) => 1.0,
        Some(Value::Number(n)) => n.as_f64().unwrap_or(f64::NAN),
        Some(other) => {
            return Err(LogError::Other(format!(
                "{what}: bad log version {}",
                js_string(Some(other))
            )));
        }
    };
    if ver.fract() != 0.0 || ver < 1.0 {
        return Err(LogError::Other(format!(
            "{what}: bad log version {}",
            k7_js::number::to_string(ver)
        )));
    }
    if ver > LOG_V {
        return Err(LogError::NeedsUpdate(format!(
            "{what} written by log version {}, this k7 reads up to {}: update k7",
            k7_js::number::to_string(ver),
            k7_js::number::to_string(LOG_V)
        )));
    }
    Ok(())
}

/// `{ v: LOG_V, ...object }` written.
fn stamped(object: &Map<String, Value>) -> String {
    let mut m = Map::with_capacity(object.len() + 1);
    m.insert("v".into(), Value::from(LOG_V));
    for (k, v) in object {
        if k != "v" {
            m.insert(k.clone(), v.clone());
        }
    }
    stringify(&Value::Object(m))
}

/// One persisted entry, stamped with the log version that wrote it.
pub fn entry_line(entry: &Value) -> String {
    let empty = Map::new();
    format!("{}\n", stamped(entry.as_object().unwrap_or(&empty)))
}

/// A saved room state as written, stamped like the entries.
pub fn state_text(state: &Map<String, Value>) -> String {
    stamped(state)
}

fn retired_conflict(c: &Value) -> bool {
    is_retired(c.get("op").and_then(|o| o.get("kind")))
}

/// One line of a log, or None for one a crash cut short or that is no entry; a newer version is NeedsUpdate.
pub fn parse_entry_line(line: &str) -> Result<Option<Parsed>, LogError> {
    let Ok(Value::Object(mut entry)) = serde_json::from_str::<Value>(line) else {
        return Ok(None);
    };
    let v = entry.shift_remove("v");
    check_version(
        v.as_ref(),
        &format!("entry {}", js_string(entry.get("seq"))),
    )?;
    let shaped = entry.get("seq").is_some_and(Value::is_number)
        && entry
            .get("batch")
            .and_then(|b| b.get("ops"))
            .is_some_and(Value::is_array)
        && entry.get("batch").is_some_and(Value::is_object)
        && entry.get("parked").is_some_and(Value::is_array);
    if !shaped {
        return Ok(None);
    }
    let seq = js_string(entry.get("seq"));
    let unknown = || LogError::NeedsUpdate(format!("entry {seq}: op kind unknown here: update k7"));
    let Some(Value::Array(ops)) = entry.get_mut("batch").and_then(|b| b.get_mut("ops")) else {
        unreachable!()
    };
    if !ops.iter().all(Value::is_object) {
        return Err(unknown());
    }
    let lost = ops
        .iter()
        .filter(|op| is_retired(op.get("kind")))
        .map(|op| js_string(op.get("file")))
        .collect();
    ops.retain(|op| !is_retired(op.get("kind")));
    if !ops.iter().all(|op| is_known_op_kind(op.get("kind"))) {
        return Err(unknown());
    }
    if let Some(Value::Array(parked)) = entry.get_mut("parked") {
        parked.retain(|c| !retired_conflict(c));
    }
    Ok(Some(Parsed {
        entry: Value::Object(entry),
        lost,
    }))
}

/// A saved room state read back, its stamp checked and dropped.
pub fn parse_state(text: &str) -> Result<Map<String, Value>, LogError> {
    let raw: Value = serde_json::from_str(text).map_err(|e| LogError::Other(e.to_string()))?;
    let Value::Object(mut state) = raw else {
        return Err(LogError::Other("room state: not an object".into()));
    };
    let v = state.shift_remove("v");
    check_version(v.as_ref(), "room state")?;
    Ok(state)
}

/// A saved room state with its retired ops skipped, in the log and among its conflicts (`withoutRetiredOps`).
pub fn without_retired_ops(state: &mut Map<String, Value>) -> Result<(), LogError> {
    if let Some(Value::Array(log)) = state.get_mut("log") {
        for e in log.iter_mut() {
            let Some(e) = e.as_object_mut() else { continue };
            let Some(Value::Array(ops)) = e.get_mut("batch").and_then(|b| b.get_mut("ops")) else {
                return Err(LogError::Other("e.batch.ops is not an array".into()));
            };
            ops.retain(|op| !is_retired(op.get("kind")));
            let parked = match e.get("parked") {
                Some(Value::Array(p)) => {
                    p.iter().filter(|c| !retired_conflict(c)).cloned().collect()
                }
                _ => Vec::new(),
            };
            e.insert("parked".into(), Value::Array(parked));
        }
    }
    if let Some(Value::Array(c)) = state.get_mut("conflicts") {
        c.retain(|c| !retired_conflict(c));
    }
    Ok(())
}

/// Every entry a log file holds; a line a crash cut short was never relayed, and is skipped.
pub fn read_lines(file: &Path) -> Result<Vec<Parsed>, LogError> {
    let text = match fsx::read_text(file) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    let mut out = Vec::new();
    for line in text.split('\n').filter(|l| !l.is_empty()) {
        if let Some(p) = parse_entry_line(line)? {
            out.push(p);
        }
    }
    Ok(out)
}

/// `String(n)` as a file name.
pub fn seq_name(n: f64) -> String {
    k7_js::number::to_string(n)
}

/// Give a room saved before it had segments its log as one, and its saved files as a snapshot (`adoptLegacy`).
pub fn adopt_legacy(dir: &Path, state: Option<&Map<String, Value>>) -> Result<(), LogError> {
    if dir.join(SEGMENTS).exists() {
        return Ok(());
    }
    std::fs::create_dir_all(dir.join(SEGMENTS))?;
    std::fs::create_dir_all(dir.join(SNAPSHOTS))?;
    let Some(state) = state else { return Ok(()) };
    let head = state
        .get("head")
        .and_then(Value::as_f64)
        .unwrap_or(f64::NAN);
    if head == 0.0 {
        return Ok(());
    }
    let log = state
        .get("log")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let kept: Vec<&Value> = log
        .iter()
        .filter(|e| crate::room::seq_of(e) <= head)
        .collect();
    if let Some(first) = kept.first() {
        let text: String = kept.iter().map(|e| entry_line(e)).collect();
        fsx::write_atomic(
            &dir.join(SEGMENTS)
                .join(format!("{}.jsonl", seq_name(crate::room::seq_of(first)))),
            &text,
        )?;
    }
    let files = state
        .get("files")
        .map(stringify)
        .unwrap_or_else(|| "undefined".into());
    write_snapshot(dir, head, &files)
}

/// `snapshots/<seq>.json`: every file's text at `seq`, already written as JSON.
pub fn write_snapshot(dir: &Path, seq: f64, files_json: &str) -> Result<(), LogError> {
    std::fs::create_dir_all(dir.join(SNAPSHOTS))?;
    fsx::write_atomic(
        &dir.join(SNAPSHOTS).join(format!("{}.json", seq_name(seq))),
        files_json,
    )?;
    Ok(())
}

/// The room's named points, as `checkpoints.json` holds them.
pub fn read_checkpoints(dir: &Path) -> Result<Vec<Value>, LogError> {
    let file = dir.join(CHECKPOINTS);
    if !file.exists() {
        return Ok(Vec::new());
    }
    match serde_json::from_str::<Value>(&fsx::read_text(&file)?) {
        Ok(Value::Array(a)) => Ok(a),
        Ok(_) => Err(LogError::Other("checkpoints.json is not a list".into())),
        Err(e) => Err(LogError::Other(e.to_string())),
    }
}

/// `{ seq, name, ts, by }`.
pub fn checkpoint(seq: f64, name: &str, ts: f64, by: &str) -> Value {
    serde_json::json!({ "seq": seq, "name": name, "ts": ts, "by": by })
}

/// Add a named point, the list kept in seq order and written as `JSON.stringify(all, null, 1)`.
pub fn add_checkpoint(dir: &Path, cp: Value) -> Result<Value, LogError> {
    let mut all = read_checkpoints(dir)?;
    all.push(cp.clone());
    let seq = |c: &Value| c.get("seq").and_then(Value::as_f64).unwrap_or(f64::NAN);
    all.sort_by(|a, b| {
        seq(a)
            .partial_cmp(&seq(b))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    fsx::write_atomic(
        &dir.join(CHECKPOINTS),
        &stringify_indented(&Value::Array(all), 1),
    )?;
    Ok(cp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_lines() {
        let e = serde_json::json!({ "seq": 3, "batch": { "id": "b", "ops": [{ "kind": "text", "file": "a", "text": "x" }] }, "parked": [] });
        let line = entry_line(&e);
        assert_eq!(
            line,
            "{\"v\":1,\"seq\":3,\"batch\":{\"id\":\"b\",\"ops\":[{\"kind\":\"text\",\"file\":\"a\",\"text\":\"x\"}]},\"parked\":[]}\n"
        );
        assert_eq!(parse_entry_line(line.trim_end()).unwrap().unwrap().entry, e);
        assert!(
            parse_entry_line("{\"v\":1,\"seq\":3,\"ba")
                .unwrap()
                .is_none()
        );
        assert!(parse_entry_line("{\"seq\":3}").unwrap().is_none());
        let newer = parse_entry_line("{\"v\":2,\"seq\":3}").unwrap_err();
        assert_eq!(
            newer.message(),
            "entry 3 written by log version 2, this k7 reads up to 1: update k7"
        );
        assert!(matches!(newer, LogError::NeedsUpdate(_)));
        assert_eq!(
            parse_entry_line("{\"v\":0.5,\"seq\":3}")
                .unwrap_err()
                .message(),
            "entry 3: bad log version 0.5"
        );
        let unknown =
            "{\"seq\":4,\"batch\":{\"ops\":[{\"kind\":\"zip\",\"file\":\"a\"}]},\"parked\":[]}";
        assert!(matches!(
            parse_entry_line(unknown).unwrap_err(),
            LogError::NeedsUpdate(_)
        ));
        let retired = "{\"seq\":4,\"batch\":{\"ops\":[{\"kind\":\"yaml\",\"file\":\"a.yaml\"}]},\"parked\":[{\"op\":{\"kind\":\"yaml\"}},5]}";
        let p = parse_entry_line(retired).unwrap().unwrap();
        assert_eq!(p.lost, ["a.yaml"]);
        assert_eq!(
            stringify(&p.entry),
            "{\"seq\":4,\"batch\":{\"ops\":[]},\"parked\":[5]}"
        );
    }
}
