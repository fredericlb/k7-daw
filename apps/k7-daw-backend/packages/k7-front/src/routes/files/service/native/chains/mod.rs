//! chains — the account's chain lists and the hub's cut resolved natively (1p-D3 to 1p-D9, 1p-D15).
//! What a reader gets wrong: only the download route writes (jobs); a library cloned with no pins file declines (1p-D7).

mod cold_set;
mod download;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_routes;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use axum::response::Response;
use hyper::HeaderMap;
use k7_file_service::chains::{
    ChainSettings, Chains, ColdRun, cold_chains_view, frozen_chains, live_chains, resolve_cold_run,
};
use k7_file_service::confine::Disk;
use k7_file_service::listings::families::library_names;
use k7_file_service::paths::join;
use k7_file_service::rig::chain_settings;
use k7_file_service::song_ask::{Input, song_roots};
use k7_file_service::tree::Limits;
use serde_json::{Value, json};
use song::{DiskSource, PathRule, Roots};

use super::answering::{BASE_WEIGHT, TREE_WEIGHT};
use super::docs::Docs;
use super::reads::{self, Json};
use super::refuse::refused;
use super::stack::{Failed, on_stack};
use super::stats::Stats;
use super::{Ctx, Req, Tag};
use crate::routes::files::service::{Acct, Who, claim_root};

/// What a request asks of the chains.
#[derive(Clone, Debug)]
pub(super) enum What {
    Frozen,
    Live,
    Cold,
    // The cold view as the download route reads it, counted as a download.
    Download,
    Run { want: String, chain: String },
    // The setting of `stems` or `instrumental`, then the view as the route answers it (F2).
    Set { which: String, chain_ref: String },
}

/// The native answer: a status and the body as the route sends it.
pub(super) struct Done {
    pub status: u16,
    pub body: Value,
}

/// What the piece answered, for the line an answering run logs every 30 seconds while it moves.
#[derive(Default)]
pub(super) struct Counts {
    pub lists: AtomicU64,
    pub cold_runs: AtomicU64,
    pub downloads: AtomicU64,
    pub declines: AtomicU64,
    pub panics: AtomicU64,
    last: AtomicU64,
    said: AtomicU64,
}

pub(super) fn counts() -> &'static Counts {
    static COUNTS: std::sync::OnceLock<Counts> = std::sync::OnceLock::new();
    COUNTS.get_or_init(Counts::default)
}

impl Counts {
    /// `[files] native chains: lists= coldRuns= downloads= declines= panics=`, at most every 30 s and only when a count moved.
    pub(super) fn report(&self, ctx: &Ctx) {
        let get = |c: &AtomicU64| c.load(Ordering::Relaxed);
        let total = get(&self.lists)
            + get(&self.cold_runs)
            + get(&self.downloads)
            + get(&self.declines)
            + get(&self.panics);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        if total == get(&self.said) || now.saturating_sub(get(&self.last)) < 30 {
            return;
        }
        self.last.store(now, Ordering::Relaxed);
        self.said.store(total, Ordering::Relaxed);
        (ctx.log)(&format!(
            "[files] native chains: lists={} coldRuns={} downloads={} declines={} panics={}",
            get(&self.lists),
            get(&self.cold_runs),
            get(&self.downloads),
            get(&self.declines),
            get(&self.panics)
        ));
    }
}

/// The tag a `GET` of the account's chains or the download's `POST` is, when it is one.
pub fn tag_of(method: &hyper::Method, path: &str) -> Option<Tag> {
    if method == hyper::Method::POST && path == "/account/cold-chains/download" {
        return Some(Tag::ColdChainsDownload);
    }
    if method == hyper::Method::PUT
        && path.starts_with("/account/cold-chains/")
        && !path["/account/cold-chains/".len()..].contains('/')
        && path != "/account/cold-chains/download"
    {
        return Some(Tag::ColdSet);
    }
    if method != hyper::Method::GET {
        return None;
    }
    match path {
        "/account/frozen-chains" => Some(Tag::FrozenChains),
        "/account/live-chains" => Some(Tag::LiveChains),
        "/account/cold-chains" => Some(Tag::ColdChains),
        _ => None,
    }
}

fn what_of(tag: Tag) -> Option<What> {
    match tag {
        Tag::FrozenChains => Some(What::Frozen),
        Tag::LiveChains => Some(What::Live),
        Tag::ColdChains => Some(What::Cold),
        _ => None,
    }
}

/// The context of a chains route: the account's bench is its workspace, and the account holds the sessions.
pub fn ctx_of(
    tag: Tag,
    env: &BTreeMap<String, String>,
    slug: &str,
    log: &k7_hub::Log,
    held: Option<(&Arc<Acct>, &std::path::Path)>,
) -> Ctx {
    let home = k7_file_service::libraries::Home::of(env);
    let root = join(&[&home.studio, "workspace", "Banc"]);
    let mut ctx = Ctx::of(
        tag,
        &Req {
            env,
            slug,
            workspace: "Banc",
            root: &root,
            query: "",
            body: &Value::Null,
            log,
            docs: None,
            thawed: None,
        },
    );
    ctx.docs = held.map(|(c, r)| Docs::of(c, r));
    ctx
}

/// The hook of the chains' routes: a context where the route is one of them, else `None`.
pub fn live_ctx(
    who: &Who<'_>,
    method: &hyper::Method,
    path: &str,
    _headers: &HeaderMap,
) -> Option<Ctx> {
    let tag = tag_of(method, path)?;
    let a = &who.held.account;
    let ctx = ctx_of(
        tag,
        &a.env,
        &a.slug,
        who.log,
        who.code_root.map(|r| (&who.held.acct, r)),
    );
    if tag != Tag::ColdChainsDownload {
        return Some(ctx);
    }
    // A download starts jobs through the account's `media` (1p-D3).
    download::download_ctx(ctx, who.held.media.is_some())
}

/// A decline counted and said once for this account.
fn note_decline(ctx: &Ctx, why: &'static str) {
    ctx.declined(why);
    Stats::bump(&counts().declines);
}

/// The bench's project folder: `<workspaces>/Banc/live.k7`, read and never made (1p-D7).
fn bench_of(ctx: &Ctx) -> String {
    join(&[&ctx.home.studio, "workspace", "Banc", "live.k7"])
}

/// The bench's roots, its session key and path rule, and the disk the vocabulary is read from.
pub(super) fn bench_session(
    ctx: &Ctx,
    docs: &Docs,
) -> Result<(Roots, String, PathRule, DiskSource), &'static str> {
    let bench = bench_of(ctx);
    let input = Input {
        home: &ctx.home,
        project: &bench,
        code_root: &docs.code_root,
        env: &docs.env,
    };
    let roots = song_roots(&input, &Disk).map_err(|d| d.as_str())?;
    let rule = roots.paths;
    let key = k7_file_service::songs::key_of(
        &roots.confine.clone().unwrap_or_else(|| bench.clone()),
        rule,
    )
    .map_err(|_| "workspace")?;
    let (disk, _) = k7_file_service::songs::disk_of(&roots).map_err(|_| "mounts")?;
    Ok((roots, key, rule, disk))
}

/// The native answer, blocking and on the compiler's stack; a `&'static str` is why it declined.
pub(super) fn compute(ctx: &Ctx, what: &What) -> Result<Done, &'static str> {
    #[cfg(test)]
    if matches!(ctx.chaos, super::Chaos::Panic) {
        panic!("a chains read that dies");
    }
    let docs = ctx.docs.as_ref().ok_or("no-docs")?;
    // The bench's workspace is pinned afresh once a library is here, as `pinBench` does before any view of it.
    if !library_names(&ctx.home).is_empty() {
        let root = join(&[&ctx.home.studio, "workspace", "Banc"]);
        std::fs::create_dir_all(&root).map_err(|_| "pins")?;
        k7_file_service::pins::ensure_fresh(&ctx.home, &root).map_err(|d| d.as_str())?;
    }
    let (roots, key, rule, disk) = bench_session(ctx, docs)?;
    let songs = docs.songs();
    let made = songs.with(&key, rule, |s| s.vocabulary(&disk, &roots));
    let vocab = made.map_err(|_| "retired")?.map_err(|_| "vocabulary")?;
    let chains = Chains::new(&disk, &roots, &vocab, &Limits::default()).map_err(|d| d.as_str())?;
    let settings = || {
        chain_settings(&ctx.home).map(|(stems, instrumental)| ChainSettings {
            stems,
            instrumental,
        })
    };
    let ok = |body: Value| Done { status: 200, body };
    match what {
        What::Frozen => frozen_chains(&chains).map(ok),
        What::Live => live_chains(&chains).map(ok),
        What::Cold | What::Download => {
            cold_chains_view(&chains, &settings().map_err(|d| d.as_str())?).map(ok)
        }
        What::Set { which, chain_ref } => (|| -> Result<Done, k7_file_service::space::Declined> {
            let view = cold_chains_view(&chains, &settings()?)?;
            let refuse = |error: String| Done {
                status: 400,
                body: json!({ "error": error }),
            };
            if !chain_ref.is_empty() {
                let row = view["chains"]
                    .as_array()
                    .and_then(|l| l.iter().find(|c| c["ref"] == chain_ref.as_str()));
                let Some(row) = row else {
                    return Ok(refuse(format!(
                        "'{chain_ref}' is not a cold chain of the library"
                    )));
                };
                if which == "instrumental" && row["instrumental"] != true {
                    return Ok(refuse(format!("'{chain_ref}' writes no instrumental")));
                }
            }
            let set = k7_file_service::preferences::update(&ctx.home, |p| {
                let value = (!chain_ref.is_empty()).then(|| chain_ref.clone());
                if which == "stems" {
                    p.stems_chain = value;
                } else {
                    p.instrumental_chain = value;
                }
            });
            if let Err(e) = set {
                (ctx.log)(&format!("[files] cold chain setting failed: {e}"));
                return Ok(Done {
                    status: 500,
                    body: json!({ "error": "internal error" }),
                });
            }
            cold_chains_view(&chains, &settings()?).map(ok)
        })(),
        What::Run { want, chain } => {
            let prefs = settings().map_err(|d| d.as_str())?;
            resolve_cold_run(&chains, &prefs, want, chain).map(|r| match r {
                ColdRun::Ok { model, outputs } => Done {
                    status: 200,
                    body: json!({ "model": model, "outputs": outputs }),
                },
                ColdRun::Refused { status, error } => Done {
                    status,
                    body: json!({ "error": error }),
                },
            })
        }
    }
    .map_err(|d| d.as_str())
}

/// The native answer off the request's thread under the answering weight, or `None` where it declined (1p-D9).
pub(super) async fn answer_what(ctx: &Ctx, what: What) -> Option<Done> {
    let c = counts();
    // A list walks a tree; the cut's chain weighs a request (1p-D30).
    let weight = if ctx.tag == Tag::ColdRun {
        BASE_WEIGHT
    } else {
        TREE_WEIGHT
    };
    let Some(_held) = ctx.gate.take(&ctx.account, weight).await else {
        ctx.declined("busy");
        Stats::bump(&c.declines);
        return None;
    };
    let ctx2 = ctx.clone();
    let made = on_stack(move || compute(&ctx2, &what)).await;
    let done = match made {
        Ok(Ok(d)) => d,
        Ok(Err(why)) => {
            ctx.declined(why);
            Stats::bump(&c.declines);
            c.report(ctx);
            return None;
        }
        Err(Failed::Panicked) => {
            Stats::bump(&c.panics);
            (ctx.log)(&format!("[native] {} panicked", ctx.tag.route()));
            c.report(ctx);
            return None;
        }
        Err(Failed::Lost) => return None,
    };
    Stats::bump(match ctx.tag {
        Tag::ColdRun => &c.cold_runs,
        Tag::ColdChainsDownload => &c.downloads,
        _ => &c.lists,
    });
    c.report(ctx);
    Some(done)
}

/// A list of the account's chains, a setting or a download answered natively; a decline is refused (1p-D4).
pub async fn live(who: &Who<'_>, path: &str, req: axum::extract::Request, ctx: Ctx) -> Response {
    if ctx.tag == Tag::ColdChainsDownload {
        let Some(media) = who.held.media.clone() else {
            return refused();
        };
        let owner = || claim_root(&who.held.account.home, who.user);
        return download::answer(ctx, &media, &owner, req).await;
    }
    if ctx.tag == Tag::ColdSet {
        return cold_set::answer(who, path, req, ctx).await;
    }
    let Some(what) = what_of(ctx.tag) else {
        return refused();
    };
    match answer_what(&ctx, what).await {
        Some(d) => reads::response(d.status, Json::Value(d.body)),
        None => refused(),
    }
}

/// The hub's cut answered natively; `None` is a decline, said by the caller (1p-D15).
pub async fn cold_run_answer(ctx: &Ctx, want: &str, chain: &str) -> Option<(u16, Value)> {
    let what = What::Run {
        want: want.to_owned(),
        chain: chain.to_owned(),
    };
    answer_what(ctx, what).await.map(|d| (d.status, d.body))
}
