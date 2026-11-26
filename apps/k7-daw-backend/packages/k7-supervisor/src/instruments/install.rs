//! install.rs — one instrument fetched, verified and unpacked. The archive is hashed whole before it is
//! unpacked, as it would be after one uninterrupted download, so a resumed prefix is never trusted beyond what
//! the hash proves: a mismatch deletes it, and a verified archive that cannot be unpacked is KEPT, so the next
//! try fetches nothing. The total of a progress is the PINNED size: GitHub serves the piano with no length.

use std::path::Path;
use std::time::Duration;

use super::store::MARKER;
use super::{Instrument, sha256_of, unpack_zip};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Phase {
    Downloading,
    Unpacking,
}

/// Where a download stands.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Progress {
    pub received: u64,
    pub total: u64,
    pub phase: Phase,
}

/// Fetch, verify and unpack `i` into `root`; `report` runs as bytes arrive. `timeout` bounds each wait of the transfer.
pub async fn install(
    root: &Path,
    i: &Instrument,
    timeout: Duration,
    mut report: impl FnMut(Progress) + Send,
) -> Result<(), String> {
    let (root, i) = (root.to_path_buf(), i.clone());
    std::fs::create_dir_all(&root).map_err(|e| format!("cannot create {}: {e}", root.display()))?;
    let part = root.join(format!("{}.zip.part", i.name));
    let folder = root.join(&i.name);
    // A `.part` left by a cut download is resumed rather than fetched again.
    let resumed = part.is_file();
    let total = i.bytes;
    let tls = crate::trust::client_config();
    // Kept on failure: what arrived is a prefix the next try resumes from.
    crate::download::fetch(tls, &i.url, &part, timeout, |received| {
        report(Progress {
            received: received.min(total),
            total,
            phase: Phase::Downloading,
        });
    })
    .await?;
    let archive = part.clone();
    let hash = tokio::task::spawn_blocking(move || sha256_of(&archive))
        .await
        .map_err(|e| e.to_string())?;
    if hash != i.sha256 {
        let _ = std::fs::remove_file(&part);
        return Err(if resumed {
            format!(
                "the resumed archive does not match its pin (sha256 {hash}); the next try downloads it whole"
            )
        } else {
            format!("the archive changed upstream (sha256 {hash}), refused")
        });
    }
    report(Progress {
        received: total,
        total,
        phase: Phase::Unpacking,
    });
    tokio::task::spawn_blocking(move || -> Result<(), String> {
        unpack_zip(&part, &folder)?;
        let _ = std::fs::remove_file(&part);
        std::fs::write(folder.join(MARKER), format!("{}\n", i.sha256)).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}
