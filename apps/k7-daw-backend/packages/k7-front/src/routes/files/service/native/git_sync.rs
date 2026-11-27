//! git_sync.rs — commit, push, pull and the merge's own routes (`docs/files/git.md`): the identity a commit is signed with,
//! the landing of a pull through the capture, and the choices of a waiting merge. What a reader gets wrong: a commit or a
//! pull with a message is refused while a merge waits, since either would move `HEAD` under it.
use std::path::{Path, PathBuf};
use std::sync::Arc;

use git_rules::PullState;
use k7_capture::landing::Land;
use k7_git::{Begin, Choice, Error, Identity};
use serde_json::{Value, json};

use super::git_history;
use super::git_key_retry::retrying;
use crate::routes::files::json_answer;

use super::git::{Answer, Ctx, bad, field, merge_waits, open, run};

/// The device's identity as a commit uses it: what was typed, each empty field filled from a source (`git_author.rs`).
/// A pull's answer: it worked, and how it ended.
fn pulled(state: PullState) -> Value {
    json!({ "ok": true, "state": state })
}

pub(super) fn prefilled(ctx: &Ctx) -> Result<Identity, Error> {
    Ok(super::git_author::resolve(ctx)?.id)
}

pub(super) fn who_am_i(ctx: &Ctx) -> Result<Identity, Error> {
    let id = prefilled(ctx)?;
    if id.is_complete() {
        Ok(id)
    } else {
        Err(Error::NoIdentity)
    }
}

pub(super) async fn commit(ctx: &Ctx, root: &Path, b: &Value) -> Answer {
    if let Some(r) = merge_waits(root) {
        return Ok(r);
    }
    super::git_author::ensure(ctx).await;
    let (id, device) = (who_am_i(ctx)?, ctx.device.clone());
    let message = field(b, "message")
        .ok_or_else(|| Error::BadPath("a message is required".into()))?
        .to_string();
    let (m, by) = (message.clone(), id.name.clone());
    let exclude: Vec<String> = b
        .get("exclude")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let ctx2 = ctx.clone();
    let done = run(ctx, root, move |r| {
        let made = r.commit_excluding(&message, &id, &device, &exclude)?;
        super::git_autosave::after_save(&ctx2, r);
        Ok(json!({ "ok": true, "id": made }))
    })
    .await?;
    git_history::committed(ctx, &root.display().to_string(), &m, &by).await;
    Ok(done)
}

pub(super) async fn push(ctx: &Ctx, root: &Path) -> Answer {
    let origin = open(root)?
        .origin_config()
        .ok_or_else(|| Error::BadUrl("this workspace has no remote".into()))?;
    let (dir, git) = (root.to_path_buf(), root.join(".git"));
    let pushed = retrying(ctx, &origin.url, || {
        ctx.net.push(dir.clone(), git.clone(), origin.clone())
    })
    .await;
    match pushed {
        // A refused push fetches, so the state read next says "to pull" and the bar offers Pull.
        Err(Error::Behind) => {
            let _ = retrying(ctx, &origin.url, || {
                ctx.net.fetch(dir.clone(), git.clone(), origin.clone())
            })
            .await;
            Err(Error::Behind)
        }
        r => r.map(|()| {
            super::git_autosave_task::push_now(ctx, root);
            json_answer(200, &json!({ "ok": true }))
        }),
    }
}

/// Everything a pull needs from the capture: the files land through it, so its ledger learns each one first.
#[derive(Clone)]
pub(super) struct Through {
    land: Option<Land>,
    root: PathBuf,
    handle: tokio::runtime::Handle,
}

impl Through {
    pub(super) fn new(ctx: &Ctx, root: &Path) -> Through {
        Through {
            land: ctx.land.clone(),
            root: root.to_path_buf(),
            handle: tokio::runtime::Handle::current(),
        }
    }
}

/// A file this size or larger is written straight to the disk: the capture's landing takes bytes held in memory.
const LAND_LIMIT: u64 = 64 << 20;

impl k7_git::Landing for Through {
    fn land(&self, rel: &str, from: &Path) -> std::io::Result<()> {
        let direct = k7_git::Direct(self.root.clone());
        let small = std::fs::metadata(from)?.len() < LAND_LIMIT;
        match (&self.land, small) {
            (Some(l), true) => {
                let bytes = hyper::body::Bytes::from(std::fs::read(from)?);
                self.handle.block_on(k7_capture::landing::land(
                    Some(l),
                    &self.root,
                    rel,
                    Some(bytes),
                ))
            }
            _ => direct.land(rel, from),
        }
    }

    fn remove(&self, rel: &str) -> std::io::Result<()> {
        match &self.land {
            Some(l) => {
                self.handle
                    .block_on(k7_capture::landing::land(Some(l), &self.root, rel, None))
            }
            None => k7_git::Direct(self.root.clone()).remove(rel),
        }
    }
}

pub(super) async fn pull(ctx: &Ctx, root: &Path, b: &Value) -> Answer {
    if field(b, "message").is_some()
        && let Some(r) = merge_waits(root)
    {
        return Ok(r);
    }
    let origin = open(root)?
        .origin_config()
        .ok_or_else(|| Error::BadUrl("this workspace has no remote".into()))?;
    let (id, device) = (
        Some(prefilled(ctx)?).filter(Identity::is_complete),
        ctx.device.clone(),
    );
    let message = field(b, "message").map(str::to_string);
    let through = Through::new(ctx, root);
    let (net, rootp) = (ctx.net.clone(), root.to_path_buf());
    let committed = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (said, by) = (committed.clone(), id.as_ref().map(|i| i.name.clone()));
    let out = retrying(ctx, &origin.url, || {
        let (net, rootp, origin) = (net.clone(), rootp.clone(), origin.clone());
        let (message, id, device) = (message.clone(), id.clone(), device.clone());
        let (through, said) = (through.clone(), said.clone());
        ctx.net.blocking(move || {
            let repo = open(&rootp)?;
            if repo.status()?.iter().any(|e| !e.too_large) {
                let (Some(m), Some(i)) = (&message, &id) else {
                    return Err(Error::Uncommitted);
                };
                repo.commit_excluding(m, i, &device, &repo.too_large()?)?;
                said.store(true, std::sync::atomic::Ordering::SeqCst);
            }
            repo.fetch_blocking(&net, &origin)?;
            let Some(theirs) = repo.remote_tip() else {
                return Ok(pulled(PullState::EmptyRemote));
            };
            let began = repo
                .merge_begin_fetching(&theirs, |lfs| repo.lfs_fetch_objects(&net, &origin, lfs))?;
            match began {
                Begin::UpToDate => Ok(pulled(PullState::UpToDate)),
                Begin::FastForward { theirs, head } => {
                    let wanted = repo.lfs_wanted_by(&theirs)?;
                    repo.lfs_fetch_objects(&net, &origin, &wanted)?;
                    repo.merge_fast_forward(&theirs, head.as_deref(), &through)?;
                    repo.lfs_prune_held()?;
                    Ok(pulled(PullState::FastForward))
                }
                Begin::Merge(s) => {
                    if s.conflicts.is_empty() {
                        let me = id.ok_or(Error::NoIdentity)?;
                        repo.merge_resolve(&[], &through, &me, &device)?;
                        repo.lfs_prune_held()?;
                        return Ok(pulled(PullState::Merged));
                    }
                    Ok(json!({ "ok": true, "state": PullState::Conflicts, "conflicts": s.conflicts }))
                }
            }
        })
    })
    .await;
    let at = root.display().to_string();
    if committed.load(std::sync::atomic::Ordering::SeqCst)
        && let Some(m) = field(b, "message")
    {
        git_history::committed(ctx, &at, m, by.as_deref().unwrap_or("")).await;
    }
    let out = out?;
    if matches!(out["state"].as_str(), Some("fast-forward" | "merged"))
        && let Some(tip) = open(root)?.remote_tip()
    {
        git_history::landed(ctx, &at, &tip).await;
    }
    Ok(json_answer(200, &out))
}

pub(super) async fn merge_view(ctx: &Ctx, root: &Path) -> Answer {
    run(ctx, root, |r| {
        Ok(match r.merge_pending()? {
            Some(p) => json!({ "pending": true, "theirs": p.theirs, "conflicts": p.conflicts }),
            None => json!({ "pending": false }),
        })
    })
    .await
}

fn choice_of(s: &str) -> Option<Choice> {
    Some(match s {
        "mine" => Choice::Mine,
        "theirs" => Choice::Theirs,
        "both" => Choice::Both,
        "keep" => Choice::Keep,
        "remove" => Choice::Remove,
        _ => return None,
    })
}

pub(super) async fn resolve_merge(ctx: &Ctx, root: &Path, b: &Value) -> Answer {
    let mut picks = Vec::new();
    for c in b
        .get("choices")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let (Some(id), Some(ch)) = (field(c, "id"), field(c, "choice").and_then(choice_of)) else {
            return Ok(bad(
                "each choice needs an id and one of mine, theirs, both, keep, remove",
            ));
        };
        picks.push((id.to_string(), ch));
    }
    let (me, device) = (who_am_i(ctx)?, ctx.device.clone());
    let through = Through::new(ctx, root);
    let theirs = open(root)?.merge_pending()?.map(|p| p.theirs);
    let done = run(ctx, root, move |r| {
        let id = r.merge_resolve(&picks, &through, &me, &device)?;
        r.lfs_prune_held()?;
        Ok(json!({ "ok": true, "id": id }))
    })
    .await?;
    if let Some(tip) = theirs {
        git_history::landed(ctx, &root.display().to_string(), &tip).await;
    }
    Ok(done)
}
