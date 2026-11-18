//! disk.rs — the scratch folders a share makes and the small disk checks they share.
//! What a reader gets wrong: a `Folder` removes itself when dropped, so whoever holds it owns the cleanup.

use std::io;
use std::path::{Path, PathBuf};

/// A random hex name.
pub fn random() -> String {
    let mut r = [0u8; 12];
    let _ = getrandom::fill(&mut r);
    r.iter().map(|b| format!("{b:02x}")).collect()
}

/// A private folder under the account's temporary folder, removed when dropped (2522, 2545).
#[derive(Debug)]
pub struct Folder {
    path: PathBuf,
}

impl Folder {
    pub fn make(under: &Path, prefix: &str) -> io::Result<Folder> {
        std::fs::create_dir_all(under)?;
        let path = under.join(format!("{prefix}{}", random()));
        make_private(&path)?;
        Ok(Folder { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[cfg(unix)]
fn make_private(p: &Path) -> io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new().mode(0o700).create(p)
}

#[cfg(not(unix))]
fn make_private(p: &Path) -> io::Result<()> {
    std::fs::create_dir(p)
}

/// A program found on `path_var`'s folders, as the shell would find it; a name with a separator is taken as it is.
pub fn which(name: &str, path_var: Option<&str>) -> Option<PathBuf> {
    if name.contains(['/', '\\']) {
        return Path::new(name).is_file().then(|| PathBuf::from(name));
    }
    let exts: &[&str] = if cfg!(windows) { &["", ".exe"] } else { &[""] };
    std::env::split_paths(path_var?)
        .flat_map(|d| exts.iter().map(move |e| d.join(format!("{name}{e}"))))
        .find(|p| p.is_file())
}

/// Whether the real path of `p` lies inside the real path of `root`.
pub fn real_inside(root: &Path, p: &Path) -> Option<PathBuf> {
    let root = std::fs::canonicalize(root).ok()?;
    let real = std::fs::canonicalize(p).ok()?;
    real.starts_with(&root).then_some(real)
}
