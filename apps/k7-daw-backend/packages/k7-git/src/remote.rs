//! remote.rs — fetch, push and clone over the transports. What a reader gets wrong: libgit2 is blocking, so
//! the `*_blocking` calls belong on a blocking thread (the async ones put them there), and a push never forces:
//! a remote ahead of this branch is `Behind`, libgit2 finds it from the advertised refs before anything is sent.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use git2::build::CheckoutBuilder;
use git2::{Direction, FetchOptions, FetchPrune, PushOptions, RemoteCallbacks};
use url::Url;

use crate::lfs::Direct;
use crate::transport::{Op, Origin, scoped};
use crate::{Error, Net, Repo, Result};

const FETCH_SPEC: &str = "+refs/heads/*:refs/remotes/origin/*";

/// The address checked once, before libgit2 sees it: ssh only, and `file://` or a local path only when allowed.
fn vet(net: &Net, url: &str) -> Result<()> {
    if url.starts_with("ssh://") {
        return crate::transport::ssh_check(url);
    }
    if url.contains("://") {
        return match Url::parse(url) {
            Ok(u) if u.scheme() == "file" && net.allow_file => Ok(()),
            _ => Err(Error::SshOnly),
        };
    }
    // `user@host:path`; anything else is a local path.
    match url.split_once(':') {
        Some((host, _)) if !host.is_empty() && !host.contains('/') => {
            crate::transport::ssh_check(url)
        }
        _ if net.allow_file && Path::new(url).exists() => Ok(()),
        _ => Err(Error::SshOnly),
    }
}

/// `refs/heads/<branch>` of a connection's advertised default, when the remote has one.
fn default_branch(repo: &Repo, net: &Arc<Net>, origin: &Origin) -> Result<Option<String>> {
    let mut remote = repo.origin(origin)?;
    let mut cb = RemoteCallbacks::new();
    cb.credentials(|_, _, _| Err(git2::Error::from_str("no credential is offered by libgit2")));
    let _ = net;
    let conn = remote.connect_auth(Direction::Fetch, Some(cb), None)?;
    let head = conn.default_branch().ok();
    Ok(head.and_then(|h| {
        String::from_utf8_lossy(&h)
            .strip_prefix("refs/heads/")
            .map(str::to_string)
    }))
}

/// Runs a libgit2 call with the transports' context, and gives a transport's typed failure back in libgit2's place.
fn call<T>(
    net: &Arc<Net>,
    origin: &Origin,
    f: impl FnOnce() -> std::result::Result<T, Error>,
) -> Result<T> {
    vet(net, &origin.url)?;
    let op = Op {
        net: net.clone(),
        user: origin.user.clone(),
        failure: None,
    };
    let (out, op) = scoped(op, f);
    match (out, op.failure) {
        (_, Some(typed)) => Err(typed),
        (out, None) => out,
    }
}

/// A clone folder a crash left behind: dot-named, `.k7-clone-` and eight hex digits, untouched for an hour.
fn sweep_stale(parent: &Path) {
    let Ok(dir) = std::fs::read_dir(parent) else {
        return;
    };
    for e in dir.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let Some((head, tail)) = name.rsplit_once(".k7-clone-") else {
            continue;
        };
        let old = e.metadata().and_then(|m| m.modified()).is_ok_and(|t| {
            t.elapsed()
                .is_ok_and(|d| d > std::time::Duration::from_secs(3600))
        });
        if head.starts_with('.')
            && tail.len() == 8
            && tail.bytes().all(|b| b.is_ascii_hexdigit())
            && old
        {
            let _ = std::fs::remove_dir_all(e.path());
        }
    }
}

impl Repo {
    /// The remote `origin`, made or re-pointed at `origin.url`.
    pub(crate) fn origin(&self, origin: &Origin) -> Result<git2::Remote<'_>> {
        match self.git.find_remote("origin") {
            Ok(r) if r.url().is_ok_and(|u| u == origin.url) => Ok(r),
            Ok(_) => {
                self.git.remote_set_url("origin", &origin.url)?;
                Ok(self.git.find_remote("origin")?)
            }
            Err(_) => Ok(self.git.remote("origin", &origin.url)?),
        }
    }

    /// The remote this workspace is connected to and the user it signs in as, when it is connected.
    pub fn origin_config(&self) -> Option<Origin> {
        let url = self.git.find_remote("origin").ok()?.url().ok()?.to_string();
        let user = self
            .git
            .config()
            .ok()?
            .get_string("k7.user")
            .ok()
            .filter(|u| !u.is_empty());
        Some(Origin { url, user })
    }

    /// Connects the workspace to a remote: the address is vetted, `.gitattributes` carries k7's LFS block, no secret is written.
    pub fn set_origin(&self, net: &Net, origin: &Origin) -> Result<()> {
        vet(net, &origin.url)?;
        self.origin(origin)?;
        self.git
            .config()?
            .set_str("k7.user", origin.user.as_deref().unwrap_or(""))?;
        crate::lfs::attributes::ensure(&self.workdir)?;
        Ok(())
    }

    /// The commit the remote's branch stood at when last fetched.
    pub fn remote_tip(&self) -> Option<String> {
        let branch = self.branch()?;
        self.git
            .refname_to_id(&format!("refs/remotes/origin/{branch}"))
            .ok()
            .map(|o| o.to_string())
    }

    /// Updates the remote-tracking refs from the remote; nothing in the work tree moves.
    pub fn fetch_blocking(&self, net: &Arc<Net>, origin: &Origin) -> Result<()> {
        call(net, origin, || {
            let mut remote = self.origin(origin)?;
            let mut opts = FetchOptions::new();
            opts.prune(FetchPrune::On);
            remote.fetch(&[FETCH_SPEC], Some(&mut opts), None)?;
            Ok(())
        })
    }

    /// A fetch of `origin.url` through an anonymous remote: it proves the sign-in and brings the refs, and writes no `origin`.
    pub fn fetch_anonymous(&self, net: &Arc<Net>, origin: &Origin) -> Result<()> {
        call(net, origin, || {
            let mut remote = self.git.remote_anonymous(&origin.url)?;
            let mut opts = FetchOptions::new();
            opts.prune(FetchPrune::On);
            remote.fetch(&[FETCH_SPEC], Some(&mut opts), None)?;
            Ok(())
        })
    }

    /// Pushes the branch as a fast-forward; a remote ahead of it is `Behind`, never overwritten.
    pub fn push_blocking(&self, net: &Arc<Net>, origin: &Origin) -> Result<()> {
        let branch = self
            .branch()
            .ok_or_else(|| Error::Git("HEAD names no branch".into()))?;
        let local = self.git.refname_to_id(&format!("refs/heads/{branch}"))?;
        call(net, origin, || {
            let tip = self
                .git
                .refname_to_id(&format!("refs/remotes/origin/{branch}"))
                .ok();
            let owed = self.lfs_owed(local, tip)?;
            self.lfs_send(net, origin, &owed)?;
            let mut remote = self.origin(origin)?;
            let rejected: std::cell::RefCell<Option<String>> = std::cell::RefCell::new(None);
            let mut cb = RemoteCallbacks::new();
            cb.push_update_reference(|name, status| {
                if let Some(why) = status {
                    *rejected.borrow_mut() = Some(format!("{name}: {why}"));
                }
                Ok(())
            });
            let mut opts = PushOptions::new();
            opts.remote_callbacks(cb);
            let spec = format!("refs/heads/{branch}:refs/heads/{branch}");
            match remote.push(&[spec.as_str()], Some(&mut opts)) {
                Err(e) if e.code() == git2::ErrorCode::NotFastForward => return Err(Error::Behind),
                other => other?,
            }
            drop(opts);
            if let Some(why) = rejected.take() {
                let behind = why.contains("non-fast-forward") || why.contains("fetch first");
                return Err(if behind {
                    Error::Behind
                } else {
                    Error::Git(why)
                });
            }
            self.git.reference(
                &format!("refs/remotes/origin/{branch}"),
                local,
                true,
                "k7: push",
            )?;
            self.lfs_prune(&self.lfs_head()?);
            Ok(())
        })
    }

    /// A new workspace at `into`: made beside it under a dot name and renamed into place, so a failure leaves nothing.
    pub fn clone_blocking(net: &Arc<Net>, origin: &Origin, into: &Path) -> Result<Repo> {
        if into.exists() {
            return Err(Error::Exists(into.display().to_string()));
        }
        let parent = into
            .parent()
            .ok_or_else(|| Error::BadPath(into.display().to_string()))?;
        sweep_stale(parent);
        let name = into
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("workspace");
        let mut nonce = [0u8; 4];
        let _ = getrandom::fill(&mut nonce);
        let temp: PathBuf = parent.join(format!(
            ".{name}.k7-clone-{:08x}",
            u32::from_le_bytes(nonce)
        ));
        let made = (|| {
            let repo = Repo::init_clone(&temp)?;
            let branch = call(net, origin, || default_branch(&repo, net, origin))?;
            repo.fetch_blocking(net, origin)?;
            repo.check_out_default(branch.as_deref())?;
            let files = repo.lfs_head()?;
            repo.lfs_pull(net, origin, &files, &Direct(temp.clone()))?;
            repo.lfs_prune(&files);
            Ok::<_, Error>(())
        })();
        if let Err(e) = made {
            let _ = std::fs::remove_dir_all(&temp);
            return Err(e);
        }
        std::fs::rename(&temp, into).inspect_err(|_| {
            let _ = std::fs::remove_dir_all(&temp);
        })?;
        Repo::open(into, &into.join(".git"))
    }

    /// Makes the remote's default branch the local one and writes its files; no filter runs.
    fn check_out_default(&self, branch: Option<&str>) -> Result<()> {
        let Some(branch) = branch else { return Ok(()) };
        let tip = self
            .git
            .refname_to_id(&format!("refs/remotes/origin/{branch}"))?;
        let commit = self.git.find_commit(tip)?;
        self.git.branch(branch, &commit, true)?;
        self.git.set_head(&format!("refs/heads/{branch}"))?;
        let mut cfg = self.git.config()?;
        cfg.set_str(&format!("branch.{branch}.remote"), "origin")?;
        cfg.set_str(
            &format!("branch.{branch}.merge"),
            &format!("refs/heads/{branch}"),
        )?;
        let mut co = CheckoutBuilder::new();
        co.force().disable_filters(true);
        self.git.checkout_head(Some(&mut co))?;
        Ok(())
    }
}

impl Net {
    pub async fn fetch(
        self: &Arc<Net>,
        workdir: PathBuf,
        gitdir: PathBuf,
        origin: Origin,
    ) -> Result<()> {
        let net = self.clone();
        self.blocking(move || Repo::open(&workdir, &gitdir)?.fetch_blocking(&net, &origin))
            .await
    }

    pub async fn push(
        self: &Arc<Net>,
        workdir: PathBuf,
        gitdir: PathBuf,
        origin: Origin,
    ) -> Result<()> {
        let net = self.clone();
        self.blocking(move || Repo::open(&workdir, &gitdir)?.push_blocking(&net, &origin))
            .await
    }

    /// A clone into `into`; the repository is opened again by whoever needs it.
    pub async fn clone_into(self: &Arc<Net>, origin: Origin, into: PathBuf) -> Result<()> {
        let net = self.clone();
        self.blocking(move || Repo::clone_blocking(&net, &origin, &into).map(drop))
            .await
    }
}
