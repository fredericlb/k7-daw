//! The large-file rig: an SSH server for git, an HTTP server for the LFS endpoint its `git-lfs-authenticate` grants.
//! What a reader gets wrong: the device's key is made by the rig and authorized on the SSH server, as a person does.

use std::sync::Arc;

use k7_git::auth::KnownHosts;
use k7_git::{Net, Origin};

use super::httpd::Httpd;
use super::ssh_keys::HOST_A;
use super::sshd::{Sshd, fingerprint};
use super::{Counting, Dir, authorize_device, bare};

pub struct SshLfs {
    pub root: Dir,
    pub sshd: Sshd,
    pub web: Httpd,
    pub store: Arc<Counting>,
    pub net: Arc<Net>,
}

pub const GRANT: &str = "Bearer granted";

/// A rig with `r.git`; `grant` false makes the SSH host a host without LFS (it knows no `git-lfs-authenticate`).
pub async fn ssh_lfs(tag: &str, grant: bool) -> SshLfs {
    let root = Dir::new(tag);
    bare(&root.path().join("r.git"));
    let sshd = Sshd::start(root.path().to_path_buf()).await;
    let web = Httpd::start().await;
    if grant {
        *web.lfs.require_auth.lock().unwrap() = Some(GRANT.into());
        *sshd.lfs_grant.lock().unwrap() = Some((web.lfs_url("r.git"), GRANT.into()));
    }
    let store = Arc::new(Counting::default());
    authorize_device(&sshd, &store);
    let hosts = Arc::new(KnownHosts::memory());
    hosts
        .remember("127.0.0.1", sshd.port(), &fingerprint(HOST_A))
        .unwrap();
    let net = Net::new(
        tokio::runtime::Handle::current(),
        super::tls(),
        store.clone(),
        hosts,
    );
    SshLfs {
        root,
        sshd,
        web,
        store,
        net,
    }
}

impl SshLfs {
    pub fn to(&self) -> Origin {
        Origin {
            url: self.sshd.url("r.git"),
            user: None,
        }
    }

    /// Whether the push reached `git-receive-pack`, which the ref moves through.
    pub fn ref_moved(&self) -> bool {
        self.sshd
            .commands
            .lock()
            .unwrap()
            .iter()
            .any(|c| c.starts_with("git-receive-pack"))
    }
}
