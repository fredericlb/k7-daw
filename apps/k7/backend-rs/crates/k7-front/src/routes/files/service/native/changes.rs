//! changes.rs — `POST /changes` (3d): the browser's verbs commit one change set here, whole or not at all, as one journal step and
//! one relay batch. What a reader gets wrong: the answer is native or a 503; freshness is judged under the lock.

use axum::response::Response;
use k7_file_service::atomic::Real;
use k7_file_service::changes::commit::{Landed, commit};
use k7_file_service::changes::fresh::fresh;
use k7_file_service::changes::plan::{Act, Planned, plan};
use k7_file_service::changes::request::parse;
use k7_file_service::changes::target::{Target, confined_in, target};
use k7_file_service::changes::{Refusal, Request, jobs};
use k7_file_service::journal::{Journals, Mode};
use k7_file_service::paths::{is_within, resolve};
use k7_file_service::revisions::Revisions;
use k7_file_service::space::Space;
use k7_file_service::verb_host::jobs::Table;
use serde_json::json;

use super::answering::Held;
use super::bracket::{self, Edit, Spec};
use super::entries::blocking;
use super::journal::{error, gate_root, put_weight};
use super::{Ctx, docs};
use crate::routes::files::json_answer;
use crate::routes::files::service::routes::Call;
use crate::routes::files::service::stream::io_words;

/// The sentence and status of a refusal; a 409 carries the files, the project's flag and the token now.
fn refused(r: &Refusal, rev: Option<&str>) -> Response {
    let mut body = json!({ "error": r.to_string() });
    match (r, body.as_object_mut()) {
        (Refusal::Stale { files, project }, Some(o)) => {
            o.insert("stale".into(), json!(files));
            o.insert("project".into(), json!(project));
            o.insert("rev".into(), json!(rev));
        }
        (Refusal::Elsewhere { open }, Some(o)) => {
            o.insert("stale".into(), json!([]));
            o.insert("project".into(), json!(false));
            o.insert("open".into(), json!(open));
            o.insert("rev".into(), json!(rev));
        }
        (Refusal::Unparsable { file, .. }, Some(o)) => {
            o.insert("file".into(), json!(file));
        }
        _ => {}
    }
    json_answer(r.status(), &body)
}

/// The writers' counters right after a landing: a later read that moved them more than the engine's push does saw another writer.
#[derive(Clone, Copy, Debug)]
pub(super) struct Marks {
    begun: u64,
}

/// What the commit came to under the lock.
pub(super) enum Outcome {
    Refused(Refusal, String),
    Failed(&'static str, String),
    /// What landed, the token before it, and the writers' marks right after it (the push may move them by one).
    Landed(Landed, String, Marks),
}

/// `POST /changes`: a set that creates a project folder is `landing`'s, any other is judged as `answer_set` does.
pub async fn answer(c: &Call, ctx: &Ctx) -> Response {
    if c.body.get("create").is_some() {
        return super::landing::answer(c, ctx).await;
    }
    answer_set(c, ctx).await
}

/// The set judged without the disk's state, then the window, the journal's bracket and the lock, then the commit.
pub(super) async fn answer_set(c: &Call, ctx: &Ctx) -> Response {
    let req = match parse(&c.body) {
        Ok(r) => r,
        Err(r) => return refused(&r, None),
    };
    // A staged output is adopted only once its job ended with its value: a running or stopped job's file is half written.
    for op in &req.ops {
        if let k7_file_service::changes::Op::Adopt { job, .. } = op
            && !Table::global().finished(&ctx.account, job)
        {
            let why = format!("job '{job}' is not a finished job of yours");
            return refused(&Refusal::Bad(why), None);
        }
    }
    let (ctx2, at) = (ctx.clone(), req.at.clone());
    let resolved = blocking(move || target(&Space::open(&ctx2.home, &ctx2.root), &at)).await;
    let resolved = match resolved {
        Some(r) => r,
        None => return error(500, "internal error"),
    };
    // A project that is not the open one waits for the gates below; every other refusal of the target is plain.
    if let Err(r) = &resolved
        && !matches!(r, Refusal::Elsewhere { .. })
    {
        return refused(r, None);
    }
    let (ctx2, ops) = (ctx.clone(), req.ops.clone());
    let planned = blocking(move || plan(&Space::open(&ctx2.home, &ctx2.root), &ops)).await;
    let planned = match planned {
        Some(Ok(p)) => p,
        Some(Err(r)) => return refused(&r, None),
        None => return error(500, "internal error"),
    };
    // The gates and the parse come first: a set that is forbidden says so, whichever project is open.
    let target = match resolved {
        Ok(t) => t,
        Err(Refusal::Elsewhere { open }) => {
            let rev = open
                .as_ref()
                .and_then(|o| token(ctx, Some(&resolve(&ctx.root, o))));
            return refused(&Refusal::Elsewhere { open }, rev.as_deref());
        }
        Err(r) => return refused(&r, None),
    };
    if let Err(r) = confined_in(&target, &ctx.root, &planned, req.create.as_deref()) {
        return refused(&r, None);
    }
    let text: usize = planned
        .acts
        .iter()
        .map(|a| match a {
            Act::Text { text, .. } => text.len(),
            _ => 0,
        })
        .sum();
    let image = Journals::global().image_bytes(&gate_root(ctx));
    let Some(held) = ctx
        .gate
        .take(&ctx.account, put_weight(text, 0, image))
        .await
    else {
        return super::journal::busy();
    };
    let (c2, ctx3) = (c.clone(), ctx.clone());
    // A set that has begun runs to its end in a task of its own: a client that left strands no step (1736).
    let ran = tokio::spawn(async move { run(&c2, &ctx3, req, planned, target, held).await }).await;
    ran.unwrap_or_else(|_| error(500, "internal error"))
}

/// The project's token, walked now: what a base is compared with.
fn token(ctx: &Ctx, active: Option<&str>) -> Option<String> {
    let space = Space::open(&ctx.home, &ctx.root);
    Some(Revisions::global().token(&ctx.home, space.root(), active, active.is_some()))
}

async fn run(
    c: &Call,
    ctx: &Ctx,
    req: Request,
    planned: Planned,
    target: Target,
    held: Held,
) -> Response {
    let journal_dir = target.journal_dir(&ctx.root);
    // The workspace alone has no project to image: its sets are never a step.
    let mode = if target == Target::Workspace {
        Mode::Skip
    } else {
        req.journal
    };
    let spec = Spec {
        label: &req.label,
        mode,
        dir: &journal_dir,
    };
    let (ctx2, base, a2) = (ctx.clone(), req.base.clone(), target.clone());
    let lost = Outcome::Failed("EIO", "the write was lost".into());
    let texts: Vec<(String, String)> = planned
        .acts
        .iter()
        .filter_map(|a| match a {
            Act::Text { abs, text, .. } => Some((abs.clone(), text.clone())),
            _ => None,
        })
        .collect();
    let done = bracket::run(c, ctx, spec, lost, move |id| {
        let out = land(&ctx2, Revisions::global(), &planned, &base, id, &a2);
        let landed = match &out {
            Outcome::Landed(l, _, _) => Some(l.clone()),
            _ => None,
        };
        Edit {
            written: if landed.is_some() { texts } else { Vec::new() },
            out,
            landed,
        }
    })
    .await;
    drop(held);
    let Some(done) = done else {
        return error(500, "internal error");
    };
    match done.out {
        Outcome::Refused(r, rev) => refused(&r, Some(&rev)),
        Outcome::Failed(code, what) => {
            c.say("warn", &format!("[files] {what}"));
            let (status, msg) = io_words(Some(code));
            error(status, &msg)
        }
        Outcome::Landed(landed, before, marks) => {
            let ended = done.ended.unwrap_or_default();
            if let Some(why) = &ended.failure {
                c.say("warn", &format!("[files] native: journal failed: {why}"));
            }
            finish(c, ctx, &req, &target, (landed, before, marks), ended).await
        }
    }
}

/// The part of a landed set that needs the lock no more: the engine's push, the graph, the answer.
async fn finish(
    c: &Call,
    ctx: &Ctx,
    req: &Request,
    target: &Target,
    (landed, before, marks): (Landed, String, Marks),
    ended: k7_file_service::journal::Ended,
) -> Response {
    let mut rev = if ended.after.is_empty() {
        before.clone()
    } else {
        ended.after.clone()
    };
    // The workspace alone is journaled over a folder that is never there: its own token is what it answers.
    if target == &Target::Workspace
        && let Some(now) = token(ctx, None)
    {
        rev = now;
    }
    let inside = target
        .project_dir()
        .is_some_and(|d| landed.touched.iter().any(|a| is_within(d, a)));
    if req.reload && inside && rev != before {
        let root = Space::open(&ctx.home, &ctx.root).root().to_string();
        let revs = Revisions::global();
        let seen = revs.mutations(&root);
        super::journal::pushed(c).await;
        #[cfg(test)]
        tests_hooks::after_push(&root);
        // The push bumps the revision (the apply), so the base the next set is checked against is read after it. But
        // only when nothing else wrote meanwhile: a second tab's commit inside the push must not be taken in as ours.
        let alone = Journals::global().begun(&gate_root(ctx)) == marks.begun
            && revs.mutations(&root) <= seen + 1;
        if alone && let Some(now) = token(ctx, target.project_dir()) {
            rev = now;
        }
    }
    let mut out = json!({
        "revBefore": before,
        "rev": rev,
        "changed": ended.files.unwrap_or_default(),
    });
    if req.want_graph
        && let Some(graph) = docs::graph_value(ctx).await
        && let Some(o) = out.as_object_mut()
    {
        o.insert("graph".into(), graph);
    }
    if req
        .ops
        .iter()
        .any(|o| matches!(o, k7_file_service::changes::Op::Adopt { .. }))
    {
        let studio = ctx.home.studio.clone();
        let _ =
            blocking(move || jobs::sweep(&studio, std::time::SystemTime::now(), jobs::KEEP)).await;
    }
    json_answer(200, &out)
}

/// Under the lock: the token now against `base`, each `was`, then the landing; the pre-images of binaries go to the record.
pub(super) fn land(
    ctx: &Ctx,
    revs: &Revisions,
    planned: &Planned,
    base: &str,
    id: u64,
    target: &Target,
) -> Outcome {
    let space = Space::open(&ctx.home, &ctx.root);
    let dir = target.project_dir();
    let now = revs.token(&ctx.home, space.root(), dir, dir.is_some());
    // Only the open project has a token that holds still; a chain's or the workspace alone's freshness is each `was` and `read`.
    let base = if matches!(target, Target::Project(_)) {
        base
    } else {
        &now
    };
    if let Err(r) = fresh(planned, base, &now) {
        return Outcome::Refused(r, now);
    }
    let journals = Journals::global();
    match commit(&Real, planned, &mut |abs| journals.snapshot(id, abs)) {
        Ok(landed) => {
            if !landed.touched.is_empty() {
                revs.bump(space.root());
            }
            let marks = Marks {
                begun: journals.begun(&gate_root(ctx)),
            };
            Outcome::Landed(landed, now, marks)
        }
        Err(f) => Outcome::Failed(f.code, f.what),
    }
}

/// What a test does where the engine's push would: bump the revision as the apply does, or let another writer in.
#[cfg(test)]
pub(super) mod tests_hooks {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};

    type Hook = Box<dyn Fn() + Send>;

    fn hooks() -> &'static Mutex<HashMap<String, Hook>> {
        static H: OnceLock<Mutex<HashMap<String, Hook>>> = OnceLock::new();
        H.get_or_init(Default::default)
    }

    pub fn on_push(root: &str, f: impl Fn() + Send + 'static) {
        hooks()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(root.to_string(), Box::new(f));
    }

    pub fn after_push(root: &str) {
        if let Some(f) = hooks().lock().unwrap_or_else(|e| e.into_inner()).get(root) {
            f();
        }
    }
}
