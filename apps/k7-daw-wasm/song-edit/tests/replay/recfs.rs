// recfs.rs — a file system that lists the paths written through it, in order, for the `snapshot` the stubs' `write` import runs.
// A reader would get wrong that only `write_atomic` is recorded: that is the one call the TypeScript snapshots before.

use song::{Entry, Source, SourceError, Span, Stat};
use song_edit::{Fs, MemFs};

pub struct RecFs<'a> {
    pub inner: &'a mut MemFs,
    pub writes: Vec<String>,
}

impl Source for RecFs<'_> {
    fn stat(&self, path: &str) -> Result<Stat, SourceError> {
        self.inner.stat(path)
    }
    fn list(&self, dir: &str) -> Result<Vec<Entry>, SourceError> {
        self.inner.list(dir)
    }
    fn read(&self, path: &str, span: Span) -> Result<Vec<u8>, SourceError> {
        self.inner.read(path, span)
    }
}

impl Fs for RecFs<'_> {
    fn write_atomic(&mut self, path: &str, bytes: &[u8]) -> Result<(), SourceError> {
        self.writes.push(path.to_owned());
        self.inner.write_atomic(path, bytes)
    }
    fn remove(&mut self, path: &str) -> Result<(), SourceError> {
        self.inner.remove(path)
    }
    fn remove_tree(&mut self, path: &str) -> Result<(), SourceError> {
        self.inner.remove_tree(path)
    }
    fn rename(&mut self, from: &str, to: &str) -> Result<(), SourceError> {
        self.inner.rename(from, to)
    }
    fn copy(&mut self, from: &str, to: &str) -> Result<(), SourceError> {
        self.inner.copy(from, to)
    }
    fn mkdir_all(&mut self, dir: &str) -> Result<(), SourceError> {
        self.inner.mkdir_all(dir)
    }
}
