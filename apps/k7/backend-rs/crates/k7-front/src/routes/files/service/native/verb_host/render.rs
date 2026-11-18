//! render.rs — `render` and `peak`: a doc bounced into the job's folder, a file's true peak (3f, M5).
//! What a reader gets wrong: the doc is judged whole before a job starts, every path field mapped through the gate (3f-5).

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use axum::response::Response;
use k7_file_service::verb_host::Error;
use k7_file_service::verb_host::docpaths::{Mapper, map_doc};
use k7_file_service::verb_host::files::job_files;
use k7_file_service::verb_host::jobs::Table;
use serde_json::{Value, json};
use song::doc::ProjectDoc;
use tokio::sync::oneshot;

use super::drive::{fail, gone, no_machine};
use super::{Checked, failure, now};
use crate::routes::files::json_answer;
use crate::routes::files::service::machine::engine_binary;
use crate::routes::files::service::native::jobs::Jobs;

/// A peak is measured within this, as the table says.
const PEAK_LIMIT: Duration = Duration::from_secs(60);

fn bad(message: &str) -> Response {
    failure(&Error::new(400, message))
}

/// `POST /verb-host/render {at, doc, stems?, base?}`: a job that bounces the doc into its own folder.
pub async fn render(req: &Checked, jobs: &Arc<Jobs>) -> Response {
    let stems = req
        .body
        .get("stems")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let base = match req.body.get("base") {
        None | Some(Value::Null) => None,
        Some(Value::String(b)) if b.is_empty() => None,
        Some(Value::String(b)) => match req.read(b) {
            Ok(real) => Some(real),
            Err(r) => return *r,
        },
        Some(_) => return bad("base must be a string"),
    };
    let Some(posted) = req.body.get("doc") else {
        return bad("doc is needed");
    };
    let mut doc: ProjectDoc = match serde_json::from_value(posted.clone()) {
        Ok(d) => d,
        Err(e) => return bad(&format!("doc: {e}")),
    };
    let mapper = Mapper {
        virt: &req.virt,
        base: base.as_deref(),
    };
    if let Err(e) = map_doc(&mapper, &mut doc) {
        return failure(&e);
    }
    if !jobs.renderer_available() {
        return no_machine("no engine machine to render on");
    }
    let doc = serde_json::to_value(&doc).unwrap_or(Value::Null);
    let run = match req.run() {
        Ok(r) => r,
        Err(r) => return *r,
    };
    let id = run.id.clone();
    let dir = match req.job_dir(&id) {
        Ok(d) => d,
        Err(r) => return *r,
    };
    let jobs = jobs.clone();
    let job = id.clone();
    tokio::spawn(async move {
        let cancel = run.cancel.clone();
        let (tx, rx) = oneshot::channel();
        let args = json!({ "outDir": dir, "doc": doc, "stems": stems, "base": base });
        jobs.ask_until(
            "render.run",
            args,
            Box::new(move |r| drop(tx.send(r))),
            gone(cancel),
        );
        match rx.await {
            Ok(Ok(answer)) => ended(&job, &dir, &answer["value"]),
            Ok(Err(e)) => fail(&job, &e, None),
            // The asker was stopped: the heartbeat already said so.
            Err(_) => {}
        }
        drop(run);
    });
    Checked::accepted(&id)
}

/// The job's end: the engine's manifest and the length of each file it named.
fn ended(job: &str, dir: &str, manifest: &Value) {
    let mut outs: Vec<String> = Vec::new();
    let mix = manifest["mix"].as_str().unwrap_or_default();
    let stems = manifest["stems"].as_array().into_iter().flatten();
    for name in std::iter::once(mix).chain(stems.filter_map(|s| s["file"].as_str())) {
        if !outs.iter().any(|o| o == name) {
            outs.push(name.to_string());
        }
    }
    match job_files(dir, &outs) {
        Ok(files) => {
            Table::global().finish(job, json!({ "manifest": manifest, "files": files }), now());
        }
        Err(e) => fail(job, &e.error, None),
    }
}

/// dBFS of a linear magnitude, floored at -100 as `toDb` floors it.
fn to_db(linear: f64) -> f64 {
    if linear > 0.0 {
        (20.0 * linear.log10()).max(-100.0)
    } else {
        -100.0
    }
}

/// `POST /verb-host/peak {file}`: the loudest sample on any channel, in dBFS.
pub async fn peak(req: &Checked) -> Response {
    let shown = match req.text("file") {
        Ok(f) => f.to_string(),
        Err(r) => return *r,
    };
    let real = match req.read(&shown) {
        Ok(r) => r,
        Err(r) => return *r,
    };
    if !Path::new(&real).is_file() {
        return failure(&Error::new(404, format!("no such file: {shown}")));
    }
    let docs = req.ctx.docs.as_ref();
    let engine = docs.and_then(|d| {
        let own = d.env.get("STUDIO_ENGINE").map(String::as_str);
        engine_binary(own, Path::new(&d.code_root))
    });
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let args = json!({ "source": real, "engineBin": engine.map(|e| e.display().to_string()) });
    let work = tokio::task::spawn_blocking(move || {
        crate::routes::files::service::native::media::direct::run_direct(
            "media.peak",
            &args,
            &|| flag.load(Ordering::Relaxed),
        )
    });
    let done = match tokio::time::timeout(PEAK_LIMIT, work).await {
        Ok(Ok(r)) => r,
        _ => {
            stop.store(true, Ordering::Relaxed);
            Err("the measure took longer than a minute".to_string())
        }
    };
    let measured = done.and_then(|v| match v["fail"]["message"].as_str() {
        Some(m) => Err(m.to_string()),
        None => v["facts"]["peak"]
            .as_f64()
            .ok_or_else(|| "no peak was measured".to_string()),
    });
    match measured {
        Ok(linear) => json_answer(200, &json!({ "db": to_db(linear) })),
        Err(m) => failure(&Error::new(
            500,
            format!("could not measure the peak of {shown}: {m}"),
        )),
    }
}
