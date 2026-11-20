//! transport — k7's own smart transport for libgit2 (`ssh`), the hyper client LFS uses, and the context they read.
//! What a reader gets wrong: libgit2 calls a subtransport on the thread that called it, so the context of one
//! operation (the credential store, the runtime, the typed failure) travels in a thread-local, set around that call.

use std::cell::RefCell;
use std::sync::{Arc, Once, OnceLock};

use tokio::runtime::Handle;

use crate::Error;
use crate::auth::{KnownHosts, Secrets};

pub(crate) mod http;
pub(crate) mod idle;
pub(crate) mod ssh;

/// Whether `url` is an address a server may name for a large-file transfer.
pub(crate) fn http_check(url: &url::Url) -> crate::Result<()> {
    http::check(url)
}

/// Whether `url` is an ssh address the ssh subtransport takes.
pub(crate) fn ssh_check(url: &str) -> crate::Result<()> {
    ssh::parse(url).map(drop)
}

/// The default bound on a read or write that moves no byte: a stalled server must not hold a call forever.
pub const IDLE: std::time::Duration = std::time::Duration::from_secs(60);

/// What a connection needs of the host: the runtime to drive, the TLS roots, the secrets and the trusted servers.
pub struct Net {
    pub(crate) handle: Handle,
    pub(crate) tls: Arc<rustls::ClientConfig>,
    pub(crate) secrets: Arc<dyn Secrets>,
    pub(crate) hosts: Arc<KnownHosts>,
    pub(crate) allow_file: bool,
    pub(crate) http: OnceLock<http::Http>,
    /// How long a read or write may move no byte before the call ends.
    pub(crate) idle: std::time::Duration,
    /// `git-lfs-authenticate` answers, kept until they expire.
    pub(crate) lfs_auth:
        std::sync::Mutex<std::collections::HashMap<String, crate::lfs::ssh::Granted>>,
}

impl Net {
    /// `tls` is the process's roots (Mozilla's plus `K7_EXTRA_CA_CERTS`); `file://` is accepted when `K7_GIT_ALLOW_FILE` is set.
    pub fn new(
        handle: Handle,
        tls: Arc<rustls::ClientConfig>,
        secrets: Arc<dyn Secrets>,
        hosts: Arc<KnownHosts>,
    ) -> Arc<Net> {
        register();
        let allow_file = std::env::var_os("K7_GIT_ALLOW_FILE").is_some_and(|v| !v.is_empty());
        Arc::new(Net {
            handle,
            tls,
            secrets,
            hosts,
            allow_file,
            http: OnceLock::new(),
            idle: IDLE,
            lfs_auth: Default::default(),
        })
    }

    fn rebuilt(self: Arc<Net>, allow_file: bool, idle: std::time::Duration) -> Arc<Net> {
        Arc::new(Net {
            handle: self.handle.clone(),
            tls: self.tls.clone(),
            secrets: self.secrets.clone(),
            hosts: self.hosts.clone(),
            allow_file,
            http: OnceLock::new(),
            idle,
            lfs_auth: Default::default(),
        })
    }

    /// The same, with `file://` allowed or refused whatever the environment says.
    pub fn with_file(self: Arc<Net>, allow: bool) -> Arc<Net> {
        let idle = self.idle;
        self.rebuilt(allow, idle)
    }

    /// The same, with another bound on a transfer that moves no byte (the tests' short one).
    pub fn with_idle(self: Arc<Net>, idle: std::time::Duration) -> Arc<Net> {
        let allow = self.allow_file;
        self.rebuilt(allow, idle)
    }

    /// The trusted servers, for the dialog that adds one.
    pub fn hosts(&self) -> &Arc<KnownHosts> {
        &self.hosts
    }

    /// A libgit2 call on a blocking thread: the front's runtime is never held by it.
    pub async fn blocking<T: Send + 'static>(
        &self,
        f: impl FnOnce() -> crate::Result<T> + Send + 'static,
    ) -> crate::Result<T> {
        self.handle
            .spawn_blocking(f)
            .await
            .map_err(|e| Error::Io(format!("the git worker stopped: {e}")))?
    }
}

/// The remote a call reaches and the user it signs in as.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Origin {
    pub url: String,
    /// The account name; the address's own when none is given (`ssh`: `git` when neither).
    pub user: Option<String>,
}

/// What one libgit2 call carries to the subtransports and brings back.
pub(crate) struct Op {
    pub net: Arc<Net>,
    pub user: Option<String>,
    pub failure: Option<Error>,
}

thread_local! {
    static OP: RefCell<Option<Op>> = const { RefCell::new(None) };
}

/// Runs `f` with `op` as this thread's context; the context comes back with whatever failure a transport left in it.
pub(crate) fn scoped<T>(op: Op, f: impl FnOnce() -> T) -> (T, Op) {
    let before = OP.with(|c| c.replace(Some(op)));
    let out = f();
    let op = OP.with(|c| c.replace(before)).expect("the scope set it");
    (out, op)
}

/// The current operation's context, or the error a transport answers without one.
pub(crate) fn with_op<R>(f: impl FnOnce(&mut Op) -> R) -> Result<R, git2::Error> {
    OP.with(|c| match c.borrow_mut().as_mut() {
        Some(op) => Ok(f(op)),
        None => Err(git2::Error::from_str(
            "a git transport was used outside k7's calls",
        )),
    })
}

/// Records a typed failure and answers libgit2 with its sentence.
pub(crate) fn fail(e: Error) -> git2::Error {
    let sentence = e.to_string();
    let _ = with_op(|op| op.failure = Some(e));
    git2::Error::from_str(&sentence)
}

/// Registers k7's transports with libgit2 and empties its config search paths; the front calls it once at start.
pub fn init() {
    register();
}

fn register() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        // SAFETY: registered once, before any libgit2 call of ours; the factories hold no state.
        unsafe {
            // The person's own gitconfig (insteadOf, pushurl) must not rewrite an address after k7 vetted it.
            for level in [
                git2::ConfigLevel::ProgramData,
                git2::ConfigLevel::System,
                git2::ConfigLevel::XDG,
                git2::ConfigLevel::Global,
            ] {
                let _ = git2::opts::set_search_path(level, "");
            }
            git2::transport::register("ssh", |r| {
                git2::transport::Transport::smart(r, false, ssh::SshSub::new())
            })
            .expect("the ssh transport registers");
        }
    });
}

/// The host and explicit port an ssh address names, as trusted servers are keyed: none for any other address.
pub fn remote_host(url: &str) -> Option<(String, Option<u16>)> {
    if url.starts_with("ssh://") || (!url.contains("://") && url.contains(':')) {
        let a = ssh::parse(url).ok()?;
        return Some((a.host, a.explicit_port.then_some(a.port)));
    }
    None
}
