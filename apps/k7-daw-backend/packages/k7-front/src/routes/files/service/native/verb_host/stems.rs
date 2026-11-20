//! stems.rs — `playlist-stems`: each record of a playlist cut through the chain's steps, its stems written in `stems/`
//! beside it (3f, M5). What a reader gets wrong: a record is a workspace path that must stay inside the workspace through
//! its links (a bad one is `absent`), and one failure stays on its record while the loop goes on.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::response::Response;
use k7_file_service::jobs::separations::NO_ENGINE;
use k7_file_service::verb_host::Error;
use k7_file_service::verb_host::jobs::{Cancel, Table};
use k7_mediatek::media::is_audio;
use serde_json::{Value, json};

use super::cut::{Cut, End, cut_run, outputs_of};
use super::drive::no_machine;
use super::{Checked, failure, now};
use crate::routes::files::service::native::jobs::Jobs;

/// The most songs one ask holds, and the most steps a chain may have.
const SONGS: usize = 100;
const STEPS: usize = 8;

/// One chain step as the verb sends it: the node's name, the model, what it writes.
struct Step {
    name: String,
    model: String,
    outputs: Vec<String>,
}

fn bad(message: impl Into<String>) -> Response {
    failure(&Error::new(400, message))
}

/// What a record is on disk.
enum Found {
    Absent,
    Held,
    Master(PathBuf),
}

/// A record under the workspace: plain segments, an audio file, still inside it through the disk's links.
fn scan(space: &Path, record: &str) -> Found {
    let plain = |s: &str| !s.is_empty() && s != "." && s != ".." && !s.contains(['\\', '\0', ':']);
    if record.is_empty()
        || record.len() > 1024
        || !record.split('/').all(plain)
        || !is_audio(record)
    {
        return Found::Absent;
    }
    let file = record
        .split('/')
        .fold(space.to_path_buf(), |p, s| p.join(s));
    let (Ok(root), Ok(real)) = (std::fs::canonicalize(space), std::fs::canonicalize(&file)) else {
        return Found::Absent;
    };
    if !real.starts_with(&root) || !real.is_file() {
        return Found::Absent;
    }
    let stems = real.parent().map(|d| d.join("stems"));
    let held = stems
        .and_then(|d| std::fs::read_dir(d).ok())
        .is_some_and(|r| {
            r.filter_map(Result::ok)
                .any(|e| is_audio(&e.file_name().to_string_lossy()))
        });
    if held {
        Found::Held
    } else {
        Found::Master(real)
    }
}

fn steps_of(req: &Checked) -> Result<Vec<Step>, Box<Response>> {
    let list = req.body.get("steps").and_then(Value::as_array);
    let list = list.filter(|l| !l.is_empty() && l.len() <= STEPS);
    let list =
        list.ok_or_else(|| Box::new(bad(format!("steps must name one to {STEPS} steps"))))?;
    list.iter()
        .map(|s| {
            let name = s["step"]
                .as_str()
                .ok_or_else(|| Box::new(bad("a step needs its name")))?;
            let model = s["model"]
                .as_str()
                .ok_or_else(|| Box::new(bad("a step needs its model")))?;
            let outputs = outputs_of(s.get("outputs")).map_err(|m| Box::new(bad(m)))?;
            Ok(Step {
                name: name.to_string(),
                model: req.read(model)?,
                outputs,
            })
        })
        .collect()
}

/// `POST /verb-host/playlist-stems {songs, steps}`: a job that cuts each record of the workspace the list names.
pub async fn playlist(req: &Checked, jobs: &Arc<Jobs>) -> Response {
    let list = req.body.get("songs").and_then(Value::as_array);
    let names: Option<Vec<String>> = list
        .filter(|l| l.len() <= SONGS)
        .map(|l| {
            l.iter()
                .filter_map(|s| s.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .filter(|n| Some(n.len()) == list.map(Vec::len));
    let Some(names) = names else {
        return bad(format!("songs must be a list of at most {SONGS} names"));
    };
    let steps = match steps_of(req) {
        Ok(s) => s,
        Err(r) => return *r,
    };
    let songs = PathBuf::from(&req.ctx.root);
    if !jobs.bound() {
        return no_machine(NO_ENGINE);
    }
    let run = match req.run() {
        Ok(r) => r,
        Err(r) => return *r,
    };
    let id = run.id.clone();
    let (jobs, job, account) = (jobs.clone(), id.clone(), req.ctx.account.clone());
    tokio::spawn(async move {
        let cancel = run.cancel.clone();
        let mut done = Vec::with_capacity(names.len());
        for (i, song) in names.iter().enumerate() {
            let at = i as f64 / names.len() as f64;
            let share = 1.0 / names.len() as f64;
            Table::global().progress(&job, at, song);
            let work = format!("{account}/{job}/{i}");
            let shown = |f: f64, p: &str| Table::global().progress(&job, at + f * share, p);
            match one(&jobs, &songs, song, &steps, &work, &cancel, &shown).await {
                Ok(s) => done.push(s),
                Err(()) => return,
            }
        }
        Table::global().finish(&job, json!({ "songs": done }), now());
        drop(run);
    });
    Checked::accepted(&id)
}

/// One song's state; `Err` when the job was stopped while it ran.
async fn one(
    jobs: &Jobs,
    songs: &Path,
    song: &str,
    steps: &[Step],
    work: &str,
    cancel: &Cancel,
    progress: &(dyn Fn(f64, &str) + Sync),
) -> Result<Value, ()> {
    let state = |s: &str| Ok(json!({ "song": song, "state": s }));
    let scanned = {
        let (songs, song) = (songs.to_path_buf(), song.to_string());
        tokio::task::spawn_blocking(move || scan(&songs, &song))
            .await
            .map_err(|_| ())?
    };
    let master = match scanned {
        Found::Absent => return state("absent"),
        Found::Held => return state("held"),
        Found::Master(m) => m,
    };
    let out = master.parent().unwrap_or(Path::new("")).join("stems");
    if std::fs::symlink_metadata(&out).is_ok_and(|m| m.is_symlink()) {
        return Ok(
            json!({ "song": song, "state": "failed", "error": "its stems folder is a link" }),
        );
    }
    // The machine is told a plain drive path, never the `\\?\` form a canonical Windows path carries.
    let plain =
        |p: &Path| k7_file_service::confine::spelled(p).unwrap_or_else(|| p.display().to_string());
    let (source, out_dir) = (plain(&master), plain(&out));
    let mut files = Vec::new();
    for (n, step) in steps.iter().enumerate() {
        let cut = Cut {
            source: &source,
            model: &step.model,
            outputs: &step.outputs,
            out_dir: &out_dir,
        };
        match cut_run(jobs, &format!("{work}/{n}"), &cut, cancel, progress).await {
            Ok(names) => files.extend(names),
            Err(End::Stopped) => return Err(()),
            Err(End::Failed(e)) => {
                let error = format!("step '{}': {e}", step.name);
                return Ok(json!({ "song": song, "state": "failed", "error": error }));
            }
        }
    }
    Ok(json!({ "song": song, "state": "done", "files": files }))
}
