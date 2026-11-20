//! store.rs — the LFS object store in the git folder, in git-lfs's own layout, and the cache of each file's hash.
//! What a reader gets wrong: an object is written through a temp name and checked by its hash before it takes its
//! final name, and the cache trusts a file's size and mtime together, never one of them.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::pointer::Pointer;
use crate::{Error, Result};

pub const STAT_FILE: &str = "k7-lfs-stat.json";

pub struct Store {
    root: PathBuf,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The hash and size of a file, read once.
pub fn hash_file(path: &Path) -> std::io::Result<Pointer> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    let mut size = 0u64;
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        size += n as u64;
    }
    Ok(Pointer {
        oid: hex(&h.finalize()),
        size,
    })
}

impl Store {
    pub fn in_gitdir(gitdir: &Path) -> Store {
        Store {
            root: gitdir.join("lfs"),
        }
    }

    /// `<root>/objects/aa/bb/<oid>`.
    pub fn path(&self, oid: &str) -> PathBuf {
        if !super::pointer::valid_oid(oid) {
            // Never a file: a name that is not an oid must not become a path.
            return self.root.join("objects").join("invalid");
        }
        self.root
            .join("objects")
            .join(&oid[0..2])
            .join(&oid[2..4])
            .join(oid)
    }

    pub fn has(&self, oid: &str) -> bool {
        self.path(oid).is_file()
    }

    pub fn remove(&self, oid: &str) -> std::io::Result<()> {
        match std::fs::remove_file(self.path(oid)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }

    /// Every object held, with its size.
    pub fn list(&self) -> Vec<Pointer> {
        let mut out = Vec::new();
        let Ok(a) = std::fs::read_dir(self.root.join("objects")) else {
            return out;
        };
        for a in a.flatten() {
            for b in std::fs::read_dir(a.path()).into_iter().flatten().flatten() {
                for f in std::fs::read_dir(b.path()).into_iter().flatten().flatten() {
                    let name = f.file_name().to_string_lossy().to_string();
                    if name.len() == 64 {
                        out.push(Pointer {
                            oid: name,
                            size: f.metadata().map(|m| m.len()).unwrap_or(0),
                        });
                    }
                }
            }
        }
        out
    }

    fn temp(&self) -> Result<PathBuf> {
        let dir = self.root.join("tmp");
        std::fs::create_dir_all(&dir)?;
        let mut nonce = [0u8; 8];
        let _ = getrandom::fill(&mut nonce);
        Ok(dir.join(format!("{}-{}", std::process::id(), hex(&nonce))))
    }

    fn place(&self, temp: &Path, oid: &str) -> Result<()> {
        let to = self.path(oid);
        if to.is_file() {
            let _ = std::fs::remove_file(temp);
            return Ok(());
        }
        std::fs::create_dir_all(to.parent().expect("an object has a folder"))?;
        std::fs::rename(temp, to).inspect_err(|_| {
            let _ = std::fs::remove_file(temp);
        })?;
        Ok(())
    }

    /// Copies a file in, hashing as it goes; the pointer says what was kept.
    pub fn put_file(&self, src: &Path) -> Result<Pointer> {
        let temp = self.temp()?;
        let copy = (|| -> std::io::Result<Pointer> {
            let mut from = std::fs::File::open(src)?;
            let mut to = std::fs::File::create(&temp)?;
            let (mut h, mut size, mut buf) = (Sha256::new(), 0u64, vec![0u8; 256 * 1024]);
            loop {
                let n = from.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                h.update(&buf[..n]);
                to.write_all(&buf[..n])?;
                size += n as u64;
            }
            to.sync_all()?;
            Ok(Pointer {
                oid: hex(&h.finalize()),
                size,
            })
        })();
        let p = match copy {
            Ok(p) => p,
            Err(e) => {
                let _ = std::fs::remove_file(&temp);
                return Err(e.into());
            }
        };
        self.place(&temp, &p.oid)?;
        Ok(p)
    }

    /// A download in progress: bytes are written to a temp and the object appears only if its hash is the one asked for.
    pub fn begin(&self) -> Result<Download> {
        let temp = self.temp()?;
        Ok(Download {
            file: std::fs::File::create(&temp)?,
            temp,
            hash: Sha256::new(),
            size: 0,
        })
    }

    pub fn finish(&self, d: Download, want: &Pointer) -> Result<()> {
        let Download {
            file,
            temp,
            hash,
            size,
        } = d;
        let _ = file.sync_all();
        let got = hex(&hash.finalize());
        if got != want.oid || size != want.size {
            let _ = std::fs::remove_file(&temp);
            return Err(Error::Network(format!(
                "the server sent {} where object {} was asked",
                got, want.oid
            )));
        }
        self.place(&temp, &want.oid)
    }
}

pub struct Download {
    file: std::fs::File,
    temp: PathBuf,
    hash: Sha256,
    size: u64,
}

impl Download {
    pub fn write(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        self.hash.update(bytes);
        self.size += bytes.len() as u64;
        self.file.write_all(bytes)
    }

    pub fn written(&self) -> u64 {
        self.size
    }

    pub fn abandon(self) {
        let _ = std::fs::remove_file(&self.temp);
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
struct Seen {
    size: u64,
    mtime: u128,
    oid: String,
}

/// The hash of each tracked file as of its size and mtime, so a take is hashed once and not at every status.
pub struct StatCache {
    file: PathBuf,
    seen: BTreeMap<String, Seen>,
    dirty: bool,
}

fn mtime(m: &std::fs::Metadata) -> u128 {
    m.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

impl StatCache {
    /// A missing or unreadable cache is an empty one: the cost is a hash, never a wrong answer.
    pub fn load(gitdir: &Path) -> StatCache {
        let file = gitdir.join(STAT_FILE);
        let seen = std::fs::read(&file)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        StatCache {
            file,
            seen,
            dirty: false,
        }
    }

    /// The hash of `abs`, from the cache when its size and mtime are the ones recorded.
    pub fn pointer(&mut self, rel: &str, abs: &Path) -> std::io::Result<Pointer> {
        let meta = std::fs::metadata(abs)?;
        let (size, at) = (meta.len(), mtime(&meta));
        if let Some(s) = self.seen.get(rel)
            && s.size == size
            && s.mtime == at
        {
            return Ok(Pointer {
                oid: s.oid.clone(),
                size,
            });
        }
        let p = hash_file(abs)?;
        self.seen.insert(
            rel.to_string(),
            Seen {
                size: p.size,
                mtime: at,
                oid: p.oid.clone(),
            },
        );
        self.dirty = true;
        Ok(p)
    }

    /// Records a hash already known (a file just written from the store).
    pub fn remember(&mut self, rel: &str, abs: &Path, p: &Pointer) {
        if let Ok(m) = std::fs::metadata(abs) {
            self.seen.insert(
                rel.to_string(),
                Seen {
                    size: p.size,
                    mtime: mtime(&m),
                    oid: p.oid.clone(),
                },
            );
            self.dirty = true;
        }
    }

    pub fn save(&mut self) {
        if self.dirty
            && let Ok(text) = serde_json::to_vec(&self.seen)
        {
            let temp = self.file.with_extension("json.tmp");
            if std::fs::write(&temp, text).is_ok() {
                let _ = std::fs::rename(&temp, &self.file);
            }
            self.dirty = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("k7-lfs-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_name_that_is_not_an_oid_is_never_a_path() {
        let s = Store::in_gitdir(Path::new("/g"));
        for bad in [
            "/etc/passwd",
            "../../x",
            "é",
            "ab",
            &"A".repeat(64),
            &format!("{}/", "a".repeat(63)),
        ] {
            assert_eq!(s.path(bad), Path::new("/g/lfs/objects/invalid"), "{bad}");
            assert!(!s.has(bad));
        }
    }

    #[test]
    fn an_object_lives_at_the_layout_git_lfs_uses_and_is_kept_once() {
        let d = dir("layout");
        std::fs::write(d.join("a.wav"), b"hello").unwrap();
        let s = Store::in_gitdir(&d);
        let p = s.put_file(&d.join("a.wav")).unwrap();
        assert_eq!(
            p.oid,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
        assert_eq!(s.path(&p.oid), d.join("lfs/objects/2c/f2").join(&p.oid));
        assert_eq!(std::fs::read(s.path(&p.oid)).unwrap(), b"hello");
        s.put_file(&d.join("a.wav")).unwrap();
        assert_eq!(s.list(), vec![p.clone()]);
        assert!(
            !d.join("lfs/tmp").read_dir().unwrap().any(|_| true),
            "no temp is left"
        );
        s.remove(&p.oid).unwrap();
        assert!(!s.has(&p.oid));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_download_whose_hash_is_not_the_one_asked_never_becomes_an_object() {
        let d = dir("dl");
        let s = Store::in_gitdir(&d);
        let want = Pointer {
            oid: "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824".into(),
            size: 5,
        };
        let mut dl = s.begin().unwrap();
        dl.write(b"hellp").unwrap();
        assert!(s.finish(dl, &want).is_err());
        assert!(!s.has(&want.oid));
        let mut dl = s.begin().unwrap();
        dl.write(b"hello").unwrap();
        s.finish(dl, &want).unwrap();
        assert!(s.has(&want.oid));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_cache_hashes_a_file_once_and_again_when_its_size_or_time_moved() {
        let d = dir("cache");
        let f = d.join("t.wav");
        std::fs::write(&f, b"one").unwrap();
        let mut c = StatCache::load(&d);
        let a = c.pointer("t.wav", &f).unwrap();
        c.save();
        std::fs::write(&f, b"two").unwrap();
        let mut c = StatCache::load(&d);
        let b = c.pointer("t.wav", &f).unwrap();
        assert_ne!(a.oid, b.oid, "same size, new mtime: hashed again");
        let cached = StatCache::load(&d);
        assert_eq!(
            cached.seen.get("t.wav").map(|s| s.oid.clone()),
            Some(a.oid),
            "what was saved is the first hash"
        );
        let _ = std::fs::remove_dir_all(&d);
    }
}
