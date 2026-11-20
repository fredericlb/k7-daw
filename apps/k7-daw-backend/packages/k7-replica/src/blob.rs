//! blob.rs — a binary file as live sync carries it: a ref in place of its text, naming its bytes by sha256
//! (`@studio/live-sync`'s `blob.ts`); the parse is the relay's own, `k7_sync::bases::parse_blob_ref`.
//! What a reader gets wrong: the ref IS the file's text on the wire, so its bytes are hashed and compared.

use indexmap::IndexMap;
use k7_sync::bases::{BLOB_PREFIX, parse_blob_ref as parse};
use serde_json::Value;

/// A blob's identity: the sha256 of its bytes as lowercase hex, and their length (a JavaScript number).
#[derive(Debug, Clone, PartialEq)]
pub struct BlobId {
    pub hash: String,
    pub size: f64,
}

/// True for a sha256 as a blob store names it: 64 lowercase hex digits.
pub fn is_blob_hash(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The text standing for a binary file's bytes (1114).
pub fn blob_ref(hash: &str, size: u64) -> Result<String, String> {
    if !is_blob_hash(hash) {
        return Err(format!("not a sha256: {hash}"));
    }
    Ok(format!("{BLOB_PREFIX}{hash} {size}\n"))
}

/// The blob a text names, or None for an ordinary text and for an absent one.
pub fn parse_blob_ref(text: Option<&str>) -> Option<BlobId> {
    parse(text?).map(|(hash, size)| BlobId {
        hash: hash.to_string(),
        size,
    })
}

/// Every blob the ops write, each once and in first-seen order, the last ref of a hash standing (1114).
pub fn blobs_of(ops: &[Value]) -> Vec<BlobId> {
    let mut out: IndexMap<String, BlobId> = IndexMap::new();
    for op in ops {
        let text = (op.get("kind").and_then(Value::as_str) == Some("text"))
            .then(|| op.get("text").and_then(Value::as_str));
        if let Some(id) = parse_blob_ref(text.flatten()) {
            out.insert(id.hash.clone(), id);
        }
    }
    out.into_values().collect()
}
