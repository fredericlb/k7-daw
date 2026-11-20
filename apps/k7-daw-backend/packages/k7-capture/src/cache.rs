//! cache.rs — a replica's local blob cache: the exact bytes a ref names, kept by sha256 while they are in flight, and
//! the files that wait for bytes, so a restart does not read their absence as a removal (`liveBlobs.ts`, 2135, 2143).
//! What a reader gets wrong: a snapshot hashes the COPY, never the source, which may change under the read.

use std::io::{self, ErrorKind, Read};
use std::path::Path;

use indexmap::IndexMap;
use k7_file_service::paths::join;
use k7_replica::blob::BlobId;
use k7_sync::fsx::{rename_retrying, write_atomic};
use k7_sync::json::stringify;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::disk::uuid_v4;

/// A copy that is a clone where the file system allows it, and a plain copy elsewhere.
pub fn clone_file(from: &str, to: &str) -> io::Result<()> {
    match std::fs::remove_file(to) {
        Err(e) if e.kind() != ErrorKind::NotFound => return Err(e),
        _ => {}
    }
    std::fs::copy(from, to).map(|_| ())
}

/// `clone_file` into place whole or not at all: a copy beside the target, hidden as an atomic write's, renamed over it.
pub fn place_file(from: &str, to: &str) -> io::Result<()> {
    let target = Path::new(to);
    let dir = target.parent().unwrap_or(Path::new("."));
    let name = target
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file");
    let temp = dir.join(format!(".{name}.{}.tmp", uuid_v4()?));
    std::fs::copy(from, &temp)
        .and_then(|_| rename_retrying(&temp, target))
        .inspect_err(|_| {
            let _ = std::fs::remove_file(&temp);
        })
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The sha256 and length of a file, read in chunks so a large take is never held whole.
pub fn sha256_file(abs: &str) -> io::Result<BlobId> {
    let mut f = std::fs::File::open(abs)?;
    let mut buf = vec![0u8; 1 << 20];
    let mut sha = Sha256::new();
    let mut size = 0u64;
    loop {
        let n = match f.read(&mut buf) {
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            r => r?,
        };
        if n == 0 {
            break;
        }
        sha.update(&buf[..n]);
        size += n as u64;
    }
    Ok(BlobId {
        hash: hex(&sha.finalize()),
        size: size as f64,
    })
}

pub fn sha256_bytes(buf: &[u8]) -> BlobId {
    BlobId {
        hash: hex(&Sha256::digest(buf)),
        size: buf.len() as f64,
    }
}

pub struct BlobCache {
    dir: String,
}

/// A snapshot's blob, and whether the cache held it already (`fresh` is false then).
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub id: BlobId,
    pub fresh: bool,
}

impl BlobCache {
    pub fn new(dir: &str) -> Self {
        BlobCache {
            dir: dir.to_string(),
        }
    }

    pub fn path(&self, hash: &str) -> String {
        join(&[&self.dir, hash])
    }

    pub fn has(&self, hash: &str) -> bool {
        Path::new(&self.path(hash)).exists()
    }

    /// Where a download of `hash` keeps the bytes that arrived, so a failed one resumes rather than restarts.
    pub fn partial(&self, hash: &str) -> io::Result<String> {
        std::fs::create_dir_all(&self.dir)?;
        Ok(join(&[&self.dir, &format!("part-{hash}")]))
    }

    /// A fresh file name in the cache, for a copy or a download that is not verified yet.
    pub fn tmp(&self) -> io::Result<String> {
        std::fs::create_dir_all(&self.dir)?;
        Ok(join(&[&self.dir, &format!("tmp-{}", uuid_v4()?)]))
    }

    /// Copy `abs` in, hash the COPY and keep it under that hash; a failed copy leaves no `tmp-` behind.
    pub fn snapshot(&self, abs: &str) -> io::Result<Snapshot> {
        let tmp = self.tmp()?;
        let made = clone_file(abs, &tmp)
            .and_then(|()| sha256_file(&tmp))
            .and_then(|id| {
                Ok(Snapshot {
                    fresh: self.keep(&tmp, &id.hash)?,
                    id,
                })
            });
        if made.is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
        made
    }

    /// Move a verified `tmp` under its hash; false (and the temp dropped) when that hash was there.
    pub fn keep(&self, tmp: &str, hash: &str) -> io::Result<bool> {
        if self.has(hash) {
            let _ = std::fs::remove_file(tmp);
            return Ok(false);
        }
        std::fs::rename(tmp, self.path(hash))?;
        Ok(true)
    }

    pub fn evict(&self, hash: &str) {
        let _ = std::fs::remove_file(self.path(hash));
        let _ = std::fs::remove_file(join(&[&self.dir, &format!("part-{hash}")]));
    }

    /// The files waiting for bytes, each with the ref it waits for; absent is empty, any other failure is an error.
    pub fn pending(&self) -> io::Result<IndexMap<String, String>> {
        let bytes = match std::fs::read(join(&[&self.dir, "pending.json"])) {
            Ok(b) => b,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(IndexMap::new()),
            Err(e) => return Err(e),
        };
        let bad =
            |why: &str| io::Error::new(ErrorKind::InvalidData, format!("pending.json: {why}"));
        let Value::Object(map) = serde_json::from_slice(&bytes).map_err(|e| bad(&e.to_string()))?
        else {
            return Err(bad("not an object"));
        };
        Ok(map
            .into_iter()
            .filter_map(|(k, v)| v.as_str().map(|s| (k, s.to_string())))
            .collect())
    }

    /// Saved BEFORE the ledger marks a file pending (2134); a save that fails is the caller's error.
    pub fn save_pending(&self, files: &IndexMap<String, String>) -> io::Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        let obj: serde_json::Map<String, Value> = files
            .iter()
            .map(|(k, v)| (k.clone(), Value::String(v.clone())))
            .collect();
        write_atomic(
            Path::new(&join(&[&self.dir, "pending.json"])),
            &stringify(&Value::Object(obj)),
        )
    }
}
