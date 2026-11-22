//! lfs — k7 is its own git LFS client: large files are a pointer in git and the bytes in a store beside the git folder.
//! What a reader gets wrong: libgit2 never filters, so the clean step is k7's commit and the smudge step is k7's
//! checkout; a file is judged changed by its hash, never by libgit2's comparison of its bytes with a pointer.

pub mod attributes;
mod batch;
pub mod pointer;
pub(crate) mod ssh;
pub mod store;
pub(crate) mod sync;

use std::path::PathBuf;
use std::sync::Arc;

use crate::transport::Origin;
use crate::{Net, Result};
pub use attributes::Attributes;
pub use pointer::Pointer;
pub use store::Store;
pub use sync::{Direct, Landing};

/// What a host does with large files.
pub(crate) trait Transfer {
    /// Sends the objects the server lacks, then verifies them when it asks.
    fn upload(&self, store: &Store, objs: &[Pointer]) -> Result<()>;
    /// Brings the objects in, each checked by its hash.
    fn download(&self, store: &Store, objs: &[Pointer]) -> Result<()>;
    /// Whether the host speaks the batch API at all.
    fn probe(&self) -> Result<()>;
}

/// A `file://` remote keeps its objects in its own `lfs/objects`, as git-lfs does for a local endpoint.
struct Local {
    remote: Store,
}

impl Transfer for Local {
    fn upload(&self, store: &Store, objs: &[Pointer]) -> Result<()> {
        for o in objs {
            if !self.remote.has(&o.oid) {
                self.remote.put_file(&store.path(&o.oid))?;
            }
        }
        Ok(())
    }

    fn download(&self, store: &Store, objs: &[Pointer]) -> Result<()> {
        for o in objs {
            if !self.remote.has(&o.oid) {
                return Err(crate::Error::Network(format!(
                    "the remote has no object {}",
                    o.oid
                )));
            }
            if !store.has(&o.oid) {
                store.put_file(&self.remote.path(&o.oid))?;
            }
        }
        Ok(())
    }

    fn probe(&self) -> Result<()> {
        Ok(())
    }
}

/// The remote's git folder named by a `file://` address or a path.
fn local_gitdir(url: &str) -> PathBuf {
    let p = crate::transport::file_path(url);
    if p.join("objects").is_dir() {
        p
    } else {
        p.join(".git")
    }
}

pub(crate) fn connect(net: &Arc<Net>, origin: &Origin) -> Result<Box<dyn Transfer>> {
    let url = origin.url.as_str();
    if url.starts_with("ssh://") || crate::transport::scp_like(url) {
        crate::transport::ssh::parse(url)?;
        return Ok(Box::new(batch::Http::new(
            net.clone(),
            origin.user.clone(),
            url.to_string(),
        )));
    }
    if url.starts_with("file://") || !url.contains("://") {
        return Ok(Box::new(Local {
            remote: Store::in_gitdir(&local_gitdir(url)),
        }));
    }
    Err(crate::Error::SshOnly)
}
