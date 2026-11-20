//! identity.rs — who a commit is by: a name and an email, kept per device in a file beside the account's
//! settings. What a reader gets wrong: it is the device's, not the
//! account's synced preferences, so two machines of one person may sign differently.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

pub const IDENTITY_FILE: &str = ".k7-git-identity.json";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    pub name: String,
    pub email: String,
}

impl Identity {
    pub fn is_complete(&self) -> bool {
        !self.name.trim().is_empty() && !self.email.trim().is_empty()
    }
}

/// The identity file of one device.
#[derive(Clone)]
pub struct IdentityStore {
    path: PathBuf,
}

impl IdentityStore {
    /// `<folder>/.k7-git-identity.json`, the folder the account's settings live in.
    pub fn in_folder(folder: &Path) -> IdentityStore {
        IdentityStore {
            path: folder.join(IDENTITY_FILE),
        }
    }

    /// A missing file is no identity; an unreadable one is an error, never "nobody".
    pub fn load(&self) -> Result<Option<Identity>> {
        match std::fs::read(&self.path) {
            Ok(b) => serde_json::from_slice(&b)
                .map(Some)
                .map_err(|e| Error::Io(format!("{IDENTITY_FILE}: {e}"))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn save(&self, id: &Identity) -> Result<()> {
        let temp = self.path.with_extension("json.tmp");
        let text = serde_json::to_vec_pretty(id).map_err(|e| Error::Io(e.to_string()))?;
        std::fs::write(&temp, text)?;
        std::fs::rename(&temp, &self.path)?;
        Ok(())
    }
}
