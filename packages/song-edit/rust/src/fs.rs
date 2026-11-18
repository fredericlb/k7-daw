// fs.rs — the file system a song-edit function reads and writes through: song's read-only `Source` plus the writes.
// A reader would get wrong that `Source::list` is sorted: a TypeScript `readdir` is in disk order, so a caller that cares sorts again.

use song::{Source, SourceError};

pub trait Fs: Source {
    /// Replaces the file whole or not at all; the folder must exist.
    fn write_atomic(&mut self, path: &str, bytes: &[u8]) -> Result<(), SourceError>;
    /// `rmSync(path, { force: true })`: a file that is not there is no error, a folder is.
    fn remove(&mut self, path: &str) -> Result<(), SourceError>;
    /// `rmSync(path, { recursive: true, force: true })`: a folder and all it holds, or a file; absent is no error.
    fn remove_tree(&mut self, path: &str) -> Result<(), SourceError>;
    fn rename(&mut self, from: &str, to: &str) -> Result<(), SourceError>;
    fn copy(&mut self, from: &str, to: &str) -> Result<(), SourceError>;
    fn mkdir_all(&mut self, dir: &str) -> Result<(), SourceError>;

    /// `existsSync`: links followed.
    fn exists(&self, path: &str) -> bool {
        self.stat(path).is_ok()
    }

    /// `lstat` succeeds: a dangling link is present, which `real_path` then refuses.
    fn present(&self, path: &str) -> bool {
        self.exists(path)
    }

    /// Whether `path` is a link on the disk, which a clone refuses to copy; a memory file system holds none.
    fn is_link(&self, _path: &str) -> bool {
        false
    }

    /// `realpathSync`, or None when it cannot be read.
    fn real_path(&self, path: &str) -> Option<String> {
        self.exists(path).then(|| path.to_owned())
    }
}
