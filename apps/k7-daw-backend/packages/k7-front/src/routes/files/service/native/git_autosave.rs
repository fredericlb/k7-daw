//! git_autosave.rs — one round of auto-save on a connected workspace (`docs/files/git.md`, "Auto-save"): a snapshot of
//! the work tree on the person's technical branch, the other devices' snapshots taken in, the branch pushed.
//! What a reader gets wrong: the workspace's lock is held only for the local steps and the remote's for the network,
//! so a Save never waits on a slow push; a round that meets either lock taken skips and the task tries again.

use std::path::Path;

use k7_git::autosave::{Owner, Picked, Sent, branch_name};
use k7_git::{Error, Repo};

use super::git::{Ctx, open};
use super::git_forge::saved;
use super::git_key_retry::account_for;
use super::git_lock;
use super::git_sync::{Through, who_am_i};

/// The person's technical branch on this workspace: the forge account's login, else the commit email.
pub(super) fn branch_for(ctx: &Ctx, repo: &Repo) -> Option<String> {
    let origin = repo.origin_config()?;
    if let Some(s) = account_for(ctx, &origin.url).and_then(|f| saved(ctx, &f)) {
        return Some(branch_name(Owner::Forge(&s.login)));
    }
    let id = who_am_i(ctx).ok()?;
    Some(branch_name(Owner::Email(&id.email)))
}

/// How long the state's answer reuses a branch name: a state poll must not read the keychain each time.
const NAME_FOR: std::time::Duration = std::time::Duration::from_secs(60);

type Names = std::sync::Mutex<
    std::collections::HashMap<std::path::PathBuf, (std::time::Instant, Option<String>)>,
>;

/// `branch_for`, remembered per workspace for `NAME_FOR`.
pub(super) fn branch_named(ctx: &Ctx, repo: &Repo) -> Option<String> {
    static NAMES: std::sync::OnceLock<Names> = std::sync::OnceLock::new();
    let names = NAMES.get_or_init(Default::default);
    let key = repo.workdir().to_path_buf();
    let mut all = names.lock().unwrap_or_else(|p| p.into_inner());
    if let Some((at, name)) = all.get(&key)
        && at.elapsed() < NAME_FOR
    {
        return name.clone();
    }
    let name = branch_for(ctx, repo);
    all.insert(key, (std::time::Instant::now(), name.clone()));
    name
}

/// What a round came to; `unsent` when the branch is still not on the remote.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Round {
    pub snapshot: bool,
    pub landed: usize,
    pub unsent: bool,
    pub skipped: bool,
}

/// One round: `snapshot` takes the work tree first; `fetched` skips the fetch the caller just made.
pub async fn round(ctx: &Ctx, root: &Path, snapshot: bool, fetched: bool) -> Result<Round, Error> {
    super::git_author::ensure(ctx).await;
    let key = root.display().to_string();
    let mut out = Round::default();
    let (ctx2, rootp) = (ctx.clone(), root.to_path_buf());
    let Some(branch) = ctx
        .net
        .blocking(move || Ok(branch_for(&ctx2, &open(&rootp)?)))
        .await?
    else {
        return Ok(out);
    };
    if snapshot {
        let Some(_held) = git_lock::try_take(&key) else {
            out.skipped = true;
            return Ok(out);
        };
        let (ctx2, rootp, b) = (ctx.clone(), root.to_path_buf(), branch.clone());
        let id = ctx
            .net
            .blocking(move || open(&rootp)?.autosave_commit(&b, &who_am_i(&ctx2)?, &ctx2.device))
            .await?;
        out.snapshot = id.is_some();
    }
    let Some(_remote) = git_lock::try_take_remote_as_autosave(&key) else {
        out.skipped = true;
        return Ok(out);
    };
    let origin = open(root)?
        .origin_config()
        .ok_or_else(|| Error::BadUrl("this workspace has no remote".into()))?;
    let mut fetch = !fetched;
    for _ in 0..3 {
        if fetch {
            ctx.net
                .fetch(root.to_path_buf(), root.join(".git"), origin.clone())
                .await?;
        }
        fetch = true;
        match take_in(ctx, root, &branch).await? {
            Some(Picked::Landed { files, .. }) => out.landed += files,
            Some(_) => {}
            None => {
                out.skipped = true;
                break;
            }
        }
        let (net, rootp, o, b) = (
            ctx.net.clone(),
            root.to_path_buf(),
            origin.clone(),
            branch.clone(),
        );
        let sent = ctx
            .net
            .blocking(move || open(&rootp)?.autosave_push_blocking(&net, &o, &b))
            .await?;
        if sent != Sent::Behind {
            break;
        }
    }
    let rootp = root.to_path_buf();
    out.unsent = ctx
        .net
        .blocking(move || Ok(open(&rootp)?.autosave_info(&branch).is_some_and(|(_, u)| u)))
        .await?;
    Ok(out)
}

/// The other devices' snapshots into the work tree, under the workspace's lock; none when the lock is taken.
async fn take_in(ctx: &Ctx, root: &Path, branch: &str) -> Result<Option<Picked>, Error> {
    let key = root.display().to_string();
    let Some(_held) = git_lock::try_take(&key) else {
        return Ok(None);
    };
    let through = Through::new(ctx, root);
    let origin = open(root)?.origin_config();
    let (ctx2, rootp, b) = (ctx.clone(), root.to_path_buf(), branch.to_string());
    let picked = ctx
        .net
        .blocking(move || {
            let repo = open(&rootp)?;
            let net = ctx2.net.clone();
            let fetch = |files: &[(String, k7_git::Pointer)]| match &origin {
                Some(o) => repo.lfs_fetch_objects(&net, o, files),
                None => Ok(()),
            };
            repo.autosave_integrate(&b, &who_am_i(&ctx2)?, &ctx2.device, &through, &fetch)
        })
        .await?;
    if let (Picked::Landed { files, .. }, Some(account)) = (&picked, &ctx.account)
        && *files > 0
    {
        account
            .record_landing(&key, "auto-save of another device")
            .await;
    }
    Ok(Some(picked))
}

/// A Save committed on the main branch: the technical branch restarts from it (pushed by the next round).
pub(super) fn after_save(ctx: &Ctx, repo: &Repo) {
    let Some(branch) = branch_for(ctx, repo) else {
        return;
    };
    if let Err(e) = repo.autosave_reset(&branch) {
        eprintln!("k7-front: the auto-save branch was not reset after a save: {e}");
    }
}
