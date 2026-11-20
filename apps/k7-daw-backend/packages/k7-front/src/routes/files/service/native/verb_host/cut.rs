//! cut.rs — `separate`: one source cut into stems on the bound machine, polled from the account's registry (3f, M5).
//! What a reader gets wrong: each job is its own work (the registry's key carries the job), so two asks never share one
//! record, and a cut's files are only ever the job's own folder's.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use axum::response::Response;
use k7_file_service::jobs::separations::NO_ENGINE;
use k7_file_service::verb_host::Error;
use k7_file_service::verb_host::files::job_files;
use k7_file_service::verb_host::jobs::{Cancel, Table};
use serde_json::{Value, json};

use super::drive::{fail, no_machine};
use super::{Checked, failure, now};
use crate::routes::files::service::native::jobs::Jobs;

/// What a module may declare it writes: the four stems or the instrumental.
const OUTPUTS: [&str; 5] = ["drums", "bass", "other", "vocals", "instrumental"];

/// How often a cut's record is read.
const POLL: Duration = Duration::from_millis(500);

fn bad(message: String) -> Response {
    failure(&Error::new(400, message))
}

/// The `outputs` of a body: some of `OUTPUTS`, the four stems when absent.
pub fn outputs_of(v: Option<&Value>) -> Result<Vec<String>, String> {
    let Some(v) = v.filter(|v| !v.is_null()) else {
        return Ok(OUTPUTS[..4].iter().map(|s| s.to_string()).collect());
    };
    let list = v.as_array().ok_or("outputs must be a list")?;
    let names: Vec<String> = list
        .iter()
        .filter_map(|s| s.as_str().map(str::to_string))
        .collect();
    if names.len() != list.len() || names.is_empty() || names.len() > OUTPUTS.len() {
        return Err("outputs must name one to five outputs".into());
    }
    match names.iter().find(|n| !OUTPUTS.contains(&n.as_str())) {
        Some(n) => Err(format!("'{n}' is no output a separation writes")),
        None => Ok(names),
    }
}

/// How a cut ended other than by writing its files.
pub enum End {
    Failed(String),
    Stopped,
}

/// Cuts `source` through `model` into `out_dir` and answers the files it wrote, as names; `progress` hears the fraction.
pub async fn cut_run(
    jobs: &Jobs,
    work: &str,
    cut: &Cut<'_>,
    cancel: &Cancel,
    progress: &(dyn Fn(f64, &str) + Sync),
) -> Result<Vec<String>, End> {
    let args = json!({
        "userEmail": work, "source": cut.source, "model": cut.model, "outputs": cut.outputs, "outDir": cut.out_dir
    });
    let started = jobs.query("separate.start", &args).map_err(End::Failed)?;
    let mut record = started["value"].clone();
    let id = record["id"].as_str().unwrap_or_default().to_string();
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(End::Stopped);
        }
        if record["failed"] == true {
            return Err(End::Failed(
                record["error"]
                    .as_str()
                    .unwrap_or("the separation failed")
                    .to_string(),
            ));
        }
        if record["done"] == true {
            return files_of(&record, cut.out_dir);
        }
        if let Some(p) = record["percent"].as_f64() {
            progress(p / 100.0, record["phase"].as_str().unwrap_or_default());
        }
        tokio::time::sleep(POLL).await;
        let answer = jobs
            .query("separate.job", &json!({ "id": id }))
            .map_err(End::Failed)?;
        record = answer["value"].clone();
        if record.is_null() {
            return Err(End::Failed(
                "the separation was forgotten by this machine".into(),
            ));
        }
    }
}

/// What a cut needs: its source, its model, what to write and where.
pub struct Cut<'a> {
    pub source: &'a str,
    pub model: &'a str,
    pub outputs: &'a [String],
    pub out_dir: &'a str,
}

/// The names of the files a finished record lists, each one a file of `out_dir`.
fn files_of(record: &Value, out_dir: &str) -> Result<Vec<String>, End> {
    let dir = Path::new(out_dir);
    let mut names = Vec::new();
    for f in record["files"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        match Path::new(f).strip_prefix(dir).ok().and_then(|n| n.to_str()) {
            Some(n) if !n.is_empty() && !n.contains(['/', '\\']) => names.push(n.to_string()),
            _ => {
                return Err(End::Failed(
                    "the machine named a file outside its cut".into(),
                ));
            }
        }
    }
    Ok(names)
}

/// `POST /verb-host/separate {at, source, model, outputs?}`: a job whose files are the cut's, in its own folder.
pub async fn separate(req: &Checked, jobs: &Arc<Jobs>) -> Response {
    let mut real = Vec::with_capacity(2);
    for key in ["source", "model"] {
        match req.text(key).and_then(|shown| req.read(shown)) {
            Ok(r) => real.push(r),
            Err(r) => return *r,
        }
    }
    let (model, source) = (
        real.pop().unwrap_or_default(),
        real.pop().unwrap_or_default(),
    );
    let outputs = match outputs_of(req.body.get("outputs")) {
        Ok(o) => o,
        Err(m) => return bad(m),
    };
    if !jobs.bound() {
        return no_machine(NO_ENGINE);
    }
    let run = match req.run() {
        Ok(r) => r,
        Err(r) => return *r,
    };
    let id = run.id.clone();
    let dir = match req.job_dir(&id) {
        Ok(d) => d,
        Err(r) => return *r,
    };
    let (jobs, job, work) = (
        jobs.clone(),
        id.clone(),
        format!("{}/{id}", req.ctx.account),
    );
    tokio::spawn(async move {
        let cancel = run.cancel.clone();
        let cut = Cut {
            source: &source,
            model: &model,
            outputs: &outputs,
            out_dir: &dir,
        };
        let shown = |f: f64, p: &str| Table::global().progress(&job, f, p);
        match cut_run(&jobs, &work, &cut, &cancel, &shown).await {
            Ok(names) => match job_files(&dir, &names) {
                Ok(files) => drop(Table::global().finish(&job, json!({ "files": files }), now())),
                Err(e) => fail(&job, &e.error, None),
            },
            Err(End::Failed(e)) => fail(&job, &e, (e == NO_ENGINE).then_some("no-machine")),
            Err(End::Stopped) => {}
        }
        drop(run);
    });
    Checked::accepted(&id)
}
