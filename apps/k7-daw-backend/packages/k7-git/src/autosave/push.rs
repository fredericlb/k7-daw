//! push.rs — sending the technical branch: a fast-forward, or the forced push of a Save's reset under its lease.
//! What a reader gets wrong: the caller fetches first; a lease the remote moved past is dropped and answered `Behind`,
//! so the caller takes the other device's snapshots in before anything is replaced.

use std::sync::Arc;

use crate::{Error, Net, Origin, Repo, Result};

/// What a push of the technical branch came to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sent {
    /// The remote already holds it, or there is nothing here.
    Nothing,
    Pushed,
    /// The remote holds snapshots this device has not taken in: integrate, then push again.
    Behind,
}

impl Repo {
    pub fn autosave_push_blocking(
        &self,
        net: &Arc<Net>,
        origin: &Origin,
        branch: &str,
    ) -> Result<Sent> {
        let (tip, remote) = self.autosave_tips(branch);
        let Some(tip) = tip else {
            return Ok(Sent::Nothing);
        };
        if let Some(expected) = self.lease_of(branch) {
            if remote == expected || remote == Some(tip) {
                if remote != Some(tip) {
                    self.push_branch(net, origin, branch, true)?;
                }
                self.drop_lease();
                return Ok(Sent::Pushed);
            }
            self.drop_lease();
            return Ok(Sent::Behind);
        }
        match remote {
            Some(r) if r == tip => return Ok(Sent::Nothing),
            Some(r) if !self.has_in_history(tip, r) => return Ok(Sent::Behind),
            _ => {}
        }
        match self.push_branch(net, origin, branch, false) {
            Ok(()) => Ok(Sent::Pushed),
            Err(Error::Behind) => Ok(Sent::Behind),
            Err(e) => Err(e),
        }
    }
}
