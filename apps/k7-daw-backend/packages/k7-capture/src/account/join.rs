//! join.rs — what the account does to capture every workspace of this machine: attach one by its name, list them, and
//! attach them all at start (`attachAllWorkspaces`). A workspace joins no relay room.
//! What a reader gets wrong: a folder removed while the start loop yielded is not attached again.

use std::path::Path;
use std::sync::Arc;

use k7_file_service::workspaces::BENCH;
use k7_file_service::workspaces::names::folder_names;

use super::{ACCOUNT_LIBRARY_ROOM, Account};

impl Account {
    /// Capture the workspace folder `name`, when it is there (none is made); the account says why when it cannot.
    pub async fn attach_named(self: &Arc<Self>, name: &str) {
        let Ok(host) = self.host().cloned() else {
            return;
        };
        let root = host.root_of(name);
        match tokio::fs::metadata(&root).await {
            Ok(m) if m.is_dir() => {}
            Ok(_) => return,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
            // Any other error is "not now", said, never read as a folder that is absent.
            Err(e) => return self.say(&format!("sync | cannot read workspace '{name}': {e}")),
        }
        if let Err(e) = self.attach(&root, name).await {
            self.say(&format!("sync | cannot capture workspace '{name}': {e}"));
        }
    }

    /// The workspace folders the account captures: every folder but the bench's.
    pub async fn workspace_names(&self) -> Vec<String> {
        let Ok(host) = self.host().cloned() else {
            return vec![];
        };
        tokio::task::spawn_blocking(move || {
            let mut names = folder_names(&host.workspaces);
            names.retain(|n| n != BENCH);
            names
        })
        .await
        .unwrap_or_default()
    }

    /// Capture every workspace on this machine from the start, not only those a request opens (2150).
    pub async fn attach_all(self: &Arc<Self>) {
        let Ok(host) = self.host().cloned() else {
            return;
        };
        if let Err(e) = host.bundle.migrate_namespaces().await {
            self.say(&format!(
                "library | the namespaces could not be rewritten: {e}"
            ));
        }
        for name in self.workspace_names().await {
            let root = host.root_of(&name);
            // Removed while this loop yielded: attaching would capture a folder that is gone.
            if Path::new(&root).exists() {
                self.attach_named(&name).await;
            }
        }
        let lib = host.library.clone();
        let made = tokio::task::spawn_blocking(move || std::fs::create_dir_all(&lib)).await;
        if matches!(made, Ok(Ok(())))
            && let Err(e) = self.attach(&host.library, ACCOUNT_LIBRARY_ROOM).await
        {
            self.say(&format!(
                "sync | the account's library could not be attached: {e}"
            ));
        }
    }
}
