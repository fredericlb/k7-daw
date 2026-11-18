//! live-sync's `TextBases` (textMerge.ts), native: the recent versions of each mergeable file a room keeps, the
//! bases a late whole text merges against. Plain bookkeeping, so it is Rust, proven by node's trace in
//! `fixtures/settle-vectors.json`. What a reader gets wrong: every size is in UTF-16 units, as `text.length` is,
//! and the file order is a JS `Map`'s — a removed file that comes back goes last (docs/parity.md, "The settle").

use indexmap::IndexMap;
use serde_json::{Map, Value};

use crate::number::text_hash;

/// How many versions of one file the room keeps as bases, and how many UTF-16 units in all.
const VERSIONS_KEEP: usize = 20;
const CHARS_KEEP: usize = 1 << 20;

const BLOB_PREFIX: &str = "\u{0}k7fs-blob ";

/// `isBlobRef`: a NUL, `k7fs-blob `, 64 lowercase hex digits, a space, decimal digits, one newline, nothing after.
pub fn is_blob_ref(text: &str) -> bool {
    let Some(rest) = text.strip_prefix(BLOB_PREFIX) else {
        return false;
    };
    let b = rest.as_bytes();
    if b.len() < 64 + 3 || b[64] != b' ' || b[b.len() - 1] != b'\n' {
        return false;
    }
    let hex = b[..64]
        .iter()
        .all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f'));
    let digits = &b[65..b.len() - 1];
    hex && !digits.is_empty() && digits.iter().all(u8::is_ascii_digit)
}

/// `mergeable`: a text that is neither a `.k7x` file's (any case) nor a blob ref.
pub fn mergeable(file: &str, text: Option<&str>) -> bool {
    let k7x = file.len() >= 4 && file.as_bytes()[file.len() - 4..].eq_ignore_ascii_case(b".k7x");
    matches!(text, Some(t) if !k7x && !is_blob_ref(t))
}

/// A text's length as JavaScript counts it.
fn units(text: &str) -> usize {
    text.encode_utf16().count()
}

/// The recent versions of each mergeable file, oldest first: only what the room wrote, so a replay rebuilds it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextBases {
    by_file: IndexMap<String, Vec<String>>,
}

impl TextBases {
    pub fn new() -> Self {
        Self::default()
    }

    /// `TextBases.from`: bases as `state()` saved them; a value that is not a list of strings is skipped.
    pub fn from_json(saved: &Map<String, Value>) -> Self {
        let mut by_file = IndexMap::new();
        for (f, v) in saved {
            let Some(list) = v.as_array() else { continue };
            let texts: Vec<String> = list
                .iter()
                .filter_map(|t| t.as_str().map(String::from))
                .collect();
            by_file.insert(f.clone(), texts);
        }
        TextBases { by_file }
    }

    /// `toJSON`: each file's versions, in the map's order.
    pub fn to_json(&self) -> Map<String, Value> {
        self.by_file
            .iter()
            .map(|(f, v)| (f.clone(), Value::from(v.clone())))
            .collect()
    }

    /// `record`: follow a write; a removed or unmergeable file keeps nothing, any other text joins its versions.
    pub fn record(&mut self, file: &str, text: Option<&str>) {
        let Some(text) = text.filter(|t| mergeable(file, Some(t)) && units(t) <= CHARS_KEEP) else {
            self.by_file.shift_remove(file);
            return;
        };
        let kept = self.by_file.entry(file.to_string()).or_default();
        if kept.last().map(String::as_str) == Some(text) {
            return;
        }
        kept.push(text.to_string());
        let mut size: usize = kept.iter().map(|t| units(t)).sum();
        let mut drop = 0;
        while kept.len() - drop > VERSIONS_KEEP || size > CHARS_KEEP {
            size -= units(&kept[drop]);
            drop += 1;
        }
        kept.drain(..drop);
    }

    /// `versions`: a copy of the versions kept for `file`, for `restore`.
    pub fn versions(&self, file: &str) -> Option<Vec<String>> {
        self.by_file.get(file).cloned()
    }

    /// `restore`: put back what `versions` returned.
    pub fn restore(&mut self, file: &str, kept: Option<Vec<String>>) {
        match kept {
            Some(k) => {
                self.by_file.insert(file.to_string(), k);
            }
            None => {
                self.by_file.shift_remove(file);
            }
        }
    }

    /// `find`: the newest kept version of `file` whose `textHash` is `hash`.
    pub fn find(&self, file: &str, hash: &str) -> Option<&str> {
        self.of(file)
            .iter()
            .rev()
            .find(|t| text_hash(t) == hash)
            .map(String::as_str)
    }

    /// The versions kept for `file`, oldest first: what a settle reads as `bases`.
    pub fn of(&self, file: &str) -> &[String] {
        self.by_file.get(file).map(Vec::as_slice).unwrap_or(&[])
    }
}
