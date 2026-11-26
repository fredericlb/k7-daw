//! git_task.rs — the task that fetches for a workspace the page told us about: it looks at the cadence and fetches when it
//! is due, and ends when the workspace has no remote or is gone. What a reader gets wrong: one task per root, ever.

use std::collections::HashSet;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use super::git::Ctx;
use super::git_fetch::{Cadence, LOOK, now};
use super::git_lock;

fn running() -> &'static Mutex<HashSet<String>> {
    static R: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    R.get_or_init(Default::default)
}

/// Starts the workspace's task unless one runs.
pub fn ensure(ctx: &Ctx, root: &Path, cadence: Arc<Mutex<Cadence>>) {
    let key = root.display().to_string();
    if !running()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .insert(key.clone())
    {
        return;
    }
    let (net, root, ctx) = (ctx.net.clone(), root.to_path_buf(), ctx.clone());
    let wake = cadence
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .wake
        .clone();
    tokio::spawn(async move {
        loop {
            // A wanted fetch wakes the task at once; otherwise it looks every LOOK.
            tokio::select! {
                () = tokio::time::sleep(LOOK) => {}
                () = wake.notified() => {}
            }
            let due = cadence.lock().unwrap_or_else(|p| p.into_inner()).due(now());
            if !root.is_dir() {
                break;
            }
            if !due {
                continue;
            }
            let turned = turn(&net, &root, &cadence).await;
            if turned == Turn::NoRemote {
                break;
            }
            let fetched = turned == Turn::Fetched
                && cadence
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .last
                    .as_ref()
                    .is_some_and(|o| o.error.is_none());
            // A fetch at open, at a return to the front or on the cadence brings the other devices' auto-saves.
            if fetched {
                super::git_autosave_task::after_fetch(&ctx, &root).await;
            }
        }
        running()
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&key);
    });
}

/// How one turn of the task ended.
#[derive(Debug, PartialEq, Eq)]
pub enum Turn {
    Fetched,
    /// A pull or a push holds the remote: this turn is skipped, with no error and no backoff.
    Skipped,
    NoRemote,
}

/// One due fetch of the workspace, unless a pull or a push holds the remote's lock; a commit goes on beside it.
pub async fn turn(net: &Arc<k7_git::Net>, root: &Path, cadence: &Arc<Mutex<Cadence>>) -> Turn {
    let origin = match k7_git::Repo::open(root, &root.join(".git"))
        .ok()
        .and_then(|r| r.origin_config())
    {
        Some(o) => o,
        None => return Turn::NoRemote,
    };
    let Some(_held) = git_lock::try_take_remote(&root.display().to_string()) else {
        return Turn::Skipped;
    };
    let result = net
        .fetch(root.to_path_buf(), root.join(".git"), origin)
        .await;
    let said = cadence.lock().unwrap_or_else(|p| p.into_inner()).done(
        now(),
        result.map_err(|e| (e.code().to_string(), e.to_string())),
    );
    if let Some(why) = said {
        eprintln!("k7-front: the fetch of {} failed: {why}", root.display());
    }
    Turn::Fetched
}
