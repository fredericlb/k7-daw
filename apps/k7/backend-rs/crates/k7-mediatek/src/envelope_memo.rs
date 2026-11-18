//! envelope_memo.rs — an envelope's answer held per file and version
//! (`packages/audio-analysis/docs/analysis.md`): the version is the file's size and time beside its sidecar's, the validator a weak hash of it.
//! What a reader gets wrong: a re-analysis rewrites only the sidecar, and that alone is a new version.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use base64::Engine;
use sha1::{Digest, Sha1};

use crate::ecma::mtime_ms;
use crate::sidecar::cache_path;

/// `ENVELOPE_MEMO_MAX`: answers held at once, the least recently used dropped first.
const MAX: usize = 16;

/// Bumped when the answer's shape changes, so a browser's copy from an older build never matches.
const SHAPE: &str = "e2";

/// One held answer.
#[derive(Clone, Debug)]
pub struct Held {
    pub version: String,
    pub body: String,
    pub etag: String,
}

/// `envelopeVersion`: the file's size and time, then its sidecar's or `none`; the error is the file's own.
pub fn version(source: &Path) -> std::io::Result<String> {
    let file = std::fs::metadata(source)?;
    let side = std::fs::metadata(cache_path(source)).ok().map_or_else(
        || "none".to_string(),
        |s| format!("{}-{}", s.len(), k7_js::number::to_string(mtime_ms(&s))),
    );
    Ok(format!(
        "{}-{}/{side}",
        file.len(),
        k7_js::number::to_string(mtime_ms(&file))
    ))
}

/// `envelopeEtag`: the validator a version is served under.
pub fn etag(version: &str) -> String {
    let h = Sha1::digest(format!("{SHAPE}:{version}").as_bytes());
    let b = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(h);
    format!("W/\"{}\"", &b[..22])
}

/// The answers held, least recently used first.
#[derive(Default)]
pub struct EnvelopeMemo {
    held: Mutex<Vec<(PathBuf, Held)>>,
}

impl EnvelopeMemo {
    /// The held answer while `version` is still the file's, touched as the most recent.
    pub fn get(&self, source: &Path, version: &str) -> Option<Held> {
        let mut held = self.held.lock().unwrap_or_else(|e| e.into_inner());
        let i = held
            .iter()
            .position(|(p, h)| p == source && h.version == version)?;
        let hit = held.remove(i);
        let out = hit.1.clone();
        held.push(hit);
        Some(out)
    }

    /// `body` held under the version read now, since the analysis that made it may have written the sidecar.
    pub fn put(&self, source: &Path, body: String) -> std::io::Result<Held> {
        let version = version(source)?;
        let next = Held {
            etag: etag(&version),
            version,
            body,
        };
        let mut held = self.held.lock().unwrap_or_else(|e| e.into_inner());
        held.retain(|(p, _)| p != source);
        held.push((source.to_path_buf(), next.clone()));
        while held.len() > MAX {
            held.remove(0);
        }
        Ok(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spells_the_validator_as_node() {
        // sha1("e2:10-1000/none") in base64url, cut at 22.
        let t = etag("10-1000/none");
        assert!(
            t.starts_with("W/\"") && t.ends_with('"') && t.len() == 26,
            "{t}"
        );
    }

    #[test]
    fn keeps_sixteen_and_drops_the_oldest() {
        let dir = std::env::temp_dir().join(format!("k7-memo-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let memo = EnvelopeMemo::default();
        let files: Vec<PathBuf> = (0..17).map(|i| dir.join(format!("{i}.wav"))).collect();
        for f in &files {
            std::fs::write(f, b"x").unwrap();
            memo.put(f, "{}".into()).unwrap();
        }
        let v0 = version(&files[0]).unwrap();
        assert!(memo.get(&files[0], &v0).is_none());
        let v1 = version(&files[1]).unwrap();
        assert!(memo.get(&files[1], &v1).is_some());
        assert!(memo.get(&files[1], "other").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
