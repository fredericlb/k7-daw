//! verb_host — the host members' routes under `/verb-host` (3f): the gates every one passes, then the driver that answers it.
//! What a reader gets wrong: a route whose driver is absent answers 503 `off`; a path is judged by the member, here.

pub mod account;
pub mod cut;
pub mod devices;
pub mod drive;
pub mod engine_ops;
pub mod media;
pub mod member;
pub mod onnx;
pub mod picture;
pub mod plugins;
pub mod prepare;
pub mod render;
pub mod stems;
pub mod take;
pub mod transcribe;

use std::future::Future;
use std::pin::Pin;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use axum::response::Response;
use k7_file_service::paths::resolve;
use k7_file_service::space::Space;
use k7_file_service::verb_host::Error;
use k7_file_service::verb_host::gate::{out_dir, read_path, virtual_for};
use k7_file_service::verb_host::jobs::{Cancel, Table};
use k7_file_service::verb_paths::Virtual;
use serde_json::{Value, json};
use song::roots::Roots;

pub use drive::production_in;
pub use member::Member;

use super::Ctx;
use super::answering::CALL_WEIGHT;
use super::entries::blocking;
use crate::routes::files::json_answer;
use crate::routes::files::service::body;
use crate::routes::files::service::routes::Call;

pub type Answer<'a> = Pin<Box<dyn Future<Output = Response> + Send + 'a>>;

/// What answers a member once its gates pass; a front with none answers 503 `off`.
pub trait Driver: Send + Sync {
    fn call<'a>(&'a self, req: &'a Checked) -> Answer<'a>;
}

/// The route's turned clock: the job table is handed this, never a clock of its own.
pub fn now() -> Duration {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed()
}

/// How often the heartbeat is judged.
const TICK: Duration = Duration::from_secs(5);

static TICKER: std::sync::Mutex<Option<tokio::task::JoinHandle<()>>> = std::sync::Mutex::new(None);

/// One task judges every job's heartbeat; it is respawned when the runtime that made it has gone.
fn ensure_ticker() {
    let mut held = TICKER.lock().unwrap_or_else(|p| p.into_inner());
    if held.as_ref().is_some_and(|h| !h.is_finished()) {
        return;
    }
    *held = Some(tokio::spawn(async {
        loop {
            tokio::time::sleep(TICK).await;
            Table::global().tick(now());
        }
    }));
}

pub fn failure(e: &Error) -> Response {
    json_answer(e.status, &e.body())
}

fn plain(status: u16, message: &str) -> Response {
    failure(&Error::new(status, message))
}

/// A request that passed the gates: the open project, the roots a path is read under, and what the member was asked.
pub struct Checked {
    pub member: Member,
    pub body: Value,
    pub query: String,
    pub ctx: Ctx,
    virt: Virtual,
}

impl Checked {
    /// The real path of a virtual one the person reads, or the 403 answer.
    pub fn read(&self, shown: &str) -> Result<String, Box<Response>> {
        read_path(&self.virt, shown).map_err(|e| Box::new(failure(&e)))
    }

    /// The real path of a virtual one under a root, for a folder the person named; none outside every root.
    pub fn real(&self, shown: &str) -> Option<String> {
        self.virt.real(shown)
    }

    /// A text field of the body, or the 400 answer.
    pub fn text(&self, key: &str) -> Result<&str, Box<Response>> {
        self.body
            .get(key)
            .and_then(Value::as_str)
            .ok_or_else(|| Box::new(plain(400, &format!("{key} must be a string"))))
    }

    /// Starts a job of this member; its id and the flag a stop sets.
    pub fn start(&self) -> Result<(String, Cancel), Box<Response>> {
        ensure_ticker();
        Table::global()
            .start(&self.ctx.account, self.member.heartbeat(), now())
            .map_err(|e| Box::new(failure(&e)))
    }

    /// Starts a job and hands back the driver's hold on it.
    pub fn run(&self) -> Result<drive::Run, Box<Response>> {
        let (id, cancel) = self.start()?;
        // A job's folder nobody landed is swept a day on, whether or not another change set ever adopts.
        let studio = self.ctx.home.studio.clone();
        drop(tokio::task::spawn_blocking(move || {
            k7_file_service::changes::jobs::sweep(
                &studio,
                std::time::SystemTime::now(),
                k7_file_service::changes::jobs::KEEP,
            )
        }));
        Ok(drive::Run::new(id, cancel, self.ctx.home.studio.clone()))
    }

    /// The folder a job writes its outputs to, made.
    pub fn job_dir(&self, job: &str) -> Result<String, Box<Response>> {
        out_dir(&self.ctx.home.studio, job).map_err(|e| Box::new(failure(&e)))
    }

    /// The 202 answer of a started job.
    pub fn accepted(job: &str) -> Response {
        json_answer(202, &json!({ "job": job }))
    }
}

/// The project's place under the root, or None: a path outside it is never shown.
pub(super) fn shown_under(root: &str, active: &str) -> Option<String> {
    active
        .strip_prefix(&format!(
            "{}/",
            root.replace('\\', "/").trim_end_matches('/')
        ))
        .map(str::to_string)
}

/// The open project's folder, or the answer that says why not.
async fn open_project(ctx: &Ctx, at: Option<&str>) -> Result<String, Box<Response>> {
    let (home, root) = (ctx.home.clone(), ctx.root.clone());
    let active = match blocking(move || Space::open(&home, &root).active()).await {
        Some(Ok(Some(a))) => a,
        Some(Ok(None)) => {
            return Err(Box::new(json_answer(
                409,
                &json!({ "error": "no project is open", "open": null }),
            )));
        }
        Some(Err(d)) => return Err(Box::new(plain(503, &format!("declined: {}", d.as_str())))),
        None => return Err(Box::new(plain(500, "internal error"))),
    };
    match at {
        Some(at) if resolve(&ctx.root, at) != active => {
            let open = shown_under(&ctx.root, &active);
            Err(Box::new(json_answer(
                409,
                &json!({ "error": format!("'{at}' is not the open project"), "open": open }),
            )))
        }
        _ => Ok(active),
    }
}

/// `GET /verb-host/job?id=`: the job's state, to its account alone.
fn poll(c: &Call, ctx: &Ctx) -> Response {
    let id = body::query(&c.query, "id");
    match Table::global().poll(&ctx.account, &id, now()) {
        Some(v) => json_answer(200, &v),
        None => plain(404, "no such job"),
    }
}

/// A host route: the shape, the project, the weight, then the driver; the poll answers itself.
pub async fn answer(c: &Call, ctx: &Ctx, member: Member) -> Response {
    if member == Member::Job {
        return poll(c, ctx);
    }
    let Some(driver) = ctx.verbs.clone() else {
        return failure(&Error::coded(
            503,
            "this host member is not served here",
            "off",
        ));
    };
    let Some(docs) = ctx.docs.clone() else {
        return failure(&Error::coded(
            503,
            "the host members need the native song compiler",
            "off",
        ));
    };
    let at = if member.names_project() {
        match c.body.get("at").and_then(Value::as_str) {
            Some(a) if !a.is_empty() => Some(a),
            _ => return plain(400, "at must be a string"),
        }
    } else {
        None
    };
    // A member that reads no path asks no project to be open and holds no share of the gate.
    let (virt, _held) = if member.reads_paths() {
        let active = match open_project(ctx, at).await {
            Ok(a) => a,
            Err(r) => return *r,
        };
        let Some(held) = ctx.gate.take(&ctx.account, CALL_WEIGHT).await else {
            return failure(&Error::coded(503, "file service: busy, ask again", "busy"));
        };
        let ctx2 = ctx.clone();
        let virt = blocking(move || {
            virtual_for(&ctx2.home, &ctx2.root, &active, &docs.code_root, &docs.env)
        })
        .await;
        match virt {
            Some(Ok(v)) => (v, Some(held)),
            Some(Err(e)) => return failure(&e),
            None => return plain(500, "internal error"),
        }
    } else {
        let none = Roots::default();
        let virt = Virtual::new(&ctx.root, &[], &docs.code_root, &ctx.home.studio, &none);
        (virt, None)
    };
    let checked = Checked {
        member,
        body: c.body.clone(),
        query: c.query.clone(),
        ctx: ctx.clone(),
        virt,
    };
    driver.call(&checked).await
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_account;
#[cfg(test)]
mod tests_account_merge;
#[cfg(test)]
mod tests_drive;
#[cfg(test)]
mod tests_media;
#[cfg(test)]
mod tests_picture;
#[cfg(test)]
mod tests_prepare;
#[cfg(test)]
mod tests_take;
#[cfg(test)]
mod tests_transcribe;
