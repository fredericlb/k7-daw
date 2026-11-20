//! disk_fs.rs — song-edit's `Fs` over the disk, each write at once and atomic by file: for the bench's own folder and the stamp.
//! What a reader gets wrong: it is not whole-or-nothing across files; an edit of a person's files goes through `StagedFs`.

use song::{Entry, Source, SourceError, Span, Stat};
use song_edit::Fs;

use super::edge::Edge;
use crate::atomic::write_atomic;

pub struct DiskFs {
    disk: Edge,
    /// The folders a write may land in: the mounts, or the narrower set `reading` names.
    writes: Edge,
}

fn io(e: &std::io::Error) -> SourceError {
    match e.kind() {
        std::io::ErrorKind::NotFound => SourceError::NotFound,
        _ => SourceError::Io(e.to_string()),
    }
}

impl DiskFs {
    /// A disk confined to the folders `mounts`: a write outside them, or through a link out of them, is refused.
    ///
    /// # Errors
    /// A mount that is not an absolute path.
    pub fn new<S: AsRef<str>>(mounts: &[S]) -> Result<DiskFs, SourceError> {
        Ok(DiskFs {
            disk: Edge::new(mounts)?,
            writes: Edge::new(mounts)?,
        })
    }

    /// A disk that reads `reads` and the `writes` folders, and writes only inside `writes`.
    ///
    /// # Errors
    /// A mount that is not an absolute path.
    pub fn reading<S: AsRef<str>, T: AsRef<str>>(
        writes: &[S],
        reads: &[T],
    ) -> Result<DiskFs, SourceError> {
        let mut all: Vec<&str> = writes.iter().map(AsRef::as_ref).collect();
        all.extend(reads.iter().map(AsRef::as_ref));
        Ok(DiskFs {
            disk: Edge::new(&all)?,
            writes: Edge::new(writes)?,
        })
    }

    /// The path when it lies in a write mount, a mount not made yet included: a write elsewhere, or through a link out, is refused.
    fn confined(&self, path: &str) -> Result<String, SourceError> {
        let p = super::posix(path);
        match self.writes.stat(&p) {
            Err(SourceError::Outside) => Err(SourceError::Outside),
            _ => Ok(p),
        }
    }
}

impl Source for DiskFs {
    fn stamps_are_times(&self) -> bool {
        true
    }

    fn stat(&self, path: &str) -> Result<Stat, SourceError> {
        self.disk.stat(path)
    }

    fn list(&self, dir: &str) -> Result<Vec<Entry>, SourceError> {
        self.disk.list(dir)
    }

    fn read(&self, path: &str, span: Span) -> Result<Vec<u8>, SourceError> {
        self.disk.read(path, span)
    }
}

impl Fs for DiskFs {
    fn write_atomic(&mut self, path: &str, bytes: &[u8]) -> Result<(), SourceError> {
        let p = self.confined(path)?;
        write_atomic(&super::real(&p), bytes).map_err(|e| io(&e))
    }

    fn remove(&mut self, path: &str) -> Result<(), SourceError> {
        let p = self.confined(path)?;
        match std::fs::remove_file(super::real(&p)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(io(&e)),
            _ => Ok(()),
        }
    }

    fn remove_tree(&mut self, path: &str) -> Result<(), SourceError> {
        let p = self.confined(path)?;
        let gone = if std::fs::symlink_metadata(super::real(&p)).is_ok_and(|m| m.is_dir()) {
            std::fs::remove_dir_all(super::real(&p))
        } else {
            std::fs::remove_file(super::real(&p))
        };
        match gone {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(io(&e)),
            _ => Ok(()),
        }
    }

    fn rename(&mut self, from: &str, to: &str) -> Result<(), SourceError> {
        let (a, b) = (self.confined(from)?, self.confined(to)?);
        std::fs::rename(super::real(&a), super::real(&b)).map_err(|e| io(&e))
    }

    fn copy(&mut self, from: &str, to: &str) -> Result<(), SourceError> {
        let b = self.confined(to)?;
        // Read through the confined source: a link out of the mounts is refused, not followed.
        let bytes = self.disk.read(from, Span::All)?;
        write_atomic(&super::real(&b), &bytes).map_err(|e| io(&e))
    }

    fn mkdir_all(&mut self, dir: &str) -> Result<(), SourceError> {
        let p = self.confined(dir)?;
        std::fs::create_dir_all(super::real(&p)).map_err(|e| io(&e))
    }
}
