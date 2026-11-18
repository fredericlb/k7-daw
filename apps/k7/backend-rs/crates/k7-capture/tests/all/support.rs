//! support — what the tests share: a scratch folder that cleans up after itself and the vectors' reader.
//! What a reader gets wrong: a scratch folder is per call, so two tests never share a disk.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::Value;

pub struct Scratch(PathBuf);

impl Scratch {
    pub fn new(name: &str) -> Scratch {
        static N: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "k7-capture-{name}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    /// A path under the folder, spelled with `/` as the crate spells every path.
    pub fn at(&self, rel: &str) -> String {
        format!("{}/{rel}", self.0.display()).replace('\\', "/")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The TypeScript's own answers, written by `js/makeVectors.mts`.
pub fn vectors() -> Value {
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/capture-vectors.json");
    serde_json::from_slice(&std::fs::read(file).expect("fixtures/capture-vectors.json")).unwrap()
}

/// A table of the vectors, as rows.
pub fn table(name: &str) -> Vec<Value> {
    vectors()[name]
        .as_array()
        .unwrap_or_else(|| panic!("no table {name}"))
        .clone()
}

/// Makes `file` unreadable but replaceable, as a lock or a permission does; false where the host cannot.
pub fn lock(file: &str) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o0)).is_ok()
            && std::fs::read(file).is_err()
    }
    #[cfg(not(unix))]
    {
        let _ = file;
        false
    }
}

/// Gives a locked file its reading back.
pub fn unlock(file: &str) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o644));
    }
    #[cfg(not(unix))]
    let _ = file;
}

/// Sets a file's or folder's mode; false where the host has none.
pub fn set_mode(path: &str, mode: u32) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).is_ok()
    }
    #[cfg(not(unix))]
    {
        let _ = (path, mode);
        false
    }
}

/// A link at `at` to `to`, typed as Windows types it; false where the host refuses links.
pub fn link(to: &str, at: &str, folder: bool) -> bool {
    #[cfg(unix)]
    {
        let _ = folder;
        std::os::unix::fs::symlink(to, at).is_ok()
    }
    #[cfg(windows)]
    {
        if folder {
            std::os::windows::fs::symlink_dir(to, at).is_ok()
        } else {
            std::os::windows::fs::symlink_file(to, at).is_ok()
        }
    }
}

/// Waits until `cond` holds, or fails after a generous bound.
pub async fn wait_for(what: &str, mut cond: impl FnMut() -> bool) {
    for _ in 0..6000 {
        if cond() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("never happened: {what}");
}
