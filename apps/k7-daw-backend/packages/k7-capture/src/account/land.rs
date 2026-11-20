//! land.rs — how a pull's file reaches the disk of a captured workspace: through its capture, so its ledger learns the file first.
//! What a reader gets wrong: the capture's worker does the write, so the ledger entry and the file never disagree for a sweep.

use std::sync::Arc;

use super::Account;
use crate::landing::Land;

impl Account {
    /// How a fetched file reaches the disk: through the workspace's capture, so its ledger learns it first.
    pub fn landing(self: &Arc<Self>) -> Land {
        let me = Arc::downgrade(self);
        Arc::new(move |root, rel, bytes| {
            let me = me.clone();
            Box::pin(async move {
                let at = me.upgrade()?.attached_at(&root)?;
                at.handle.run(move |c| c.land(&rel, bytes.as_deref())).await
            })
        })
    }
}
