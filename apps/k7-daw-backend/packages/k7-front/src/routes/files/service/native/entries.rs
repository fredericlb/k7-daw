//! entries.rs — `POST /folder`, `/rename`, `/move`, `/entry/delete` and `/upload`'s two halves, written natively: the
//! plan of the crate, the disk's part under the account's write lock, and the step that leaves
//! the native memos right. What a reader gets wrong: a write that has begun is never declined.

use std::sync::Arc;
use std::time::Duration;

use axum::response::Response;
use k7_file_service::confine::Disk;
use k7_file_service::edits;
use k7_file_service::entries::apply::{
    self, RETRY, Renamer, is_transient, make_folder, rename_once, upload_begin as begin_on_disk,
};
use k7_file_service::entries::plan::{answer_of, plan};
use k7_file_service::entries::{EntryError, Op, Outcome};
use k7_file_service::libraries::library_of;
use k7_file_service::paths::dirname;
use k7_file_service::rel::to_rel;
use k7_file_service::space::Space;
use serde_json::{Value, json};

use super::{Ctx, Tag, locks};
use crate::routes::files::json_answer;
use crate::routes::files::service::body::{self, Field};
use crate::routes::files::service::routes::{self, Call, H};
use crate::routes::files::service::stream::io_words;

const FOLDER: [(&str, Field); 2] = [("dir", Field::Text), ("name", Field::Text)];

/// The two strings a route carries: a path or a folder, and the name or destination beside it.
#[derive(Clone, Default)]
pub struct Params {
    pub a: String,
    pub b: String,
}

impl Params {
    /// The request's, by the keys its route names.
    pub fn of(tag: Tag, body: &Value) -> Params {
        let (a, b) = match tag {
            Tag::Folder => ("dir", "name"),
            Tag::Rename => ("path", "name"),
            Tag::Move => ("path", "toDir"),
            _ => ("path", ""),
        };
        Params {
            a: body::text(body, a),
            b: if b.is_empty() {
                String::new()
            } else {
                body::text(body, b)
            },
        }
    }

    pub fn op(&self, tag: Tag) -> Op<'_> {
        let (a, b) = (self.a.as_str(), self.b.as_str());
        match tag {
            Tag::Folder => Op::Folder { dir: a, name: b },
            Tag::Rename => Op::Rename { path: a, name: b },
            Tag::Move => Op::Move { path: a, to_dir: b },
            Tag::Upload => Op::Upload { dir: a, name: b },
            _ => Op::Delete { path: a },
        }
    }
}

/// The plan, blocking: every guard and stat the TypeScript runs, nothing written.
pub fn plan_now(ctx: &Ctx, p: &Params) -> Result<Outcome, EntryError> {
    #[cfg(test)]
    match &ctx.chaos {
        super::Chaos::None => {}
        super::Chaos::Panic => panic!("a plan that panics"),
        super::Chaos::Decline(d) => return Err(EntryError::Declined(*d)),
    }
    plan(&Space::open(&ctx.home, &ctx.root), &p.op(ctx.tag), &Disk)
}

/// The status a refusal is answered with: `EEXIST` is a 409 for an upload and a 400 elsewhere.
pub fn status_of(tag: Tag, e: &EntryError) -> u16 {
    match e {
        EntryError::NotFound { .. } => 404,
        EntryError::Exists { .. } if tag == Tag::Upload => 409,
        _ => 400,
    }
}

pub(super) fn error(status: u16, message: &str) -> Response {
    json_answer(status, &json!({ "error": message }))
}

/// A write refused or failed, as the route answers it; an I/O failure is logged whole and worded without a path (1233).
pub(super) fn refusal(tag: Tag, e: &EntryError, warn: &(dyn Fn(&str) + Sync)) -> Response {
    if let EntryError::Io { code, .. } = e {
        warn(&format!("[files] {e}"));
        let (status, msg) = io_words(Some(code));
        return error(status, &msg);
    }
    error(status_of(tag, e), &e.to_string())
}

pub(super) async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> T + Send + 'static,
) -> Option<T> {
    tokio::task::spawn_blocking(f).await.ok()
}

/// A rename retried while its code is transient: each attempt on a blocking thread, each wait a timer (1230).
pub async fn rename_retrying(
    r: Arc<dyn Renamer + Send + Sync>,
    from: &str,
    to: &str,
    delay: Duration,
) -> Result<(), EntryError> {
    let mut attempt = 0;
    loop {
        let (r, f, t) = (r.clone(), from.to_string(), to.to_string());
        let done = blocking(move || rename_once(&*r, &f, &t))
            .await
            .ok_or_else(|| lost("rename"))?;
        match done {
            Err(EntryError::Io { code, .. }) if is_transient(code) && attempt < RETRY.0 => {
                attempt += 1;
                tokio::time::sleep(delay).await;
            }
            done => return done,
        }
    }
}

pub(super) fn lost(syscall: &'static str) -> EntryError {
    EntryError::Io {
        code: "EIO",
        syscall,
        path: String::new(),
    }
}

/// A native write's stage, which the tests see in order and may hold; the front itself does nothing here.
pub async fn told(root: &str, stage: &str) {
    #[cfg(test)]
    {
        calls::gate(root, stage).await;
        calls::push(root, stage);
    }
    let _ = (root, stage);
}

/// The library's refs and the workspace's songs repointed as one landed set; the absolute files it changed.
async fn repointed(
    roots: [String; 2],
    refs: &(String, String, bool),
) -> Result<Vec<String>, String> {
    let (from, to, under) = (refs.0.clone(), refs.1.clone(), refs.2);
    blocking(move || edits::modules::repoint(&roots, &from, &to, under))
        .await
        .unwrap_or_else(|| Err("the repoint was lost".to_string()))
}

/// The four routes answered natively, or `None` where it declined (a decline, before anything is written).
pub async fn answer(h: H, c: &Call, ctx: &Ctx) -> Option<Response> {
    let schema: &[(&str, Field)] = match h {
        H::Folder => &FOLDER,
        H::Rename => &routes::PATH_NAME,
        H::Move => &routes::PATH_TO,
        H::EntryDelete => &routes::PATH,
        _ => return None,
    };
    if let Some(r) = routes::invalid(c, schema) {
        return Some(r);
    }
    let warn = |l: &str| c.say("warn", l);
    let lock = locks::write(&ctx.home.studio).await;
    let p = Params::of(ctx.tag, &c.body);
    let (ctx2, p2) = (ctx.clone(), p.clone());
    let planned = match blocking(move || plan_now(&ctx2, &p2)).await {
        Some(r) => r,
        None => return None,
    };
    let outcome = match planned {
        Ok(o) => o,
        Err(EntryError::Declined(d)) => {
            ctx.declined(d.as_str());
            return None;
        }
        Err(e) => return Some(refusal(ctx.tag, &e, &warn)),
    };
    let (c, ctx) = (c.clone(), ctx.clone());
    Some(
        finished(async move {
            let _lock = lock;
            let warn = |l: &str| c.say("warn", l);
            write(&c, &ctx, outcome, &warn).await
        })
        .await,
    )
}

/// A write begun runs to its end in a task of its own: a client gone mid-write must not strand a repoint or a memo.
pub(super) async fn finished(
    work: impl std::future::Future<Output = Response> + Send + 'static,
) -> Response {
    tokio::spawn(work)
        .await
        .unwrap_or_else(|_| error(500, "internal error"))
}

/// What a planned write does to the disk, then to the memos; its answer.
async fn write(c: &Call, ctx: &Ctx, outcome: Outcome, warn: &(dyn Fn(&str) + Sync)) -> Response {
    let real = Arc::new(apply::Disk);
    let (from, to, refs) = match &outcome {
        Outcome::Folder { abs } => {
            let abs = abs.clone();
            let made = blocking(move || make_folder(&abs))
                .await
                .unwrap_or_else(|| Err(lost("mkdir")));
            if let Err(e) = made {
                return refusal(ctx.tag, &e, warn);
            }
            told(&c.root, "after").await;
            return answered(ctx, &outcome, &[]).await;
        }
        Outcome::Unchanged { .. } => return answered(ctx, &outcome, &[]).await,
        Outcome::Rename { from, to, refs } => (from.clone(), to.clone(), refs.clone()),
        Outcome::Move { from, to } | Outcome::Recycle { from, to } => {
            (from.clone(), to.clone(), None)
        }
        Outcome::Upload { .. } => return error(500, "internal error"),
    };
    let (home, root, f2) = (ctx.home.clone(), ctx.root.clone(), from.clone());
    let facts = blocking(move || {
        let library = library_of(&home, &f2).map(|l| l.root);
        (library, root)
    })
    .await;
    let Some((library, root)) = facts else {
        return error(500, "internal error");
    };
    if let Outcome::Recycle { to, .. } = &outcome {
        let bin = dirname(to);
        let made = blocking(move || make_folder(&bin))
            .await
            .unwrap_or_else(|| Err(lost("mkdir")));
        if let Err(e) = made {
            return refusal(ctx.tag, &e, warn);
        }
    }
    // The prewarm's handles go first, straight from the media: the runtime may hold no workspace to cancel through (2431).
    if let Some(m) = &c.media {
        m.prewarms.cancel(&c.root, &m.waves).await;
    }
    told(&c.root, "before").await;
    if let Err(e) = rename_retrying(real, &from, &to, RETRY.1).await {
        return refusal(ctx.tag, &e, warn);
    }
    let mut files = Vec::new();
    let mut failed = None;
    if let (Some(refs), Some(lib)) = (&refs, library) {
        match repointed([lib, root], refs).await {
            Ok(f) => files = f,
            Err(e) => failed = Some(e),
        }
    }
    told(&c.root, "after").await;
    match failed {
        Some(e) => error(400, &e),
        None => answered(ctx, &outcome, &files).await,
    }
}

/// The route's body for a write that landed: the plan's, with the files a repoint changed beside the path.
async fn answered(ctx: &Ctx, outcome: &Outcome, files: &[String]) -> Response {
    let (home, root, outcome, files) = (
        ctx.home.clone(),
        ctx.root.clone(),
        outcome.clone(),
        files.to_vec(),
    );
    let body = blocking(move || {
        let space = Space::open(&home, &root);
        let mut v = answer_of(&space, &outcome);
        if let (Some(m), false) = (v.as_object_mut(), files.is_empty()) {
            m.insert(
                "repointed".into(),
                Value::from(files.iter().map(|f| to_rel(&space, f)).collect::<Vec<_>>()),
            );
        }
        v
    })
    .await;
    match body {
        Some(v) => json_answer(200, &v),
        None => error(500, "internal error"),
    }
}

/// What an upload's first half came to.
pub enum Begun {
    Ready { abs: String, tmp: String },
    Refused(Response),
    Declined,
}

/// `uploadBegin`: the target judged, its parents made, the temp's path; under the lock, which the bytes do not hold.
pub async fn upload_begin(ctx: &Ctx, dir: &str, name: &str) -> Begun {
    let warn = |l: &str| (ctx.log)(l);
    let _lock = locks::write(&ctx.home.studio).await;
    let p = Params {
        a: dir.to_string(),
        b: name.to_string(),
    };
    let (ctx2, p2) = (ctx.clone(), p);
    let Some(planned) = blocking(move || plan_now(&ctx2, &p2)).await else {
        return Begun::Declined;
    };
    let abs = match planned {
        Ok(Outcome::Upload { abs }) => abs,
        Ok(_) => return Begun::Declined,
        Err(EntryError::Declined(d)) => {
            ctx.declined(d.as_str());
            return Begun::Declined;
        }
        Err(e) => return Begun::Refused(refusal(Tag::Upload, &e, &warn)),
    };
    let (home, root, a2) = (ctx.home.clone(), ctx.root.clone(), abs.clone());
    let begun = blocking(move || begin_on_disk(&Space::open(&home, &root), &a2))
        .await
        .unwrap_or_else(|| Err(lost("mkdir")));
    match begun {
        Ok(tmp) => Begun::Ready { abs, tmp },
        Err(e) => Begun::Refused(refusal(Tag::Upload, &e, &warn)),
    }
}

/// `uploadEnd`: the temp renamed over a target still free, `{path}` answered; run to its end as a write is.
pub async fn upload_end(ctx: &Ctx, abs: &str, tmp: &str) -> Response {
    let (ctx, abs, tmp) = (ctx.clone(), abs.to_string(), tmp.to_string());
    let lock = locks::write(&ctx.home.studio).await;
    finished(async move {
        let _lock = lock;
        let warn = |l: &str| (ctx.log)(l);
        let (home, root) = (ctx.home.clone(), ctx.root.clone());
        let done = blocking(move || {
            let space = Space::open(&home, &root);
            apply::upload_end(&space, &abs, &tmp).map(|()| to_rel(&space, &abs))
        })
        .await
        .unwrap_or_else(|| Err(lost("rename")));
        match done {
            Ok(path) => {
                told(&ctx.root, "after").await;
                json_answer(200, &json!({ "path": path }))
            }
            Err(e) => refusal(Tag::Upload, &e, &warn),
        }
    })
    .await
}

/// The bridge calls a test watches, in order, by workspace root; a test may hold one stage until it lets it go.
#[cfg(test)]
pub mod calls {
    use std::sync::{Arc, Mutex};

    use tokio::sync::Semaphore;

    static CALLS: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());
    type Gates = Mutex<Vec<(String, String, Arc<Semaphore>)>>;
    static GATES: Gates = Mutex::new(Vec::new());

    /// `what` of `root` waits, before it is sent, until the test adds a permit to the answer.
    pub fn hold(root: &str, what: &str) -> Arc<Semaphore> {
        let s = Arc::new(Semaphore::new(0));
        GATES.lock().unwrap_or_else(|p| p.into_inner()).push((
            root.to_string(),
            what.to_string(),
            s.clone(),
        ));
        s
    }

    pub async fn gate(root: &str, what: &str) {
        let held = GATES
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .iter()
            .find(|(r, w, _)| r == root && w == what)
            .map(|(_, _, s)| s.clone());
        if let Some(s) = held {
            let _ = s.acquire().await;
        }
    }

    static ARGS: Mutex<Vec<(String, String, serde_json::Value)>> = Mutex::new(Vec::new());

    /// The arguments an ask of `what` for `root` carried, in order.
    pub fn args_of(root: &str, what: &str) -> Vec<serde_json::Value> {
        let all = ARGS.lock().unwrap_or_else(|p| p.into_inner());
        all.iter()
            .filter(|(r, w, _)| r == root && w == what)
            .map(|(_, _, a)| a.clone())
            .collect()
    }

    /// `push`, with the arguments kept for `args_of`.
    pub fn push_args(root: &str, what: &str, args: &serde_json::Value) {
        let mut all = ARGS.lock().unwrap_or_else(|p| p.into_inner());
        all.push((root.to_string(), what.to_string(), args.clone()));
        drop(all);
        push(root, what);
    }

    pub fn push(root: &str, what: &str) {
        CALLS
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push((root.to_string(), what.to_string()));
    }

    /// What was sent for `root`, in order.
    pub fn of(root: &str) -> Vec<String> {
        CALLS
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .iter()
            .filter(|(r, _)| r == root)
            .map(|(_, w)| w.clone())
            .collect()
    }
}
