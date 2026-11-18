//! gitlink.rs — turns a `.git` symlink to `.git.nosync` back into a real `.git` folder.
//! What a reader gets wrong: nothing is copied, the folder is renamed in place, and a real `.git` is never touched.

use std::path::Path;

use crate::Result;

/// The name an iCloud workspace once kept its repository under, behind a relative `.git` link.
const NOSYNC: &str = ".git.nosync";

/// Makes `workdir/.git` a real folder again; true when something changed, and a second call changes nothing.
pub fn restore_gitdir(workdir: &Path) -> Result<bool> {
    let (dot, nosync) = (workdir.join(".git"), workdir.join(NOSYNC));
    match std::fs::symlink_metadata(&dot) {
        Ok(m) if m.file_type().is_symlink() => {
            let to = std::fs::read_link(&dot)?;
            // Only the link this code once made is undone; any other link is the person's.
            if to != Path::new(NOSYNC) || !nosync.is_dir() {
                return Ok(false);
            }
            std::fs::remove_file(&dot)?;
            std::fs::rename(&nosync, &dot)?;
            Ok(true)
        }
        // A cut between the two steps left the folder alone under its old name.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound && nosync.join("HEAD").is_file() => {
            std::fs::rename(&nosync, &dot)?;
            Ok(true)
        }
        _ => Ok(false),
    }
}
