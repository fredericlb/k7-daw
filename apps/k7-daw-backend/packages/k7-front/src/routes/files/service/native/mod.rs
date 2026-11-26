//! native — the file service's logic answered in Rust, every piece answering (docs/file-native.md): a route's
//! context is built here and handed to the piece its tag belongs to.
//! What a reader gets wrong: a piece that declines refuses with a sentence (`refuse.rs`); nothing answers in its place.

pub mod answering;
pub mod bench;
mod bracket;
pub mod callbacks;
pub mod capture_bundle;
pub mod capture_start;
pub mod chains;
pub mod changes;
pub mod docs;
pub(crate) mod docs_failure;
pub mod engine;
pub mod entries;
pub mod git;
mod git_author;
mod git_autosave;
pub mod git_autosave_task;
pub mod git_connect;
pub mod git_fetch;
mod git_forge;
mod git_forge_api;
pub mod git_forge_routes;
mod git_forge_token;
mod git_forgejo;
/// Where a sign-in that came back to the front's root is sent on (`routes/last`).
pub(crate) use git_forgejo::CALLBACK as FORGE_CALLBACK;
mod git_forgejo_flow;
mod git_forgejo_servers;
mod git_github;
pub mod git_history;
mod git_key_retry;
mod git_keys;
pub mod git_lock;
mod git_roam;
mod git_sync;
mod git_task;
pub mod history;
pub mod import_run;
pub mod jobs;
pub mod journal;
pub mod journal_undo;
mod landing;
pub mod listings;
pub mod locks;
pub mod media;
pub mod modules;
pub mod open_prepare;
pub mod orphans;
pub mod orphans_project;
pub mod picture_run;
pub mod prefs_routes;
pub mod projects;
pub mod projects_open;
pub mod reads;
pub mod refuse;
pub mod revision;
pub mod runtime;
pub mod shares;
pub mod stack;
pub mod stats;
mod tag;
pub mod verb_ask;
pub mod verb_host;
pub mod verb_run;
pub mod workspaces;
pub mod workspaces_hook;
pub mod workspaces_memo;
pub mod workspaces_start;

use std::collections::BTreeMap;

use k7_file_service::libraries::Home;
use k7_file_service::listings::Remote;
use k7_hub::Log;
use serde_json::Value;

use super::Acct;
use super::body;
use super::routes::H;
pub use tag::{Piece, Tag};

/// What a route asked and what it was asked of, as `ctx_for` reads it.
pub struct Req<'a> {
    /// The account's environment: where `STUDIO_HOME` and the libraries are.
    pub env: &'a BTreeMap<String, String>,
    pub slug: &'a str,
    /// The workspace's name; for `/media` the query's, which may be unchecked.
    pub workspace: &'a str,
    /// The workspace's root; for `/media` the account's folder of workspaces, the name joined by the read.
    pub root: &'a str,
    pub query: &'a str,
    pub body: &'a Value,
    pub log: &'a Log,
    /// The account and the code root, for a docs route (1113).
    pub docs: Option<(&'a std::sync::Arc<Acct>, &'a std::path::Path)>,
    /// The account's thawed modules where `engine-jobs` answers: an ask reads them here, not from the runtime (2310).
    pub thawed: Option<&'a std::sync::Arc<jobs::Thawed>>,
}

/// What one request's native read needs, built per request and only for a route whose piece is on.
#[derive(Clone)]
pub struct Ctx {
    pub tag: Tag,
    /// Whether a compile marks the renders it used: false for media's reads of the document, which use none.
    pub touches: bool,
    pub home: Home,
    pub root: String,
    pub workspace: String,
    /// The path the route was asked (`path` of the query or the body).
    pub rel: String,
    pub account: String,
    pub log: Log,
    /// What answering may hold at once (1057).
    pub gate: &'static answering::Gate,
    /// Where its counts go: the process's own, a test's apart.
    pub stats: &'static stats::Stats,
    /// Filled only for a docs tag.
    pub docs: Option<docs::Docs>,
    /// The origin's published versions, asked before the walk (1431); empty for any listing but the levels.
    pub remote: Option<std::sync::Arc<Remote>>,
    /// What answers a host member once its gates pass (3f); none where no member is served yet.
    pub verbs: Option<std::sync::Arc<dyn verb_host::Driver>>,
    #[cfg(test)]
    pub chaos: Chaos,
}

/// What a test makes the native side do.
#[cfg(test)]
#[derive(Clone)]
pub enum Chaos {
    None,
    Panic,
    /// The native read declines for this reason, as a hostile folder would make it.
    Decline(k7_file_service::space::Declined),
}

#[cfg(test)]
thread_local! {
    /// Contexts this thread built, for a test that counts them.
    pub static BUILT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

impl Ctx {
    fn of(tag: Tag, req: &Req<'_>) -> Ctx {
        #[cfg(test)]
        BUILT.with(|b| b.set(b.get() + 1));
        let from_body = |key: &str| {
            req.body
                .get(key)
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string()
        };
        let rel = match tag {
            Tag::Fmt | Tag::Rename | Tag::Move | Tag::LibraryMove | Tag::Delete | Tag::FilePut => {
                from_body("path")
            }
            Tag::Folder => from_body("dir"),
            Tag::Upload => body::query(req.query, "dir"),
            _ => body::query(req.query, "path"),
        };
        Ctx {
            tag,
            touches: true,
            home: Home::of(req.env),
            root: req.root.to_string(),
            workspace: req.workspace.to_string(),
            rel,
            account: req.slug.to_string(),
            log: req.log.clone(),
            gate: answering::global(),
            stats: stats::global(),
            docs: (tag.piece() == Piece::Docs
                || matches!(
                    tag,
                    Tag::Changes
                        | Tag::Import
                        | Tag::Verb
                        | Tag::VerbComplete
                        | Tag::Picture
                        | Tag::PictureScene
                        | Tag::VerbHost(_)
                        | Tag::ModuleCreate
                        | Tag::ModuleClone
                        | Tag::NodeCreate
                        | Tag::Presets
                        | Tag::ModuleUi
                        | Tag::Consoles
                        | Tag::ModuleEngines
                        | Tag::ModuleBase
                        | Tag::ProjectPlugins
                        | Tag::ProjectCompat
                        | Tag::Resources
                        | Tag::ProjectCreate
                ))
            .then(|| {
                req.docs.map(|(c, r)| {
                    let docs = docs::Docs::of(c, r);
                    match req.thawed {
                        Some(t) => docs.with_thawed(t.clone()),
                        None => docs,
                    }
                })
            })
            .flatten(),
            remote: None,
            verbs: None,
            #[cfg(test)]
            chaos: Chaos::None,
        }
    }

    /// A decline said once per workspace and reason, noted for the refusal that follows (1010).
    pub fn declined(&self, why: &'static str) {
        self.stats.say_declined(self, why);
    }
}

/// The route answered natively, or `None` where its piece declined, from the piece its tag belongs to.
pub async fn answer(h: H, c: &super::routes::Call, ctx: &Ctx) -> Option<axum::response::Response> {
    match ctx.tag.piece() {
        Piece::Entries if modules::owns(h) => modules::answer(h, c, ctx).await,
        Piece::Entries if orphans::owns(h) => orphans::answer(h, c, ctx).await,
        Piece::Entries if orphans_project::owns(h) => orphans_project::answer(h, c, ctx).await,
        Piece::Entries => entries::answer(h, c, ctx).await,
        Piece::Revision => revision::answer(ctx).await,
        Piece::Docs => docs::answer(c, ctx).await,
        Piece::Journal => journal::answer(c, ctx).await,
        Piece::Projects => projects::answer(ctx).await,
        _ => reads::answer(h, c, ctx).await,
    }
}

/// The context of a route the native pieces answer, or `None` for a route no tag names.
pub fn ctx_for(tag: Option<Tag>, req: &Req<'_>) -> Option<Ctx> {
    Some(Ctx::of(tag?, req))
}

#[cfg(test)]
pub(crate) mod fold;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_account;
#[cfg(test)]
mod tests_adopt;
#[cfg(test)]
mod tests_bench;
#[cfg(test)]
mod tests_bench_more;
#[cfg(test)]
mod tests_bounds;
#[cfg(test)]
mod tests_bundle;
#[cfg(test)]
mod tests_capture;
#[cfg(test)]
mod tests_changes;
#[cfg(test)]
mod tests_changes_fix;
#[cfg(test)]
mod tests_device;
#[cfg(test)]
mod tests_docs;
#[cfg(test)]
mod tests_entries;
#[cfg(test)]
mod tests_entries_end;
#[cfg(test)]
mod tests_git;
#[cfg(test)]
mod tests_git_author;
#[cfg(test)]
mod tests_git_autosave;
#[cfg(test)]
mod tests_git_forge;
#[cfg(test)]
mod tests_git_forgejo;
#[cfg(test)]
mod tests_git_forgejo_fake;
#[cfg(test)]
mod tests_git_forgejo_sync;
#[cfg(test)]
mod tests_git_github;
#[cfg(test)]
mod tests_git_history;
#[cfg(test)]
mod tests_git_key_retry;
#[cfg(test)]
mod tests_git_review;
#[cfg(test)]
mod tests_git_shared_key;
#[cfg(test)]
#[cfg(test)]
mod tests_history;
#[cfg(test)]
mod tests_hub;
#[cfg(test)]
mod tests_import;
#[cfg(test)]
mod tests_journal;
#[cfg(test)]
mod tests_journal_more;
#[cfg(test)]
mod tests_landing;
#[cfg(test)]
mod tests_last_ops;
#[cfg(test)]
mod tests_listings;
#[cfg(test)]
mod tests_modules;
#[cfg(test)]
mod tests_open_repairs;
#[cfg(test)]
mod tests_orphans;
#[cfg(test)]
mod tests_picture_run;
#[cfg(test)]
mod tests_projects;
#[cfg(test)]
mod tests_projects_more;
#[cfg(test)]
mod tests_projects_open;
#[cfg(test)]
mod tests_projects_review;
#[cfg(test)]
mod tests_refuse;
#[cfg(test)]
mod tests_revision;
#[cfg(test)]
mod tests_roam;
#[cfg(test)]
mod tests_verb_run;
#[cfg(test)]
mod tests_workspaces;
