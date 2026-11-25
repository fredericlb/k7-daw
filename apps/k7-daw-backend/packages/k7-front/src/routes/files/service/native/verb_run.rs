//! verb_run.rs — `POST /verb`: one command line run by the `verbs` crate on the open project, landed as one journal step.
//! What a reader gets wrong: a line that calls a route runs again from its start with the answer, and only its last run may land.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use axum::response::Response;
use k7_file_service::edits::verb::{Line, Ran, Replay, complete_verb, run_verb};
use k7_file_service::journal::{Journals, Mode};
use k7_file_service::revisions::Revisions;
use k7_file_service::space::Space;
use k7_file_service::verb_host::gate::virtual_for;
use k7_file_service::verb_paths::Virtual;
use serde_json::{Value, json};
use song::Cache;

use super::Ctx;
use super::bracket::{self, Edit, Spec};
use super::entries::blocking;
use super::journal::{error, gate_root, put_weight};
use super::verb_ask::Asker;
use crate::routes::files::json_answer;
use crate::routes::files::service::routes::Call;

/// The longest line a verb takes: a pasted `events add` is the only long one.
const MAX_LINE: usize = 1 << 20;

/// The most routes one line may call: a process over many files calls one per file.
const MAX_CALLS: usize = 256;

/// One compile cache per project, kept from one line to the next; a cache keyed by stamps cannot serve a file that moved.
fn caches() -> &'static Mutex<HashMap<String, Cache>> {
    static C: OnceLock<Mutex<HashMap<String, Cache>>> = OnceLock::new();
    C.get_or_init(Default::default)
}

/// The project's cache, taken out for the run; the map holds one project, the last one run.
fn take_cache(key: &str) -> Cache {
    let mut all = caches().lock().unwrap_or_else(|e| e.into_inner());
    let held = all.remove(key);
    all.clear();
    held.unwrap_or_default()
}

fn give_cache(key: String, cache: Cache) {
    caches()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(key, cache);
}

/// What a line came to under the lock, with the ids the run drew, replayed in order on its next run.
enum Outcome {
    Ran(Box<Ran>, Vec<String>),
    Failed(String),
}

fn mint() -> String {
    let mut b = [0u8; 16];
    let _ = getrandom::fill(&mut b);
    k7_file_service::media::models::uuid_v4(b)
}

/// One run that ended: the verb's answer and what landed, or the answer that ends the request.
struct Done {
    ran: Ran,
    minted: Vec<String>,
    files: Value,
}

/// `POST /verb {line}`: the verb's answer, with the token after it and the files its step changed.
pub async fn answer(c: &Call, ctx: &Ctx) -> Response {
    let Some(line) = c
        .body
        .get("line")
        .and_then(Value::as_str)
        .map(str::to_owned)
    else {
        return error(400, "a verb needs its 'line'");
    };
    if line.len() > MAX_LINE {
        return error(413, "the line is too long");
    }
    let Some(docs) = ctx.docs.clone() else {
        return error(503, "file service: no code root to read k7's modules from");
    };
    let code_root = docs.code_root.clone();
    let (home, root) = (ctx.home.clone(), ctx.root.clone());
    let active = blocking(move || Space::open(&home, &root).active()).await;
    let project = match active {
        Some(Ok(p)) => p,
        Some(Err(d)) => return error(409, &format!("declined: {}", d.as_str())),
        None => return error(500, "internal error"),
    };
    let virt: Option<Virtual> = match &project {
        Some(p) => {
            let (ctx2, p2) = (ctx.clone(), p.clone());
            blocking(move || {
                virtual_for(&ctx2.home, &ctx2.root, &p2, &docs.code_root, &docs.env).ok()
            })
            .await
            .flatten()
        }
        None => None,
    };
    let asker = Asker {
        reenter: c.reenter.as_ref(),
        account: &ctx.account,
        studio: &ctx.home.studio,
        at: project
            .as_deref()
            .and_then(|p| super::verb_host::shown_under(&ctx.root, p)),
        virt: virt.as_ref(),
    };
    let mut replay = Replay {
        answers: Vec::new(),
        stamp: Some(super::engine::filing::file_stamp()),
    };
    let mut minted = Vec::new();
    for _ in 0..=MAX_CALLS {
        let image = Journals::global().image_bytes(&gate_root(ctx));
        let Some(held) = ctx.gate.take(&ctx.account, put_weight(0, 0, image)).await else {
            return super::journal::busy();
        };
        let (c2, ctx2, l2, p2, r2) = (
            c.clone(),
            ctx.clone(),
            line.clone(),
            project.clone(),
            code_root.clone(),
        );
        let (replay2, minted2) = (replay.clone(), std::mem::take(&mut minted));
        // A line that has begun runs to its end in a task of its own: a client that left strands no step.
        let ran =
            tokio::spawn(async move { run(&c2, &ctx2, l2, p2, r2, replay2, minted2).await }).await;
        drop(held);
        let done = match ran {
            Ok(Ok(d)) => d,
            Ok(Err(r)) => return *r,
            Err(_) => return error(500, "internal error"),
        };
        let Some(pending) = done.ran.answer.pending.clone() else {
            return finish(c, &asker, done, project.is_some()).await;
        };
        minted = done.minted;
        let route = pending
            .get("route")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let args = pending.get("args").cloned().unwrap_or(Value::Null);
        let value = asker.call(&route, &args).await;
        replay
            .answers
            .push(json!({ "route": route, "args": args, "value": value }));
    }
    error(
        500,
        &format!("the verb called more than {MAX_CALLS} routes"),
    )
}

/// The landed run's answer: the engine pushed, the asks asked and their lines said, the step's files.
async fn finish(c: &Call, asker: &Asker<'_>, done: Done, open: bool) -> Response {
    let mut ran = done.ran;
    let pushes = ran
        .answer
        .effects
        .iter()
        .any(|e| e == "reload" || e == "applyProject");
    if ran.landed.is_some() && pushes && open {
        super::journal::pushed(c).await;
    }
    if !ran.answer.refused {
        for a in std::mem::take(&mut ran.answer.asks) {
            let route = a.get("route").and_then(Value::as_str).unwrap_or_default();
            let lines = asker
                .ask(route, a.get("args").unwrap_or(&Value::Null))
                .await;
            ran.answer.log.extend(lines);
        }
    }
    let mut out = serde_json::to_value(&ran.answer).unwrap_or_else(|_| json!({}));
    if let Some(o) = out.as_object_mut() {
        o.insert("changed".into(), done.files);
        o.insert("landed".into(), json!(ran.landed.is_some()));
    }
    json_answer(200, &out)
}

async fn run(
    c: &Call,
    ctx: &Ctx,
    line: String,
    project: Option<String>,
    code_root: String,
    replay: Replay,
    minted: Vec<String>,
) -> Result<Done, Box<Response>> {
    // No project open: the workspace alone has nothing to image, and a project-less verb is never a step.
    let (mode, dir) = match &project {
        Some(p) => (Mode::Step, p.clone()),
        None => (
            Mode::Skip,
            k7_file_service::paths::join(&[&ctx.root, ".no-project"]),
        ),
    };
    let spec = Spec {
        label: &line,
        mode,
        dir: &dir,
    };
    let (ctx2, l2, p2) = (ctx.clone(), line.clone(), project.clone());
    let lost = Outcome::Failed("the verb was lost".into());
    let done = bracket::run(c, ctx, spec, lost, move |id| {
        let key = format!("{}\n{}", ctx2.account, p2.as_deref().unwrap_or(""));
        let mut cache = take_cache(&key);
        let at = Line {
            home: &ctx2.home,
            root: &ctx2.root,
            code_root: &code_root,
            project: p2.as_deref(),
            line: &l2,
            replay,
        };
        let journals = Journals::global();
        let mut queue = minted.into_iter();
        let mut drawn = Vec::new();
        let mut draw = || {
            let id = queue.next().unwrap_or_else(mint);
            drawn.push(id.clone());
            id
        };
        let out = run_verb(&at, &mut cache, &mut draw, &mut |abs| {
            journals.snapshot(id, abs)
        });
        give_cache(key, cache);
        match out {
            Ok(ran) => {
                if ran.landed.as_ref().is_some_and(|l| !l.touched.is_empty()) {
                    Revisions::global().bump(Space::open(&ctx2.home, &ctx2.root).root());
                }
                Edit {
                    landed: ran.landed.clone(),
                    written: ran.written.clone(),
                    out: Outcome::Ran(Box::new(ran), drawn),
                }
            }
            Err(e) => Edit {
                out: Outcome::Failed(e),
                landed: None,
                written: Vec::new(),
            },
        }
    })
    .await;
    let Some(done) = done else {
        return Err(Box::new(error(500, "internal error")));
    };
    let (ran, minted) = match done.out {
        Outcome::Failed(what) => {
            c.say("warn", &format!("[files] verb: {what}"));
            let what = format!("the verb's writes did not land: {what}");
            return Err(Box::new(error(500, &what)));
        }
        Outcome::Ran(r, m) => (*r, m),
    };
    let ended = done.ended.unwrap_or_default();
    if let Some(why) = &ended.failure {
        c.say("warn", &format!("[files] native: journal failed: {why}"));
    }
    Ok(Done {
        ran,
        minted,
        files: json!(ended.files.unwrap_or_default()),
    })
}

/// `POST /verb/complete {line, cursor?, labelled?}`: `{items}`, each a whole line, bare or `{line, label}` when labelled.
pub async fn complete(c: &Call, ctx: &Ctx) -> Response {
    let Some(line) = c
        .body
        .get("line")
        .and_then(Value::as_str)
        .map(str::to_owned)
    else {
        return error(400, "a completion needs its 'line'");
    };
    if line.len() > MAX_LINE {
        return error(413, "the line is too long");
    }
    let cursor = c
        .body
        .get("cursor")
        .and_then(Value::as_u64)
        .and_then(|n| usize::try_from(n).ok());
    let labelled = c.body.get("labelled").and_then(Value::as_bool) == Some(true);
    let Some(code_root) = ctx.docs.as_ref().map(|d| d.code_root.clone()) else {
        return error(503, "file service: no code root to read k7's modules from");
    };
    let ctx2 = ctx.clone();
    let items = blocking(move || {
        let project = Space::open(&ctx2.home, &ctx2.root).active().ok().flatten();
        let key = format!("{}\n{}", ctx2.account, project.as_deref().unwrap_or(""));
        let mut cache = take_cache(&key);
        let at = Line {
            home: &ctx2.home,
            root: &ctx2.root,
            code_root: &code_root,
            project: project.as_deref(),
            line: &line,
            replay: Replay::default(),
        };
        let out = complete_verb(&at, cursor, &mut cache);
        give_cache(key, cache);
        out
    })
    .await;
    let items = match items {
        Some(Ok(items)) => items,
        Some(Err(e)) => return error(500, &e),
        None => return error(500, "internal error"),
    };
    let items: Vec<Value> = items
        .into_iter()
        .map(|i| match i.label.filter(|_| labelled) {
            Some(label) => json!({ "line": i.line, "label": label }),
            None => json!(i.line),
        })
        .collect();
    json_answer(200, &json!({ "items": items }))
}
