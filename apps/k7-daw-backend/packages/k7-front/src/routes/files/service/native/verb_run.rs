//! verb_run.rs — `POST /verb`: one command line run natively by the `verbs` crate on the open project, landed as one journal step.
//! What a reader gets wrong: the verb reads and writes under the account's lock, so no base is checked; a refusal is a 200 that wrote nothing.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use axum::response::Response;
use k7_file_service::edits::verb::{Line, Ran, run_verb};
use k7_file_service::journal::{Journals, Mode};
use k7_file_service::revisions::Revisions;
use k7_file_service::space::Space;
use serde_json::{Value, json};
use song::Cache;

use super::Ctx;
use super::bracket::{self, Edit, Spec};
use super::entries::blocking;
use super::journal::{error, gate_root, put_weight};
use crate::routes::files::json_answer;
use crate::routes::files::service::routes::Call;

/// The longest line a verb takes: a pasted `events add` is the only long one.
const MAX_LINE: usize = 1 << 20;

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

/// What a line came to under the lock.
enum Outcome {
    Ran(Ran),
    Failed(String),
}

fn mint() -> String {
    let mut b = [0u8; 16];
    let _ = getrandom::fill(&mut b);
    k7_file_service::media::models::uuid_v4(b)
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
    let Some(code_root) = ctx.docs.as_ref().map(|d| d.code_root.clone()) else {
        return error(503, "file service: no code root to read k7's modules from");
    };
    let (home, root) = (ctx.home.clone(), ctx.root.clone());
    let active = blocking(move || Space::open(&home, &root).active()).await;
    let project = match active {
        Some(Ok(p)) => p,
        Some(Err(d)) => return error(409, &format!("declined: {}", d.as_str())),
        None => return error(500, "internal error"),
    };
    let image = Journals::global().image_bytes(&gate_root(ctx));
    let Some(held) = ctx.gate.take(&ctx.account, put_weight(0, 0, image)).await else {
        return super::journal::busy();
    };
    let (c2, ctx2) = (c.clone(), ctx.clone());
    // A line that has begun runs to its end in a task of its own: a client that left strands no step.
    let ran = tokio::spawn(async move { run(&c2, &ctx2, line, project, code_root).await }).await;
    drop(held);
    ran.unwrap_or_else(|_| error(500, "internal error"))
}

async fn run(
    c: &Call,
    ctx: &Ctx,
    line: String,
    project: Option<String>,
    code_root: String,
) -> Response {
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
        };
        let journals = Journals::global();
        let mut mint = mint;
        let out = run_verb(&at, &mut cache, &mut mint, &mut |abs| {
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
                    out: Outcome::Ran(ran),
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
        return error(500, "internal error");
    };
    let ran = match done.out {
        Outcome::Failed(what) => {
            c.say("warn", &format!("[files] verb: {what}"));
            return error(500, &format!("the verb's writes did not land: {what}"));
        }
        Outcome::Ran(r) => r,
    };
    let ended = done.ended.unwrap_or_default();
    if let Some(why) = &ended.failure {
        c.say("warn", &format!("[files] native: journal failed: {why}"));
    }
    let pushes = ran
        .answer
        .effects
        .iter()
        .any(|e| e == "reload" || e == "applyProject");
    if ran.landed.is_some() && pushes && project.is_some() {
        super::journal::pushed(c).await;
    }
    let mut out = serde_json::to_value(&ran.answer).unwrap_or_else(|_| json!({}));
    if let Some(o) = out.as_object_mut() {
        o.insert("changed".into(), json!(ended.files.unwrap_or_default()));
        o.insert("landed".into(), json!(ran.landed.is_some()));
    }
    json_answer(200, &out)
}
