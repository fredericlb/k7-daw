//! index.rs — the blob index: the binaries' and the text files' hashes by path and stamp, kept across restarts in one
//! JSON file written through a temp and a rename, so a start does not read every file again (`BlobIndex`, liveLedger.ts).
//! What a reader gets wrong: only a SETTLED stamp is kept, and an index that does not parse is empty, not an error.

use std::collections::BTreeMap;
use std::io;
use std::path::PathBuf;

use indexmap::IndexMap;
use k7_replica::blob::{BlobId, parse_blob_ref};
use k7_sync::json::stringify;
use serde_json::{Map, Value, json};

use crate::ledger::{Known, SETTLE_MS};
use crate::num::js_number;

#[derive(Debug, Clone)]
struct Indexed {
    stamp: String,
    hash: String,
    size: f64,
}

pub struct BlobIndex {
    file: PathBuf,
    entries: BTreeMap<String, Indexed>,
    texts: BTreeMap<String, (String, String)>,
    written: String,
}

/// The mtime part of a stamp as `Number(stamp.split(":")[1])`: NaN when it is not one.
fn mtime_of(stamp: &str) -> f64 {
    stamp.split(':').nth(1).map_or(f64::NAN, js_number)
}

impl BlobIndex {
    /// The index at `file`; none yet, or a torn one, is empty and costs a re-hash (2136).
    pub fn open(file: &str) -> Self {
        let mut me = BlobIndex {
            file: PathBuf::from(file),
            entries: BTreeMap::new(),
            texts: BTreeMap::new(),
            written: String::new(),
        };
        let Ok(bytes) = std::fs::read(file) else {
            return me;
        };
        let text = String::from_utf8_lossy(&bytes).into_owned();
        me.written = text.clone();
        let Ok(Value::Object(saved)) = serde_json::from_str::<Value>(&text) else {
            return me;
        };
        if let Some(Value::Object(files)) = saved.get("files") {
            for (rel, v) in files {
                let (stamp, hash, size) = (
                    v.get("stamp").and_then(Value::as_str),
                    v.get("hash").and_then(Value::as_str),
                    v.get("size").and_then(Value::as_f64),
                );
                if let (Some(stamp), Some(hash), Some(size)) = (stamp, hash, size) {
                    me.entries.insert(
                        rel.clone(),
                        Indexed {
                            stamp: stamp.into(),
                            hash: hash.into(),
                            size,
                        },
                    );
                }
            }
        }
        if let Some(Value::Object(texts)) = saved.get("texts") {
            for (rel, v) in texts {
                if let (Some(stamp), Some(hash)) = (
                    v.get("stamp").and_then(Value::as_str),
                    v.get("hash").and_then(Value::as_str),
                ) {
                    me.texts.insert(rel.clone(), (stamp.into(), hash.into()));
                }
            }
        }
        me
    }

    /// The blob `rel` held when it last had this stamp.
    pub fn get(&self, rel: &str, stamp: &str) -> Option<BlobId> {
        let e = self.entries.get(rel)?;
        (e.stamp == stamp).then(|| BlobId {
            hash: e.hash.clone(),
            size: e.size,
        })
    }

    /// The `textHash` of the text `rel` held when it last had this stamp.
    pub fn text_hash(&self, rel: &str, stamp: &str) -> Option<&str> {
        let (s, h) = self.texts.get(rel)?;
        (s == stamp).then_some(h.as_str())
    }

    /// Rewrite the index from the ledger's settled files, only when something moved; a failed write is the caller's to ignore.
    pub fn save(&mut self, ledger: &IndexMap<String, Known>, now_ms: f64) -> io::Result<()> {
        let (mut files, mut texts) = (Map::new(), Map::new());
        let (mut new_entries, mut new_texts) = (BTreeMap::new(), BTreeMap::new());
        for (rel, k) in ledger {
            if k.pending || k.stamp.is_empty() || now_ms - mtime_of(&k.stamp) <= SETTLE_MS {
                continue;
            }
            let r = k.r#ref.as_deref().filter(|r| !r.is_empty());
            if let Some(blob) = parse_blob_ref(k.r#ref.as_deref()) {
                files.insert(
                    rel.clone(),
                    json!({ "stamp": k.stamp, "hash": blob.hash, "size": blob.size }),
                );
                new_entries.insert(
                    rel.clone(),
                    Indexed {
                        stamp: k.stamp.clone(),
                        hash: blob.hash,
                        size: blob.size,
                    },
                );
            } else if r.is_none() {
                texts.insert(rel.clone(), json!({ "stamp": k.stamp, "hash": k.hash }));
                new_texts.insert(rel.clone(), (k.stamp.clone(), k.hash.clone()));
            }
        }
        let text = stringify(&json!({ "files": files, "texts": texts }));
        if text == self.written {
            return Ok(());
        }
        if let Some(dir) = self.file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = PathBuf::from(format!("{}.tmp", self.file.display()));
        std::fs::write(&tmp, &text)?;
        std::fs::rename(&tmp, &self.file)?;
        self.written = text;
        self.entries = new_entries;
        self.texts = new_texts;
        Ok(())
    }
}
