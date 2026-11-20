//! confirmed.rs — a replica's confirmed text on disk: the room's files as it last applied them, in a mirror folder
//! beside the workspace's log, never inside the workspace (`liveConfirmed.ts`).
//! What a reader gets wrong: a write is atomic but not durable, and only `NotFound` reads as an absent file (2102); a
//! batch holds its sets in memory until `commit`, and `get` and `files` answer from them (2131b).

use std::collections::{HashMap, HashSet};
use std::io::{self, ErrorKind, Write};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

use indexmap::IndexMap;
use sha2::{Digest as _, Sha256};

use k7_file_service::paths::{join, resolve_all};
use k7_replica::Confirmed;
use k7_sync::fsx::rename_retrying;

use crate::disk::confined_path;

pub struct ConfirmedDir {
    root: String,
    /// The hash of every file's bytes as last read or written here, once built: a `set` of the same text writes nothing.
    index: Option<HashMap<String, Digest>>,
    writes: AtomicUsize,
    /// While a batch is open: what was set and not yet written out, a file's text or `None` for a removal.
    batch: Option<IndexMap<String, Option<String>>>,
}

type Digest = [u8; 32];

/// No text hashes to this: the index's entry for a file whose bytes could not be read.
const UNKNOWN: Digest = [0; 32];

fn digest(bytes: &[u8]) -> Digest {
    Sha256::digest(bytes).into()
}

/// `isAtomicTemp`: `.<name>.<pid>.<8 hex>.tmp`, the temp an atomic write leaves if it dies mid-write.
pub fn is_atomic_temp(name: &str) -> bool {
    let Some(stem) = name.strip_prefix('.').and_then(|n| n.strip_suffix(".tmp")) else {
        return false;
    };
    let Some((rest, hexpart)) = stem.rsplit_once('.') else {
        return false;
    };
    let Some((front, pid)) = rest.rsplit_once('.') else {
        return false;
    };
    hexpart.len() == 8
        && hexpart
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        && !pid.is_empty()
        && pid.bytes().all(|b| b.is_ascii_digit())
        && !front.is_empty()
}

/// A file that is not there: a path below a file is `ENOTDIR` here and `NotFound` on Windows, absent on both (2132).
fn gone(e: &io::Error) -> bool {
    matches!(e.kind(), ErrorKind::NotFound | ErrorKind::NotADirectory)
}

fn invalid(why: String) -> io::Error {
    io::Error::new(ErrorKind::InvalidInput, why)
}

impl ConfirmedDir {
    pub fn new(root: &str) -> Self {
        ConfirmedDir {
            root: root.to_string(),
            index: None,
            writes: AtomicUsize::new(0),
            batch: None,
        }
    }

    /// From now on `set` only holds its change in memory, until `commit`: the workspace is written first (2131b).
    pub fn begin_batch(&mut self) {
        self.batch.get_or_insert_with(IndexMap::new);
    }

    /// Write out what the batch holds, in the order it was set, and close it; the files that failed are named.
    pub fn commit(&mut self) -> Vec<String> {
        self.commit_chunk(usize::MAX).0
    }

    /// Writes out the next `n` files of the batch, closing it when none is left; `get` and `files` answer from what is left.
    pub fn commit_chunk(&mut self, n: usize) -> (Vec<String>, bool) {
        let Some(batch) = self.batch.as_mut() else {
            return (Vec::new(), true);
        };
        let take = n.min(batch.len());
        let held: Vec<(String, Option<String>)> = batch.drain(..take).collect();
        let done = batch.is_empty();
        if done {
            self.batch = None;
        }
        let mut failed = Vec::new();
        for (file, text) in held {
            if self.put(&file, text.as_deref()).is_err() {
                failed.push(file);
            }
        }
        (failed, done)
    }

    /// The workspace write of these files failed: their `confirmed/` change is not made, and `get` reads the disk again.
    pub fn unbatch(&mut self, files: &[String]) {
        for f in files {
            if let Some(b) = self.batch.as_mut() {
                b.shift_remove(f);
            }
            if let Some(index) = self.index.as_mut() {
                index.insert(f.clone(), UNKNOWN);
            }
        }
    }

    /// The disk's change for one file, the index kept honest: unknown while the change is under way and after it failed.
    fn put(&mut self, file: &str, text: Option<&str>) -> io::Result<()> {
        let abs = self.abs(file)?;
        let want = text.map(|t| digest(t.as_bytes()));
        if let Some(index) = self.index.as_mut() {
            index.insert(file.to_string(), UNKNOWN);
        }
        self.writes.fetch_add(1, Ordering::SeqCst);
        match text {
            None => match std::fs::remove_file(&abs) {
                Err(e) if !gone(&e) => return Err(e),
                _ => {}
            },
            Some(t) => {
                if let Some(dir) = Path::new(&abs).parent() {
                    std::fs::create_dir_all(dir)?;
                }
                write_light(Path::new(&abs), t)?;
            }
        }
        if let Some(index) = self.index.as_mut() {
            match want {
                Some(d) => index.insert(file.to_string(), d),
                None => index.remove(file),
            };
        }
        Ok(())
    }

    /// One read of every file held, outside any lock; a file that cannot be read is UNKNOWN, so it is written or removed.
    pub fn indexed(root: &str) -> Self {
        let mut dir = ConfirmedDir::new(root);
        // A folder that cannot be listed gets no index: an error is never read as an empty folder (2102).
        let Ok(held) = dir.files() else { return dir };
        let mut index = HashMap::new();
        for file in held {
            let held = match dir.abs(&file).and_then(std::fs::read) {
                Ok(bytes) => digest(&bytes),
                Err(_) => UNKNOWN,
            };
            index.insert(file, held);
        }
        dir.index = Some(index);
        dir
    }

    /// Files written or removed on the disk so far: a test counts what a write phase spent.
    #[doc(hidden)]
    pub fn writes(&self) -> usize {
        self.writes.load(Ordering::SeqCst)
    }

    fn abs(&self, file: &str) -> io::Result<String> {
        confined_path(&self.root, file).map_err(invalid)
    }
}

/// A write through a hidden temp and a rename, with no fsync: a mirror torn by a crash only resends text the room holds.
fn write_light(file: &Path, text: &str) -> io::Result<()> {
    let dir = file.parent().unwrap_or(Path::new("."));
    let name = file.file_name().and_then(|n| n.to_str()).unwrap_or("file");
    let mut nonce = [0u8; 4];
    let _ = getrandom::fill(&mut nonce);
    let temp = dir.join(format!(
        ".{name}.{}.{:08x}.tmp",
        std::process::id(),
        u32::from_le_bytes(nonce)
    ));
    std::fs::File::create(&temp)?.write_all(text.as_bytes())?;
    rename_retrying(&temp, file).inspect_err(|_| {
        let _ = std::fs::remove_file(&temp);
    })
}

impl Confirmed for ConfirmedDir {
    fn get(&self, file: &str) -> io::Result<Option<String>> {
        if let Some(held) = self.batch.as_ref().and_then(|b| b.get(file)) {
            return Ok(held.clone());
        }
        match std::fs::read(self.abs(file)?) {
            Ok(b) => Ok(Some(String::from_utf8_lossy(&b).into_owned())),
            Err(e) if gone(&e) => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn set(&mut self, file: &str, text: Option<&str>) -> io::Result<()> {
        self.abs(file)?;
        let want = text.map(|t| digest(t.as_bytes()));
        if let Some(index) = &self.index
            && index.get(file) == want.as_ref()
        {
            return Ok(());
        }
        match self.batch.as_mut() {
            Some(batch) => {
                batch.insert(file.to_string(), text.map(str::to_string));
                if let Some(index) = self.index.as_mut() {
                    match want {
                        Some(d) => index.insert(file.to_string(), d),
                        None => index.remove(file),
                    };
                }
                Ok(())
            }
            None => self.put(file, text),
        }
    }

    /// Every file held, a folder's files before its folders, in name order, torn temps left out; a folder link is a file.
    fn files(&self) -> io::Result<Vec<String>> {
        let listed = self.listed()?;
        let Some(batch) = self.batch.as_ref().filter(|b| !b.is_empty()) else {
            return Ok(listed);
        };
        let mut held: HashSet<String> = listed.into_iter().collect();
        for (file, text) in batch {
            if text.is_some() {
                held.insert(file.clone());
            } else {
                held.remove(file);
            }
        }
        let mut out: Vec<String> = held.into_iter().collect();
        out.sort_by_cached_key(|f| order_key(f));
        Ok(out)
    }
}

/// The order `listed` walks in: a folder's files by name, then its folders, each folder in turn.
fn order_key(file: &str) -> Vec<(u8, String)> {
    let parts: Vec<&str> = file.split('/').collect();
    let last = parts.len() - 1;
    parts
        .iter()
        .enumerate()
        .map(|(i, p)| (u8::from(i < last), p.to_string()))
        .collect()
}

impl ConfirmedDir {
    fn listed(&self) -> io::Result<Vec<String>> {
        let root = resolve_all(&[&self.root]);
        let mut out = vec![];
        let mut stack = vec![root.clone()];
        while let Some(dir) = stack.pop() {
            let mut entries = match std::fs::read_dir(&dir) {
                Ok(r) => r.collect::<Result<Vec<_>, _>>()?,
                Err(e) if e.kind() == ErrorKind::NotFound && dir == root => return Ok(out),
                Err(e) => return Err(e),
            };
            entries.sort_by_key(|e| e.file_name());
            let mut below = vec![];
            for e in entries {
                let name = e.file_name().to_string_lossy().into_owned();
                let child = join(&[&dir, &name]);
                if e.file_type()?.is_dir() {
                    below.push(child);
                } else if !is_atomic_temp(&name) {
                    out.push(child[root.len()..].trim_start_matches('/').to_string());
                }
            }
            stack.extend(below.into_iter().rev());
        }
        Ok(out)
    }
}
