//! history.rs — the account's side of a workspace's own history: the rooms it keeps on this device, a room's first entry
//! when a workspace already has files, and the room let go of before its folder moves.
//! What a reader gets wrong: a room is seeded only while empty, so a restart never writes a second "starts here".

use std::sync::Arc;

use super::Account;
use crate::capture::{Baseline, LiveCapture, Options};
use crate::history::Histories;

impl Account {
    /// The device's history rooms, their idle timer started on first use.
    pub fn histories(&self) -> Arc<Histories> {
        let h = self
            .history
            .get_or_init(|| {
                let (roll, snap) = self.cadence;
                Arc::new(Histories::with(self.log.clone(), roll, snap))
            })
            .clone();
        if tokio::runtime::Handle::try_current().is_ok() {
            h.start();
        }
        h
    }

    /// The baseline of a recording workspace, its room first made to hold the files it has; None says why in the history.
    pub(super) async fn baseline_for(
        &self,
        h: &Arc<Histories>,
        root: &str,
        log_dir: &str,
        o: &Options,
    ) -> Option<Arc<dyn Baseline>> {
        match self.seed_history(h, log_dir, o).await {
            Ok(()) => {
                super::lock(&self.history_errors).remove(&super::root_key(root));
                Some(h.baseline(log_dir))
            }
            Err(e) => {
                self.say(&format!(
                    "history | {root}: no history room, capture goes on: {e}"
                ));
                super::lock(&self.history_errors).insert(super::root_key(root), e);
                None
            }
        }
    }

    /// An empty room is given the workspace's files as its first entry, so every later edit finds the text it replaces.
    async fn seed_history(
        &self,
        h: &Arc<Histories>,
        log_dir: &str,
        o: &Options,
    ) -> Result<(), String> {
        let (h, log_dir) = (h.clone(), log_dir.to_string());
        let mut so = Options::new(&o.root, &o.replica, o.k7x.clone());
        so.blob_dir = o.blob_dir.clone();
        let root = o.root.clone();
        tokio::task::spawn_blocking(move || -> Result<(), String> {
            if !h.is_empty(&log_dir)? {
                return Ok(());
            }
            let files = LiveCapture::new(so)
                .texts()
                .map_err(|e| format!("{root}: {e}"))?;
            h.reconcile(&log_dir, &files, "history starts here")
        })
        .await
        .map_err(|e| e.to_string())?
    }
}
