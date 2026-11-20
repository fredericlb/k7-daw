//! bench/source.rs — what the bench copied, recorded BESIDE the rig in `bench-source.json`, and the fingerprint that says it was edited.
//! What a reader gets wrong: the stamp hashes the graph and every node file in sorted order, names included, exactly as the TypeScript does.

use indexmap::IndexMap;
use serde_json::{Value, json};
use sha1::{Digest, Sha1};
use song_edit::Fs;
use song_edit::workspace_file::{ws_list, ws_read, ws_write};

use crate::paths::join;

/// What the bench holds a copy of: the chain (none for the bare rig), each node's source step id, and the rig as copied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BenchSource {
    pub ref_: Option<String>,
    pub ids: IndexMap<String, String>,
    pub stamp: String,
}

pub(super) fn file(workspace: &str) -> String {
    join(&[workspace, "bench-source.json"])
}

/// A fingerprint of the rig's graph and node files: a bench whose fingerprint moved was edited.
///
/// # Errors
/// A graph that cannot be read.
pub fn bench_stamp(fs: &dyn Fs, dir: &str) -> Result<String, String> {
    let mut h = Sha1::new();
    h.update(
        ws_read(fs, &join(&[dir, "graph.k7x"]))
            .map_err(|e| e.to_string())?
            .as_bytes(),
    );
    let nodes = join(&[dir, "nodes"]);
    if fs.exists(&nodes) {
        for f in ws_list(fs, &nodes).map_err(|e| e.to_string())? {
            h.update(f.as_bytes());
            h.update(
                ws_read(fs, &join(&[&nodes, &f]))
                    .map_err(|e| e.to_string())?
                    .as_bytes(),
            );
        }
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// What the bench copied, or none when no bench was made since this was recorded.
#[must_use]
pub fn read_source(fs: &dyn Fs, workspace: &str) -> Option<BenchSource> {
    let text = ws_read(fs, &file(workspace)).ok()?;
    let v: Value = serde_json::from_str(&text).ok()?;
    // A source with no `ref` names no bench at all, which the TypeScript reads as not the bare rig.
    v.get("ref")?;
    let ids = match v.get("ids") {
        Some(Value::Object(m)) => m
            .iter()
            .map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_owned()))
            .collect(),
        _ => IndexMap::new(),
    };
    Some(BenchSource {
        ref_: v.get("ref").and_then(Value::as_str).map(str::to_owned),
        ids,
        stamp: v
            .get("stamp")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
    })
}

/// Record what the bench now holds, stamped as it stands.
///
/// # Errors
/// A rig that cannot be fingerprinted, or the write's.
pub fn write_source(
    fs: &mut dyn Fs,
    workspace: &str,
    dir: &str,
    ref_: Option<&str>,
    ids: &IndexMap<String, String>,
) -> Result<(), String> {
    let stamp = bench_stamp(fs, dir)?;
    let ids: serde_json::Map<String, Value> =
        ids.iter().map(|(k, v)| (k.clone(), json!(v))).collect();
    let text = serde_json::to_string_pretty(&json!({ "ref": ref_, "ids": ids, "stamp": stamp }))
        .map_err(|e| e.to_string())?;
    ws_write(fs, &file(workspace), text.as_bytes()).map_err(|e| e.to_string())
}
