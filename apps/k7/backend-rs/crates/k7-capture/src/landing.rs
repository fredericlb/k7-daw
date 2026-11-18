//! landing.rs — how a file a pull fetched reaches the disk: through the workspace's capture when it
//! has one, so its ledger learns the file before the capture can see it as an edit, and straight to the disk when it has none.
//! What a reader gets wrong: the capture's worker does the write, so the ledger entry and the file never disagree for a sweep.

use std::future::Future;
use std::io;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;

use hyper::body::Bytes;

/// `None` when no capture holds the root: the caller writes the disk itself.
pub type LandFuture = Pin<Box<dyn Future<Output = Option<io::Result<()>>> + Send>>;

/// (root, path, bytes or `None` to remove) handed to the workspace's capture.
pub type Land = Arc<dyn Fn(String, String, Option<Bytes>) -> LandFuture + Send + Sync>;

pub async fn land(
    through: Option<&Land>,
    root: &Path,
    rel: &str,
    bytes: Option<Bytes>,
) -> io::Result<()> {
    let back = bytes.clone();
    if let Some(l) = through
        && let Some(done) = l(root.display().to_string(), rel.to_string(), bytes).await
    {
        return done;
    }
    let abs = root.join(rel);
    match back {
        None => match tokio::fs::remove_file(&abs).await {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        },
        Some(b) => {
            if let Some(dir) = abs.parent() {
                tokio::fs::create_dir_all(dir).await?;
            }
            let name = abs
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let part = abs.with_file_name(format!(".{name}.k7-part"));
            tokio::fs::write(&part, &b).await?;
            tokio::fs::rename(&part, &abs).await
        }
    }
}
