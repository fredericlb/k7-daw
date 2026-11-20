//! reads.rs — `GET /file`, `POST /fmt`, `GET /tree` and `/media`'s path, read natively, the answer built as
//! `fileRoutes.ts` and `entryFailure` build it (1042 to 1056).
//! What a reader gets wrong: a decline, a panic or a lost task is refused with a sentence, never a 500 of its own.

use std::path::PathBuf;

use axum::body::Body;
use axum::response::Response;
use hyper::StatusCode;
use hyper::header::{CONTENT_TYPE, HeaderValue};
use k7_file_service::confine::Disk;
use k7_file_service::paths::join;
use k7_file_service::projects::info::project_info;
use k7_file_service::read::{ReadError, read_text_gated};
use k7_file_service::space::{Declined, PathError, Space};
use k7_file_service::tree::{Limits, tree};
use serde_json::{Value, json};

use super::answering::{BASE_WEIGHT, Held, TREE_WEIGHT};
use super::listings;
use super::{Ctx, Tag};
use crate::routes::files::service::routes::{self, Call, H};
use crate::routes::files::service::workspace_name_ok;

/// What a native read answers: JSON as a value, or as the text the tree is written in.
pub enum Json {
    Value(Value),
    Text(String),
}

/// A native answer and what the read met on the way.
pub struct Native {
    pub status: u16,
    pub body: Json,
    /// Folders the walk could not list, which the levels' listing says in a line.
    pub unlistable: u32,
    /// An I/O failure a route logs and does not tell the client (`entryFailure`).
    pub warn: Option<String>,
}

pub enum Outcome {
    Answer(Native),
    Declined(Declined),
}

fn plain(status: u16, body: Value) -> Native {
    Native {
        status,
        body: Json::Value(body),
        unlistable: 0,
        warn: None,
    }
}

fn failure(status: u16, message: String) -> Outcome {
    Outcome::Answer(plain(status, json!({ "error": message })))
}

fn value(v: Value) -> Outcome {
    Outcome::Answer(plain(200, v))
}

/// The native read: no HTTP, blocking; `admit` is asked the size of a file.
pub fn compute_with(ctx: &Ctx, admit: &dyn Fn(u64) -> bool) -> Outcome {
    #[cfg(test)]
    match &ctx.chaos {
        super::Chaos::None => {}
        super::Chaos::Panic => panic!("a read that panics"),
        super::Chaos::Decline(d) => return Outcome::Declined(*d),
    }
    let limits = Limits::default();
    match ctx.tag {
        Tag::Playlists
        | Tag::References
        | Tag::LibraryChains
        | Tag::LibraryAssemblies
        | Tag::ModuleLibraries => listings::compute_with(ctx, admit),
        Tag::File | Tag::Fmt => {
            let space = Space::open(&ctx.home, &ctx.root);
            match read_text_gated(&space, &ctx.rel, &limits, admit) {
                Ok(f) => {
                    value(json!({ "path": ctx.rel, "binary": f.binary, "content": f.content }))
                }
                Err(ReadError::NotFound { rel }) => {
                    failure(404, format!("no such file or folder: {rel}"))
                }
                Err(ReadError::IsFolder { rel }) => {
                    failure(400, format!("'{rel}' is a folder, not a file"))
                }
                Err(ReadError::Path(e)) => failure(400, e.to_string()),
                Err(ReadError::Io { code: "ENOENT" }) => {
                    failure(404, "no such file or folder".into())
                }
                Err(ReadError::Io { code }) => {
                    let mut n = plain(
                        400,
                        json!({ "error": format!("the file could not be changed ({code})") }),
                    );
                    n.warn = Some(format!("[files] {code}: could not read '{}'", ctx.rel));
                    Outcome::Answer(n)
                }
                Err(ReadError::Declined(d)) => Outcome::Declined(d),
            }
        }
        Tag::Tree => match tree(&Space::open(&ctx.home, &ctx.root), &limits) {
            Ok(t) => Outcome::Answer(Native {
                body: Json::Text(t.json),
                unlistable: t.unlistable,
                ..plain(200, Value::Null)
            }),
            Err(d) => Outcome::Declined(d),
        },
        Tag::Media => media_of(ctx),
        Tag::Revision => unreachable!("revision has its own route (revision.rs)"),
        Tag::Project => match project_info(&Space::open(&ctx.home, &ctx.root)) {
            Ok(t) => Outcome::Answer(plain(200, serde_json::from_str(&t).unwrap_or(Value::Null))),
            Err(d) => Outcome::Declined(d),
        },
        // The entries' writes are planned by `entries`, never read here.
        _ => Outcome::Declined(Declined::Busy),
    }
}

/// `getWorkspace`'s order, then `mediaPath` (1056): the name, the folder, the gates.
fn media_of(ctx: &Ctx) -> Outcome {
    if !workspace_name_ok(&ctx.workspace) {
        return failure(400, format!("invalid workspace name: {}", ctx.workspace));
    }
    let root = join(&[&ctx.root, &ctx.workspace]);
    if !std::fs::metadata(&root).is_ok_and(|m| m.is_dir()) {
        return failure(404, format!("unknown workspace: {}", ctx.workspace));
    }
    match Space::open(&ctx.home, &root).safe_path(&ctx.rel, &Disk) {
        Ok(abs) => value(json!({ "file": abs })),
        Err(PathError::Declined(d)) => Outcome::Declined(d),
        Err(e) => failure(400, e.to_string()),
    }
}

pub(super) fn response(status: u16, body: Json) -> Response {
    let text = match body {
        Json::Text(t) => t,
        Json::Value(v) => k7_sync::json::stringify(&v),
    };
    let mut r = Response::new(Body::from(text));
    *r.status_mut() = StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    r.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    r
}

/// The blocking read, off the request's thread and under the answering weights; `None` where it declined,
/// found no weight, or its thread was lost.
pub(super) async fn run(ctx: &Ctx) -> Option<Native> {
    let base = if ctx.tag == Tag::Tree || listings::weighs_a_tree(ctx.tag) {
        TREE_WEIGHT
    } else {
        BASE_WEIGHT
    };
    let Some(held) = ctx.gate.take(&ctx.account, base).await else {
        ctx.declined(Declined::Busy.as_str());
        return None;
    };
    let ctx2 = ctx.clone();
    match tokio::task::spawn_blocking(move || {
        let held = std::sync::Mutex::new(held);
        // A file's text is held, copied once and written once: three times its size on top of the base.
        let admit = |len: u64| grow(&held, len);
        compute_with(&ctx2, &admit)
    })
    .await
    {
        Ok(Outcome::Answer(n)) => Some(n),
        Ok(Outcome::Declined(d)) => {
            ctx.declined(d.as_str());
            None
        }
        Err(_) => None,
    }
}

fn grow(held: &std::sync::Mutex<Held>, len: u64) -> bool {
    let more = usize::try_from(len).map_or(usize::MAX, |n| n.saturating_mul(3));
    held.lock()
        .unwrap_or_else(|p| p.into_inner())
        .grow(more.saturating_sub(BASE_WEIGHT))
}

/// `GET /file`, `POST /fmt` and `GET /tree` answered natively, or `None` where it declined.
pub async fn answer(h: H, c: &Call, ctx: &Ctx) -> Option<Response> {
    match h {
        H::FileGet if ctx.rel.is_empty() => {
            return Some(super::entries::error(400, "path required"));
        }
        H::Fmt => {
            if let Some(r) = routes::invalid(c, &routes::PATH) {
                return Some(r);
            }
        }
        _ => {}
    }
    let n = run(ctx).await?;
    if let Some(w) = &n.warn {
        c.say("warn", w);
    }
    if n.unlistable > 0 {
        c.say(
            "warn",
            &format!(
                "[files] native: {} folder(s) of the tree could not be listed",
                n.unlistable
            ),
        );
    }
    Some(response(n.status, n.body))
}

/// `/media`'s path answered natively: the file, the route's own refusal, or `None` where it declined.
pub async fn media(ctx: &Ctx) -> Option<Result<PathBuf, Box<Response>>> {
    let n = run(ctx).await?;
    match (n.status, n.body) {
        (200, Json::Value(v)) => v
            .get("file")
            .and_then(Value::as_str)
            .map(|p| Ok(PathBuf::from(p))),
        (status, body) => Some(Err(Box::new(response(status, body)))),
    }
}
