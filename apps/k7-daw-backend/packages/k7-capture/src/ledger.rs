//! ledger.rs — what a capture's ledger records of one file, and the rules that decide how a file travels: a stamp,
//! binary by content, the text ceiling, which texts are kept (`liveLedger.ts`).
//! What a reader gets wrong: the stamp prints `mtimeMs` as JavaScript does, so an mtime with a fraction keeps it.

use k7_js::number::to_string;
use k7_sync::bases::is_k7x;

/// A stamp younger than this has not settled: a same-size rewrite inside one timestamp tick keeps it.
pub const SETTLE_MS: f64 = 2000.0;

/// What the ledger knows of one file: its stamp, the `textHash` of its text or ref, the text of a k7x file.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Known {
    pub stamp: String,
    pub hash: String,
    pub text: Option<String>,
    /// An opaque file's blob ref.
    pub r#ref: Option<String>,
    /// Known by its ref but not on disk yet: its bytes are being fetched, and its absence is no removal.
    pub pending: bool,
}

/// What the ledger says a file shows, enough to tell whether a given text is already on the disk without reading it.
#[derive(Debug, Clone, PartialEq)]
pub enum View {
    Absent,
    /// A k7x file: its exact text.
    Text(String),
    /// Any other text file: the `textHash` of its text, which is how the capture itself tells a change.
    Hash(String),
    /// An opaque file: its blob ref.
    Ref(String),
}

impl View {
    pub fn is(&self, text: Option<&str>) -> bool {
        match (self, text) {
            (View::Absent, None) => true,
            (View::Text(t), Some(x)) => t == x,
            (View::Hash(h), Some(x)) => k7_js::number::text_hash(x) == *h,
            (View::Ref(r), Some(x)) => r == x,
            _ => false,
        }
    }
}

/// A file whose text the ledger keeps, to diff its next change into path ops: a k7x document.
pub fn keeps_text(rel: &str) -> bool {
    is_k7x(rel)
}

/// A text file past this travels as an opaque file: a k7x one past 5 MiB, any other past 128 KiB (amp models).
pub fn text_ceiling(rel: &str) -> u64 {
    if keeps_text(rel) {
        5 * 1024 * 1024
    } else {
        128 * 1024
    }
}

/// Git's own test: a NUL byte in the first 8000 bytes makes a file binary, whatever its extension.
pub fn looks_binary(buf: &[u8]) -> bool {
    buf[..buf.len().min(8000)].contains(&0)
}

/// `${size}:${mtimeMs}`, the mtime printed as JavaScript prints a number.
pub fn stamp_of(size: u64, mtime_ms: f64) -> String {
    format!("{size}:{}", to_string(mtime_ms))
}

/// `statSync(..).mtimeMs`: seconds times a thousand plus the nanoseconds in milliseconds, as node adds them.
pub fn mtime_ms(md: &std::fs::Metadata) -> f64 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        md.mtime() as f64 * 1e3 + md.mtime_nsec() as f64 / 1e6
    }
    #[cfg(not(unix))]
    {
        let since = md
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok());
        since.map_or(0.0, |d| {
            d.as_secs() as f64 * 1e3 + f64::from(d.subsec_nanos()) / 1e6
        })
    }
}
