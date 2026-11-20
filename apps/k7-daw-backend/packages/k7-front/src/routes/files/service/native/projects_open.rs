//! projects_open.rs — `POST /open`: the libraries and the sync gate asked first, the plan and the marker under the
//! account's lock, the memory set by `activate`, the engine started after the lock (2033, 2034, 2053).
//! What a reader gets wrong: past its plan an open runs to its end in a task of its own, a client leaving mid-way or not.

use axum::response::Response;
use k7_file_service::confine::Disk;
use k7_file_service::edits::open::repair_open;
use k7_file_service::journal::Journals;
use k7_file_service::paths::resolve_all;
use k7_file_service::projects::marker::write_if_changed;
use k7_file_service::projects::open::{default_project, plan_open};
use k7_file_service::projects::{ProjError, info::workspace_info_opened, info_json};
use k7_file_service::revisions::Revisions;
use k7_file_service::space::Space;
use k7_file_service::workspaces::demos::demos_dir;
use serde_json::{Value, json};

use super::entries::{blocking, told};
use super::open_prepare::{Failure, prepare};
use super::projects::{Kind, Proj, error, finished, raw, refusal, uncaught};
use super::{Tag, locks, workspaces_memo as memo};
use crate::routes::files::service::body;
use crate::routes::files::service::live;

/// The project each workspace root was last opened as in this process: the TypeScript's `opened`.
static OPENED: std::sync::Mutex<Vec<(String, String)>> = std::sync::Mutex::new(Vec::new());

/// Whether this process opened `root` as `abs` last.
fn was_opened(root: &str, abs: &str) -> bool {
    let all = OPENED.lock().unwrap_or_else(|p| p.into_inner());
    all.iter().any(|(r, a)| r == root && a == abs)
}

/// `root` noted as opened as `abs`, or forgotten with `None` (a removed workspace).
pub(super) fn opened(root: &str, abs: Option<&str>) {
    let mut all = OPENED.lock().unwrap_or_else(|p| p.into_inner());
    all.retain(|(r, _)| r != root);
    if let Some(abs) = abs {
        all.push((root.to_string(), abs.to_string()));
    }
}

/// A moved or recycled folder under `abs` is no longer the project `root` has open.
pub(super) fn moved_away(root: &str, abs: &str) {
    let mut all = OPENED.lock().unwrap_or_else(|p| p.into_inner());
    let under = |a: &str| a == abs || a.starts_with(&format!("{abs}/"));
    all.retain(|(r, a)| r != root || !under(a));
}

/// The project a bare `.` names, or the path itself; `None` is a workspace that opens as a tree.
async fn wanted(p: &Proj, root: &str, path: &str) -> Result<Option<String>, ()> {
    if path != "." {
        return Ok(Some(path.to_string()));
    }
    let (home, r2, limits) = (p.home.clone(), root.to_string(), p.limits());
    let found = blocking(move || default_project(&Space::open(&home, &r2), &limits)).await;
    match found {
        Some(Ok(w)) => Ok(w),
        Some(Err(d)) => {
            p.declined(Tag::Open, d.as_str());
            Err(())
        }
        None => Err(()),
    }
}

/// `POST /open`, answered natively or `None` where it declined (a decline, before anything is written).
pub(super) async fn open(p: &Proj, name: &str, root: &str, body: &Value) -> Option<Response> {
    let want = wanted(p, root, &body::text(body, "path")).await.ok()?;
    // The libraries and the pins come before the lock and the task: nothing native is begun (2055).
    match prepare(p, root).await {
        Ok(()) => {}
        Err(Failure::Declined(why)) => {
            p.declined(Tag::Open, why);
            return None;
        }
        Err(Failure::Lost) => return Some(uncaught(p, name, Kind::Open)),
    }
    let Some(want) = want else {
        return Some(finish(p, root, false, None).await);
    };
    // The journal's gate first, then the lock, as every native writer: an open record must not absorb a repair.
    let gate = Journals::global().exclusive(&resolve_all(&[root])).await;
    let lock = locks::write(&p.home.studio).await;
    let (home, r2) = (p.home.clone(), root.to_string());
    let planned = blocking(move || plan_open(&Space::open(&home, &r2), &want, &Disk)).await?;
    let plan = match planned {
        Err(ProjError::Declined(d)) => {
            p.declined(Tag::Open, d.as_str());
            return None;
        }
        Err(e) => return Some(refusal(p, &e)),
        Ok(plan) => plan,
    };
    let (p, root) = (p.clone(), root.to_string());
    finished(async move {
        // The project this process has open leaves the marker as written, whatever it says, as `isOpenAs` did (2022).
        let held = was_opened(&root, &plan.abs);
        if !held {
            let (r2, rel) = (root.clone(), plan.rel.clone());
            match blocking(move || write_if_changed(&r2, &rel)).await {
                Some(Ok(_)) => {}
                Some(Err(e)) => return Some(unwritten(&p, &e)),
                None => return Some(error(500, "internal error")),
            }
        }
        memo::dropped(&p.account);
        opened(&root, Some(&plan.abs));
        // The repairs and the stamp are native (C9): they run under this lock when the marker moved.
        if !held {
            repaired(&p, &root, &plan).await;
        }
        drop((gate, lock));
        Some(finish(&p, &root, true, held.then_some(plan.rel.as_str())).await)
    })
    .await
}

/// The repairs and the seconds stamp of an open that moved the marker, then the opened project's waveforms warmed.
async fn repaired(p: &Proj, root: &str, plan: &k7_file_service::projects::open::OpenPlan) {
    let (home, code) = (p.home.clone(), p.demos_from.1.clone());
    let (abs, rel, r2) = (plan.abs.clone(), plan.rel.clone(), root.to_string());
    let done = blocking(move || {
        let mut mint = || {
            let mut b = [0u8; 16];
            let _ = getrandom::fill(&mut b);
            k7_file_service::media::models::uuid_v4(b)
        };
        repair_open(&home, &r2, &code, &abs, &rel, &mut mint)
    })
    .await;
    if let Some(done) = done {
        if let Some(why) = &done.skipped {
            p.say("info", &format!("[files] open: repairs skipped, {why}"));
        }
        for n in &done.notes {
            p.say(if n.warn { "warn" } else { "info" }, &n.text);
        }
        if done.touched {
            Revisions::global().bump(Space::open(&p.home, root).root());
            told(root, "after").await;
        }
    }
    // The waveforms of the opened project are warmed, as the stand-in's prewarm did; none without an active project.
    if let Some(site) = &p.site {
        let (home, r2) = (p.home.clone(), root.to_string());
        let asked = blocking(move || super::docs::asked_in(&home, &r2)).await;
        if let Some(Ok(asked)) = asked
            && asked.active.is_some()
        {
            site.prewarm(root, asked);
        }
    }
}

/// A marker that could not be written fails the open: every native reader would name the old project (2093).
fn unwritten(p: &Proj, e: &ProjError) -> Response {
    p.say("warn", &format!("[files] {e}"));
    match e.code() {
        Some(code) => error(500, &format!("the file could not be changed ({code})")),
        None => error(500, "internal error"),
    }
}

/// The engine started outside the lock (2053), then the info walked afresh and answered as node spells it (2032).
async fn finish(p: &Proj, root: &str, start: bool, held: Option<&str>) -> Response {
    let (load, rev) = match &p.engine {
        Some(e) => {
            #[cfg(test)]
            super::entries::calls::gate(root, "startEngine").await;
            let rev = e.start_engine(root, start).await;
            let load = match start {
                true => link_load(e, root).await,
                false => e.load_now(root).await,
            };
            (load, rev)
        }
        None => (Value::Null, Value::Null),
    };
    let tail = format!("\"engineLoad\":{load},\"rev\":{rev}");
    let ((home, r2), (env, code), limits) = (
        (p.home.clone(), root.to_string()),
        p.demos_from.clone(),
        p.limits(),
    );
    let walked = blocking(move || {
        let demos = demos_dir(env.as_deref(), &code);
        workspace_info_opened(&Space::open(&home, &r2), demos.as_deref(), &limits)
    })
    .await;
    match walked {
        Some(Ok(mut info)) => {
            // The memory's project is the answer where it kept the marker as written.
            if let Some(rel) = held {
                info.active = Some(rel.to_string());
            }
            live::as_node_answers(raw(200, info_json(&info, &tail))).await
        }
        Some(Err(d)) => {
            p.declined(Tag::Open, d.as_str());
            super::refuse::response(d.as_str())
        }
        None => error(500, "internal error"),
    }
}

/// The load the open began, read from the link once the start queued by the bundle has run its first step; bounded, so a
/// busy queue answers what it holds.
pub(super) async fn link_load(e: &super::engine::routes::Engine, root: &str) -> Value {
    let s = e.link.session(Some(root));
    let active = {
        let (home, r) = (e.host.home_now(), root.to_string());
        blocking(move || Space::open(&home, &r).active().ok().flatten())
            .await
            .flatten()
    };
    for _ in 0..200 {
        // Another project's start still queued leaves `serving` on the old one: wait for the load or for this project.
        if s.load_phase().is_some() || (active.is_some() && s.serving() == active) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    s.engine_load_of(active.as_deref())
        .map_or(Value::Null, |p| json!(p.as_str()))
}
