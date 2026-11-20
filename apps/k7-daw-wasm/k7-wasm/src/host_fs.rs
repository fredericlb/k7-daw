// host_fs.rs — song-edit's `Fs` over the host's imports (abi.rs): the Vfs in the Worker, the disk in node.
// A reader would get wrong that a path the host lacks is not an error here: it is noted, and the call is rerun once it is fetched.

use crate::abi::{self, FsErr};
use serde::Deserialize;
use song::{Entry, Kind, Source, SourceError, Span, Stamp, Stat};
use song_edit::Fs;
use std::cell::RefCell;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StatWire {
    kind: Kind,
    len: u64,
    mtime_ns: String,
    settled: bool,
}

/// The imports as a file system, remembering the paths the host could not answer.
#[derive(Default)]
pub struct HostFs {
    missing: RefCell<Vec<String>>,
}

impl HostFs {
    pub fn new() -> HostFs {
        HostFs::default()
    }

    /// The paths to fetch before the call is run again; empty when it was answered whole.
    pub fn take_missing(&self) -> Vec<String> {
        let mut m = std::mem::take(&mut *self.missing.borrow_mut());
        m.sort();
        m.dedup();
        m
    }

    fn mapped<T>(&self, path: &str, r: Result<T, FsErr>) -> Result<T, SourceError> {
        r.map_err(|e| match e {
            FsErr::NotFound => SourceError::NotFound,
            FsErr::Needs => {
                self.missing.borrow_mut().push(path.to_owned());
                SourceError::Missing
            }
            FsErr::Io(m) => SourceError::Io(m),
        })
    }

    fn json<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> Result<T, SourceError> {
        serde_json::from_slice(bytes).map_err(|e| SourceError::Io(e.to_string()))
    }
}

impl Source for HostFs {
    fn stat(&self, path: &str) -> Result<Stat, SourceError> {
        let bytes = self.mapped(path, abi::fs_stat(path))?;
        let w: StatWire = HostFs::json(&bytes)?;
        Ok(Stat {
            kind: w.kind,
            len: w.len,
            stamp: Stamp {
                mtime_ns: w.mtime_ns.parse().unwrap_or(0),
                len: w.len,
                settled: w.settled,
            },
        })
    }

    fn list(&self, dir: &str) -> Result<Vec<Entry>, SourceError> {
        let bytes = self.mapped(dir, abi::fs_list(dir))?;
        let mut entries: Vec<Entry> = HostFs::json(&bytes)?;
        entries.sort_by(|a, b| a.name.as_bytes().cmp(b.name.as_bytes()));
        Ok(entries)
    }

    fn read(&self, path: &str, span: Span) -> Result<Vec<u8>, SourceError> {
        let bytes = self.mapped(path, abi::fs_read(path))?;
        Ok(match span {
            Span::All => bytes,
            Span::Range { offset, len } => {
                let from = usize::try_from(offset).unwrap_or(usize::MAX).min(bytes.len());
                let to = from
                    .saturating_add(usize::try_from(len).unwrap_or(usize::MAX))
                    .min(bytes.len());
                bytes[from..to].to_vec()
            }
        })
    }
}

impl Fs for HostFs {
    fn write_atomic(&mut self, path: &str, bytes: &[u8]) -> Result<(), SourceError> {
        self.mapped(path, abi::fs_write(path, bytes)).map(drop)
    }

    fn remove(&mut self, path: &str) -> Result<(), SourceError> {
        self.mapped(path, abi::fs_remove(path)).map(drop)
    }

    fn remove_tree(&mut self, path: &str) -> Result<(), SourceError> {
        self.mapped(path, abi::fs_remove_tree(path)).map(drop)
    }

    fn rename(&mut self, from: &str, to: &str) -> Result<(), SourceError> {
        self.mapped(from, abi::fs_rename(from, to)).map(drop)
    }

    fn copy(&mut self, from: &str, to: &str) -> Result<(), SourceError> {
        self.mapped(from, abi::fs_copy(from, to)).map(drop)
    }

    fn mkdir_all(&mut self, dir: &str) -> Result<(), SourceError> {
        self.mapped(dir, abi::fs_mkdir(dir)).map(drop)
    }

    fn real_path(&self, path: &str) -> Option<String> {
        abi::fs_real_path(path)
            .ok()
            .map(|b| String::from_utf8_lossy(&b).into_owned())
    }
}
