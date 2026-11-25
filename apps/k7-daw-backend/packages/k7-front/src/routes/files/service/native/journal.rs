//! journal.rs — `PUT /file`, `POST /undo`, `/redo`, `GET /history` and `/history/entries` answered natively (1770 to 1773, 1760
//! to 1764): the crate's journal, the account's lock after its gate (1733), a write or a step finished in a task of its own.
//! What a reader gets wrong: while this answers, no other history is kept, so only `PUT /file` may decline.

use axum::response::Response;
use k7_file_service::atomic::write_atomic;
use k7_file_service::confine::Disk;
use k7_file_service::edits::stamp::stamp_format_for;
use k7_file_service::filter::{code_of, project_app};
use k7_file_service::journal::put::{PutPlan, plan_put};
use k7_file_service::journal::{Journals, Mode as Bracket, Owner};
use k7_file_service::paths::{basename, dirname, is_within, join, resolve_all};
use k7_file_service::rel::to_rel;
use k7_file_service::revisions::Revisions;
use k7_file_service::space::Space;
use k7_file_service::tree::Limits;
use serde_json::{Value, json};

use super::answering::{ACCOUNT_SHARE, Held, PUT_BASE};
#[cfg(test)]
use super::entries::calls;
use super::entries::{blocking, told};
use super::{Ctx, Tag, locks};
use crate::routes::files::json_answer;
use crate::routes::files::service::Window;
use crate::routes::files::service::body::{self, Field};
use crate::routes::files::service::routes::{self, Call};
use crate::routes::files::service::stream::io_words;

pub(super) const PUT: [(&str, Field); 3] = [
    ("path", Field::Text),
    ("content", Field::Str),
    ("base", Field::OptStr),
];
/// What a read or a step weighs before the journal's own size is added (1772).
pub(super) const WEIGHT: usize = 1 << 20;
/// The level an event `start` written in seconds needs (`SECONDS_START_LEVEL`).
const SECONDS_START_LEVEL: u32 = 2;

pub(super) fn error(status: u16, message: &str) -> Response {
    json_answer(status, &json!({ "error": message }))
}

pub(super) fn busy() -> Response {
    error(503, "file service: busy, ask again")
}

/// The five routes answered natively; only a save may be `None`, before anything is written, where it is declined.
pub async fn answer(c: &Call, ctx: &Ctx) -> Option<Response> {
    match ctx.tag {
        Tag::FilePut => put(c, ctx).await,
        Tag::Undo | Tag::Redo => Some(super::journal_undo::step(c, ctx).await),
        Tag::History | Tag::HistoryEntries => Some(read(c, ctx).await),
        Tag::Changes => Some(super::changes::answer(c, ctx).await),
        Tag::Verb => Some(super::verb_run::answer(c, ctx).await),
        Tag::VerbBundle | Tag::VerbLibrary => Some(super::verb_bundle::answer(c, ctx).await),
        Tag::VerbHost(m) => Some(super::verb_host::answer(c, ctx, m).await),
        _ => None,
    }
}

/// The gate's key for this workspace: 1341's spelling, as the bridge keys its records.
pub(super) fn gate_root(ctx: &Ctx) -> String {
    resolve_all(&[&ctx.root])
}

/// The weight of a read or a step: the journal's file, parsed, within what an account may hold (1772).
pub(super) async fn weigh(ctx: &Ctx, active: &str) -> Option<Held> {
    let (home, dir) = (ctx.home.clone(), active.to_string());
    let len = blocking(move || {
        let store = k7_file_service::journal::store::store_for(&home, &dir);
        std::fs::metadata(join(&[&store, "journal.json"])).map_or(0, |m| m.len() as usize)
    })
    .await
    .unwrap_or(0);
    ctx.gate
        .take(
            &ctx.account,
            (WEIGHT + len.saturating_mul(3)).min(ACCOUNT_SHARE),
        )
        .await
}

/// The open project, the marker's; a marker that cannot be read is a 503.
pub(super) async fn active_of(_c: &Call, ctx: &Ctx) -> Result<Option<String>, Box<Response>> {
    let (home, root) = (ctx.home.clone(), ctx.root.clone());
    match blocking(move || Space::open(&home, &root).active()).await {
        Some(Ok(active)) => Ok(active),
        _ => Err(Box::new(busy())),
    }
}

/// The reads' lossy flag, counted once per read of the journal's file (1796).
pub(super) fn count_lossy(ctx: &Ctx) {
    if Journals::global().take_lossy(&gate_root(ctx)).any() {
        super::stats::Stats::bump(&ctx.stats.lossy);
    }
}

/// `HistoryListEntry[]` with each path made project-relative, as `WorkspaceHistory.list` (1771).
pub fn entries_answer(list: Vec<Value>, prefix: &str) -> Value {
    let strip = |p: &str| p.strip_prefix(prefix).unwrap_or(p).to_string();
    let list = list.into_iter().map(|mut e| {
        if let Some(Value::Array(paths)) = e.get_mut("paths") {
            for p in paths.iter_mut() {
                if let Some(s) = p.as_str() {
                    *p = Value::String(strip(s));
                }
            }
        }
        e
    });
    Value::Array(list.collect())
}

/// `GET /history` and `/history/entries`: the journal of the open project, read where it is (1771).
async fn read(c: &Call, ctx: &Ctx) -> Response {
    let active = match active_of(c, ctx).await {
        Ok(a) => a,
        Err(r) => return *r,
    };
    let Some(active) = active else {
        let empty = match ctx.tag {
            Tag::History => json!({ "canUndo": false, "canRedo": false }),
            _ => json!([]),
        };
        return json_answer(200, &empty);
    };
    let Some(_held) = weigh(ctx, &active).await else {
        return busy();
    };
    let (ctx2, entries) = (ctx.clone(), ctx.tag == Tag::HistoryEntries);
    let answered = blocking(move || {
        let (journals, root) = (Journals::global(), gate_root(&ctx2));
        let out = if entries {
            let space = Space::open(&ctx2.home, &ctx2.root);
            let prefix = format!("{}/", to_rel(&space, &active));
            entries_answer(journals.list_of(&ctx2.home, &root, &active), &prefix)
        } else {
            journals.state_of(&ctx2.home, &root, &active)
        };
        count_lossy(&ctx2);
        out
    })
    .await;
    match answered {
        Some(v) => json_answer(200, &v),
        None => error(500, "internal error"),
    }
}

/// What a save's write came to under the account's lock.
pub(super) enum Done {
    Written { abs: String },
    Refused { status: u16, body: Value },
    Declined(&'static str),
    Failed { code: &'static str, what: String },
}

/// The base, the guards and the write, blocking: the plan of the crate, then the disk (1760, 1761).
pub(super) fn write_now(ctx: &Ctx, path: &str, content: &str, base: Option<&str>) -> Done {
    let space = Space::open(&ctx.home, &ctx.root);
    let abs = match plan_put(&space, path, content, base, &Limits::default()) {
        PutPlan::Write { abs } => abs,
        PutPlan::Refused { status, body } => return Done::Refused { status, body },
        PutPlan::Declined(d) => return Done::Declined(d.as_str()),
    };
    let fail = |e: std::io::Error, what: &str| Done::Failed {
        code: code_of(&e),
        what: format!("{what}: {e}"),
    };
    if let Err(e) = std::fs::create_dir_all(dirname(&abs)) {
        return fail(e, "mkdir");
    }
    if let Err(e) = write_atomic(&abs, content.as_bytes()) {
        return fail(e, "write");
    }
    Revisions::global().bump(space.root());
    Done::Written { abs }
}

/// The text of `project.k7x`'s `format(N)` line, the level a project declares (`formatLevelOfText`).
fn declared_level(text: &str) -> Option<u32> {
    text.lines().find_map(|line| {
        let rest = line.strip_prefix(['"', '\''].as_slice()).unwrap_or(line);
        let rest = rest.strip_prefix("format")?;
        let rest = rest.strip_prefix(['"', '\''].as_slice()).unwrap_or(rest);
        let rest = rest.trim_start().strip_prefix('(')?.trim_start();
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        digits.parse().ok()
    })
}

/// Whether a saved `.k7x` may need its project's level raised (1764): the stamp makes the exact answer, this only spares its lock.
pub(super) fn may_stamp(root: &str, abs: &str, content: &str) -> bool {
    // A seconds start is a key `start`, so a text without the word places none.
    if !abs.ends_with(".k7x") || !content.contains("start") {
        return false;
    }
    let mut dir = dirname(abs);
    while project_app(&basename(&dir)).is_none() {
        if dir == root || dirname(&dir) == dir {
            return false;
        }
        dir = dirname(&dir);
    }
    let level = std::fs::read(join(&[&dir, "project.k7x"]))
        .ok()
        .and_then(|b| declared_level(&String::from_utf8_lossy(&b)));
    level.is_none_or(|n| n < SECONDS_START_LEVEL)
}

/// A written `.k7x` that may place an event in seconds raises its project's level, natively, under the account's lock (O2).
pub(super) async fn stamp_after(c: &Call, ctx: &Ctx, root: &str, abs: &str, content: &str) {
    let (r, a, t) = (root.to_string(), abs.to_string(), content.to_string());
    if blocking(move || may_stamp(&r, &a, &t)).await != Some(true) {
        return;
    }
    #[cfg(test)]
    calls::gate(&c.root, "stamp").await;
    #[cfg(test)]
    calls::push(&c.root, "stamp");
    let _ = c;
    let lock = locks::write(&ctx.home.studio).await;
    let (r, a, studio_root) = (root.to_string(), abs.to_string(), ctx.root.clone());
    let home = ctx.home.clone();
    blocking(move || {
        if stamp_format_for(&r, &a) {
            Revisions::global().bump(Space::open(&home, &studio_root).root());
        }
    })
    .await;
    drop(lock);
}

/// Opens the save's capture window: the registry's where a native capture holds the root (2157).
pub(super) async fn open_window(c: &Call, label: &str) -> Window {
    #[cfg(test)]
    calls::gate(&c.root, "hold").await;
    #[cfg(test)]
    calls::push(&c.root, "hold");
    Window::open(&c.root, label).await
}

pub(super) async fn close_window(c: &Call, window: Window) {
    #[cfg(test)]
    calls::gate(&c.root, "release").await;
    #[cfg(test)]
    calls::push(&c.root, "release");
    let _ = c;
    window.close().await;
}

/// What a save weighs: a small base, its text and the base's read held twice over, and the project's image it keeps, within a share (1780a).
pub(super) fn put_weight(content: usize, base: usize, image: usize) -> usize {
    PUT_BASE
        .saturating_add(image)
        .saturating_add(content.saturating_mul(2))
        .saturating_add(base.saturating_mul(2))
        .min(ACCOUNT_SHARE)
}

/// The size of the file a save's base is read from, without following a link; 0 where it is not there or not a file.
fn base_len(space: &Space<'_>, path: &str) -> usize {
    let Ok(abs) = space.safe_path(path, &Disk) else {
        return 0;
    };
    std::fs::symlink_metadata(abs).map_or(0, |m| if m.is_file() { m.len() as usize } else { 0 })
}

/// `PUT /file`: the capture's window, the record, the base and the write, the stamp, the engine's push (1735, 1762).
async fn put(c: &Call, ctx: &Ctx) -> Option<Response> {
    if let Some(r) = routes::invalid(c, &PUT) {
        return Some(r);
    }
    let path = body::text(&c.body, "path");
    let content = body::text(&c.body, "content");
    let base = c.body.get("base").and_then(Value::as_str).map(String::from);
    let (home, root, p2, has_base) = (
        ctx.home.clone(),
        ctx.root.clone(),
        path.clone(),
        base.is_some(),
    );
    let seen = blocking(move || {
        let space = Space::open(&home, &root);
        let base_len = if has_base { base_len(&space, &p2) } else { 0 };
        space.active().map(|a| (a, base_len))
    })
    .await;
    let (active, base_len) = match seen {
        Some(Ok(a)) => a,
        Some(Err(d)) => {
            ctx.declined(d.as_str());
            return None;
        }
        None => return None,
    };
    let Some(held) = ctx
        .gate
        .take(
            &ctx.account,
            put_weight(
                content.len(),
                base_len,
                Journals::global().image_bytes(&gate_root(ctx)),
            ),
        )
        .await
    else {
        ctx.declined("busy");
        return None;
    };
    let (c2, ctx2) = (c.clone(), ctx.clone());
    // A save that has begun runs to its end in a task of its own: a client that left strands no step (1736).
    let ran = tokio::spawn(async move {
        let _held = held;
        save(&c2, &ctx2, active, path, content, base).await
    })
    .await;
    match ran {
        Ok(r) => r,
        Err(_) => Some(error(500, "internal error")),
    }
}

/// A record this front began: it leaves the gate when the task holding it ends, panic included; ending it first makes this a no-op.
pub(super) struct Open(pub(super) u64);

impl Drop for Open {
    fn drop(&mut self) {
        Journals::global().abort(self.0);
    }
}

async fn save(
    c: &Call,
    ctx: &Ctx,
    active: Option<String>,
    path: String,
    content: String,
    base: Option<String>,
) -> Option<Response> {
    let label = format!("edit {path}");
    let window = open_window(c, &label).await;
    let journals = Journals::global();
    let root = gate_root(ctx);
    // The journal's gate first, the account's lock after it, never the reverse (1733).
    let guard = journals.enter(&root).await;
    let (home, r2, dir, l2) = (
        ctx.home.clone(),
        root.clone(),
        active.clone(),
        label.clone(),
    );
    let begun = blocking(move || {
        journals.begin_with(
            guard,
            &home,
            &r2,
            dir.as_deref(),
            &l2,
            Bracket::Step,
            Owner::mint(),
        )
    })
    .await;
    let Some(begun) = begun else {
        close_window(c, window).await;
        return Some(error(500, "internal error"));
    };
    let _open = Open(begun.id);
    let lock = locks::write(&ctx.home.studio).await;
    let (ctx2, p2, c2, b2) = (ctx.clone(), path.clone(), content.clone(), base.clone());
    let done = blocking(move || write_now(&ctx2, &p2, &c2, b2.as_deref())).await;
    let done = done.unwrap_or(Done::Failed {
        code: "EIO",
        what: "the write was lost".into(),
    });
    if let Done::Written { .. } = &done {
        told(&c.root, "after").await;
    }
    drop(lock);
    let mut moved = false;
    let mut inside = false;
    if let Done::Written { abs } = &done {
        stamp_after(c, ctx, &root, abs, &content).await;
        let id = begun.id;
        let ended = blocking(move || journals.end_now(id, None)).await;
        if let Some(ended) = ended {
            if let Some(why) = &ended.failure {
                c.say("warn", &format!("[files] native: journal failed: {why}"));
            }
            moved = !ended.abandoned && ended.after != begun.rev;
        }
        inside = active.as_deref().is_some_and(|dir| is_within(dir, abs));
    } else {
        journals.abort(begun.id);
    }
    count_lossy(ctx);
    close_window(c, window).await;
    match done {
        Done::Declined(why) => {
            ctx.declined(why);
            None
        }
        Done::Refused { status, body } => Some(json_answer(status, &body)),
        Done::Failed { code, what } => {
            c.say("warn", &format!("[files] {what}"));
            let (status, msg) = io_words(Some(code));
            Some(error(status, &msg))
        }
        Done::Written { .. } => {
            if moved && inside {
                pushed(c).await;
            }
            Some(json_answer(200, &json!({ "ok": true })))
        }
    }
}

/// The open project recompiled and sent to the engine, after a write inside it; a failure is said and costs nothing (1763).
pub(crate) async fn pushed(c: &Call) {
    if let Some(e) = &c.engine {
        e.push_active(&c.root).await;
    } else {
        #[cfg(test)]
        calls::push(&c.root, "push");
    }
}
