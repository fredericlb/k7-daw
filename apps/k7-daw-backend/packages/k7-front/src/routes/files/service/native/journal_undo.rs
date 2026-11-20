//! journal_undo.rs — `POST /undo` and `/redo` answered natively: the peek, the audio fetched before any gate (1743), the gate alone then
//! the account's lock (1733), the step applied by the crate and the memos told after (1770). What a reader gets wrong: it never declines.

use axum::response::Response;
use k7_file_service::atomic::Real;
use k7_file_service::confine::Disk;
use k7_file_service::filter::rename_retry;
use k7_file_service::journal::apply::Writer;
use k7_file_service::journal::blobs::BlobError;
use k7_file_service::journal::{Failure, Journals, Undo, Undone};
use k7_file_service::revisions::Revisions;
use k7_file_service::space::Space;
use serde_json::{Map, Value, json};

use super::entries::{blocking, finished, told};
use super::journal::{active_of, busy, count_lossy, error, gate_root, pushed, weigh};
use super::{Ctx, Tag, locks};
use crate::routes::files::json_answer;
use crate::routes::files::service::body::Field;
use crate::routes::files::service::routes::{self, Call};

/// How long an undo waits for the records in flight before it answers a 409 (1799, approved by the user).
const GATE_WAIT: std::time::Duration = std::time::Duration::from_secs(10);

/// The wait for a workspace's gate: a test shortens it for its own root only.
fn gate_wait(root: &str) -> std::time::Duration {
    #[cfg(test)]
    if let Some(d) = tests_wait::of(root) {
        return d;
    }
    let _ = root;
    GATE_WAIT
}

/// How often an undo starts again when the step or its blobs moved under it (1743).
const TRIES: usize = 3;

/// `POST /undo` and `/redo`, a step applied alone behind every record in flight (1770, 1730).
pub async fn step(c: &Call, ctx: &Ctx) -> Response {
    if let Some(r) = routes::invalid(c, &[("confirm", Field::OptBool)]) {
        return r;
    }
    let confirm = c.body.get("confirm") == Some(&Value::Bool(true));
    let active = match active_of(c, ctx).await {
        Ok(a) => a,
        Err(r) => return *r,
    };
    let Some(active) = active else {
        return json_answer(200, &json!({ "applied": false }));
    };
    let Some(held) = weigh(ctx, &active).await else {
        return busy();
    };
    let (c2, ctx2) = (c.clone(), ctx.clone());
    // A step that has passed its peek runs to its end in a task of its own (1736).
    finished(async move {
        let _held = held;
        stepped(&c2, &ctx2, &active, confirm).await
    })
    .await
}

/// A step answered as the TypeScript's route does: an audio's sentence as a 409 (1790), any other failure a 500.
fn failure(c: &Call, what: &str, f: &Failure) -> Response {
    match f {
        Failure::Blob(e) => error(409, &e.to_string()),
        Failure::Apply(e) => broken(c, what, &e.to_string()),
        Failure::Journal(e) => broken(c, what, e),
    }
}

fn broken(c: &Call, what: &str, why: &str) -> Response {
    c.say(
        "error",
        &format!("[{}] {what} failed: Error: {why}", c.workspace),
    );
    error(500, "internal error")
}

/// One blob fetched back from the engine that keeps it, before any gate (1743): `historyKeeper.ts`' `send-up`.
async fn fetch(c: &Call, m: &k7_file_service::journal::blobs::Missing) -> Result<(), BlobError> {
    let binding = c.engine.as_ref().map(|e| e.link.binding());
    let Some(b) = binding.filter(|b| b.engine.is_some()) else {
        return Err(m.gone());
    };
    if b.engine.as_ref().map(|(id, _)| id.as_str()) != Some(m.kept_on.id.as_str()) {
        let (path, kept) = (m.path.clone(), m.kept_on.name.clone());
        return Err(BlobError::KeptElsewhere { path, kept });
    }
    let Some(url) = b.jobs_url else {
        return Err(m.gone());
    };
    match send_up(&url, &m.file).await {
        Ok(()) if std::path::Path::new(&m.file).exists() => Ok(()),
        _ => Err(m.gone()),
    }
}

/// The `send-up` job, settled by `sent`: the machine uploads its copy of `file` back here.
async fn send_up(url: &str, file: &str) -> Result<(), String> {
    use crate::routes::engine_reach::job::{JOB_IDLE, JOB_TOTAL, JobChannel, of_type, request_job};
    let mut chan = super::engine::jobs::WsChannel::open(url).await?;
    let body = json!({ "type": "send-up", "file": file })
        .as_object()
        .cloned()
        .unwrap_or_default();
    let done = request_job(
        &mut chan,
        body,
        of_type("sent"),
        |_| {},
        JOB_TOTAL,
        JOB_IDLE,
    )
    .await;
    chan.close();
    done.map(|_| ()).map_err(|e| e.0)
}

async fn stepped(c: &Call, ctx: &Ctx, active: &str, confirm: bool) -> Response {
    let (redo, what) = (ctx.tag == Tag::Redo, ctx.tag.route());
    let (journals, root) = (Journals::global(), gate_root(ctx));
    for attempt in 0..=TRIES {
        let (home, r2, dir) = (ctx.home.clone(), root.clone(), active.to_string());
        let peeked = blocking(move || journals.peek_of(&home, &r2, &dir, redo)).await;
        count_lossy(ctx);
        let Some(Some(peek)) = peeked else {
            return json_answer(200, &json!({ "applied": false }));
        };
        let process = peek.get("kind") == Some(&json!("process"));
        if process && !confirm {
            let mut out = Map::new();
            out.insert("needsConfirm".into(), Value::Bool(true));
            out.extend(peek.as_object().cloned().unwrap_or_default());
            return json_answer(200, &Value::Object(out));
        }
        let (home, r2, dir) = (ctx.home.clone(), root.clone(), active.to_string());
        let planned = blocking(move || journals.plan_kept(&home, &r2, &dir, redo)).await;
        let planned = match planned {
            Some(Ok(p)) => p,
            Some(Err(e)) => return error(409, &e.to_string()),
            None => return error(500, "internal error"),
        };
        let mut fetched = Vec::new();
        // The last try fetches nothing: it answers as the step stands at that moment (1743).
        if attempt < TRIES {
            for m in &planned.missing {
                if let Err(e) = fetch(c, m).await {
                    return error(409, &e.to_string());
                }
                fetched.push(m.name.clone());
            }
        }
        let space_root = ctx.root.clone();
        let home = ctx.home.clone();
        // The gate alone first, the account's lock after it, around the writes (1733).
        let Ok(alone) = tokio::time::timeout(gate_wait(&root), journals.exclusive(&root)).await
        else {
            return error(409, "a command is still running on this workspace");
        };
        let lock = locks::write(&ctx.home.studio).await;
        let (home2, root2, dir2) = (home.clone(), space_root.clone(), active.to_string());
        let expect = (attempt < TRIES).then_some(planned.id).flatten();
        let kept = planned.kept.clone();
        let gate = root.clone();
        let done = blocking(move || {
            let space = Space::open(&home2, &root2);
            let confine = |rel: &str| space.safe_path(rel, &Disk).map_err(|e| e.to_string());
            let writer = Writer {
                fs: &Real,
                retry: rename_retry(),
                sleep: &std::thread::sleep,
            };
            journals.undo_held(&Undo {
                home: &home2,
                root: &gate,
                dir: &dir2,
                redo,
                expect,
                bound: kept,
                confine: &confine,
                writer: &writer,
                fetched: &fetched,
            })
        })
        .await;
        match done {
            Some(Ok(Undone::Moved(peek))) => {
                Revisions::global().bump(&gate_root(ctx));
                drop((lock, alone));
                told(&c.root, "after").await;
                count_lossy(ctx);
                pushed(c).await;
                let mut out = Map::new();
                out.insert("applied".into(), Value::Bool(true));
                out.extend(peek.as_object().cloned().unwrap_or_default());
                return json_answer(200, &Value::Object(out));
            }
            Some(Ok(Undone::Nothing)) => {
                return json_answer(200, &json!({ "applied": false }));
            }
            Some(Ok(Undone::Retry(missing))) => {
                drop((lock, alone));
                if attempt == TRIES {
                    return match missing.first() {
                        Some(m) => error(409, &m.gone().to_string()),
                        None => error(500, "internal error"),
                    };
                }
            }
            Some(Err(f)) => {
                drop((lock, alone));
                count_lossy(ctx);
                return failure(c, what, &f);
            }
            None => return error(500, "internal error"),
        }
    }
    error(500, "internal error")
}

#[cfg(test)]
pub(super) mod tests_wait {
    use std::sync::Mutex;
    use std::time::Duration;

    static SHORT: Mutex<Vec<(String, Duration)>> = Mutex::new(Vec::new());

    pub fn set(root: &str, d: Duration) {
        SHORT
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push((root.to_string(), d));
    }

    pub fn of(root: &str) -> Option<Duration> {
        let all = SHORT.lock().unwrap_or_else(|p| p.into_inner());
        all.iter().find(|(r, _)| r == root).map(|(_, d)| *d)
    }
}
