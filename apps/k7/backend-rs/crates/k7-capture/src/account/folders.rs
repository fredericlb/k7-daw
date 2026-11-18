//! folders.rs — what the account does to a workspace's folder: a rename with its history and marker (`renameWorkspaceFolder`).
//! What a reader gets wrong: the folder moves under the host's lock, after the host let go of everything under it.

use std::io;
use std::path::Path;
use std::sync::Arc;

use super::Account;
use crate::disk::log_dir_of;

impl Account {
    /// Workspace folder `from` renamed to `to` with everything filed by its path: its sync history, its active marker (2154).
    pub async fn rename(self: &Arc<Self>, from: &str, to: &str) -> Result<(), String> {
        let host = self.host()?.clone();
        let (from_root, to_root) = (host.root_of(from), host.root_of(to));
        host.bundle.let_go_under(&from_root).await?;
        // No attach of the old root between the detach and the move.
        let attaching = self.attaching.lock().await;
        // The capture lets go first, so the move is not captured as every file removed and added again.
        self.detach(&from_root).await;
        let guard = host.bundle.lock().await;
        let h = host.clone();
        let (f, t) = (from_root.clone(), to_root.clone());
        let moved = tokio::task::spawn_blocking(move || -> io::Result<()> {
            std::fs::rename(&f, &t)?;
            let (lf, lt) = (log_dir_of(&h.home, &f), log_dir_of(&h.home, &t));
            if Path::new(&lf).exists() {
                std::fs::rename(&lf, &lt)?;
            }
            let (mf, mt) = (h.marker_of(&f), h.marker_of(&t));
            if Path::new(&mf).exists() {
                std::fs::rename(&mf, &mt)?;
            }
            Ok(())
        })
        .await
        .map_err(|e| e.to_string())
        .and_then(|r| r.map_err(|e| e.to_string()));
        drop(guard);
        drop(attaching);
        host.bundle.folders_changed();
        moved?;
        self.attach_named(to).await;
        Ok(())
    }
}
