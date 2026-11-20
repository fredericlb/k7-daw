//! uploads.rs — a file the engine's machine sends up, written: beside its target
//! under a random `.tunnel-<hex>.part` name created exclusively, renamed over the target only once complete,
//! and deleted when the upload fails. Only a `writable` root takes one, inside its real tree.

use std::path::{Path, PathBuf};

use tokio::io::AsyncWriteExt;

use super::files::{mtime_ms, root_of, still_inside};
use super::frames::{RemoteStat, Root};

/// What one tunnel may upload: open at once, per upload, per hour, and how long without a chunk.
#[derive(Clone, Copy, Debug)]
pub struct UploadLimits {
    pub max_open: usize,
    pub max_bytes: u64,
    pub bytes_per_hour: u64,
    pub idle_ms: u64,
}

const GIB: u64 = 1024 * 1024 * 1024;

/// Bytes from a setting, or `fallback` when unset or not a positive number.
fn env_bytes(get: &dyn Fn(&str) -> Option<String>, name: &str, fallback: u64) -> u64 {
    get(name)
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|n| n.is_finite() && *n > 0.0)
        .map_or(fallback, |n| n as u64)
}

impl UploadLimits {
    /// Two of them configurable: `STUDIO_TUNNEL_MAX_UPLOAD_BYTES`, `STUDIO_TUNNEL_UPLOAD_BYTES_PER_HOUR`.
    pub fn from_settings(get: &dyn Fn(&str) -> Option<String>) -> Self {
        UploadLimits {
            max_open: 16,
            max_bytes: env_bytes(get, "STUDIO_TUNNEL_MAX_UPLOAD_BYTES", 2 * GIB),
            bytes_per_hour: env_bytes(get, "STUDIO_TUNNEL_UPLOAD_BYTES_PER_HOUR", 20 * GIB),
            idle_ms: 60_000,
        }
    }
}

impl Default for UploadLimits {
    fn default() -> Self {
        Self::from_settings(&|_| None)
    }
}

/// `path.dirname` of a server path, in the server's own spelling.
fn dirname(p: &str) -> String {
    Path::new(p)
        .parent()
        .map_or_else(|| p.to_string(), |d| d.to_string_lossy().into_owned())
}

pub struct Upload {
    path: String,
    tmp: PathBuf,
    out: Option<tokio::fs::File>,
}

impl Upload {
    pub async fn start(roots: &[Root], path: &str) -> Result<Upload, String> {
        let root = root_of(roots, path)
            .filter(|r| r.writable)
            .ok_or_else(|| format!("not writable from the engine: {path}"))?;
        let dir = dirname(path);
        if !still_inside(root, &dir).await {
            return Err(format!("outside the shared tree: {path}"));
        }
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|e| e.to_string())?;
        let mut nonce = [0u8; 8];
        getrandom::fill(&mut nonce).map_err(|e| e.to_string())?;
        let hex: String = nonce.iter().map(|b| format!("{b:02x}")).collect();
        let tmp = PathBuf::from(format!("{path}.tunnel-{hex}.part"));
        let out = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .await
            .map_err(|e| e.to_string())?;
        Ok(Upload {
            path: path.to_string(),
            tmp,
            out: Some(out),
        })
    }

    pub async fn write(&mut self, chunk: &[u8]) -> Result<(), String> {
        let out = self.out.as_mut().ok_or("the upload is closed")?;
        out.write_all(chunk).await.map_err(|e| e.to_string())
    }

    pub async fn finish(mut self) -> Result<RemoteStat, String> {
        let mut out = self.out.take().ok_or("the upload is closed")?;
        let done = async {
            out.flush().await?;
            drop(out);
            tokio::fs::rename(&self.tmp, &self.path).await?;
            tokio::fs::metadata(&self.path).await
        }
        .await;
        match done {
            Ok(m) => Ok(RemoteStat::File {
                path: self.path.clone(),
                size: m.len(),
                mtime_ms: mtime_ms(&m),
            }),
            Err(e) => {
                let _ = tokio::fs::remove_file(&self.tmp).await;
                Err(e.to_string())
            }
        }
    }

    pub async fn abort(mut self) {
        drop(self.out.take());
        let _ = tokio::fs::remove_file(&self.tmp).await;
    }
}
