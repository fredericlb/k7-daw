// routes.rs — the front's routes a verb calls or asks, their names in one place, and what a job's answer hands back.
// A reader would get wrong a job's files: they wait in the job's `dir` until a verb copies them into the project.

use crate::host::{Failed, Host, Thrown};
use serde_json::{Value as Json, json};

pub const ENGINE: &str = "POST /verb-host/engine";
pub const PLUGINS: &str = "GET /verb-host/plugins";
pub const RESCAN: &str = "POST /verb-host/plugins/rescan";
pub const STEREO: &str = "POST /verb-host/devices/stereo";
pub const ONNX: &str = "POST /verb-host/onnx";
pub const TAKE_SAVE: &str = "POST /verb-host/take/save";
pub const BENCH_SAVE: &str = "POST /bench/save";
pub const PLAYLIST_STEMS: &str = "POST /verb-host/playlist-stems";
pub const WAVEFORM: &str = "GET /waveform";
pub const SEPARATE: &str = "POST /verb-host/separate";
pub const RENDER: &str = "POST /verb-host/render";
pub const PEAK: &str = "POST /verb-host/peak";

/// A route's failure that the TypeScript lets escape the verb: its sentence is the throw.
pub fn thrown(f: Failed) -> Thrown {
    Thrown(f.error)
}

/// A route's answer, its failure thrown as the awaited member's rejection would be.
pub fn call_or_throw(h: &mut Host, route: &str, args: Json) -> Result<Json, Thrown> {
    h.call(route, args)?.map_err(thrown)
}

/// One op of the engine route asked after the writes land.
pub fn engine_ask(h: &mut Host, body: Json) {
    h.ask(ENGINE, body);
}

/// The lines an answer carries, said in the log.
pub fn say_lines(h: &mut Host, answer: &Json) {
    let lines = answer.get("lines").and_then(Json::as_array).into_iter().flatten();
    let said: Vec<String> = lines.filter_map(Json::as_str).map(str::to_owned).collect();
    h.log(said);
}

/// A file a job left in its folder: its name below it and the seconds the route measured, if any.
#[derive(Clone, Debug, PartialEq)]
pub struct JobFile {
    pub out: String,
    pub seconds: Option<f64>,
}

/// The `files` of a job's value.
pub fn job_files(value: &Json) -> Vec<JobFile> {
    let rows = value.get("files").and_then(Json::as_array).into_iter().flatten();
    rows.filter_map(|f| {
        Some(JobFile {
            out: f.get("out")?.as_str()?.to_owned(),
            seconds: f.get("seconds").and_then(Json::as_f64),
        })
    })
    .collect()
}

/// The absolute folder a finished job's files wait in, which the backend adds to its value.
pub fn job_dir(value: &Json) -> Result<String, Thrown> {
    value
        .get("dir")
        .and_then(Json::as_str)
        .map(str::to_owned)
        .ok_or_else(|| Thrown("the job's answer names no folder".to_owned()))
}

/// Copies the job's file `out` to `dest`, its folder made first: the "adopt" the TypeScript's change set landed.
pub fn adopt(h: &mut Host, dir: &str, out: &str, dest: &str) -> Result<(), Thrown> {
    if let Some((parent, _)) = dest.rsplit_once('/')
        && !parent.is_empty()
    {
        h.fs.mkdir_all(parent).map_err(|e| Thrown(e.to_string()))?;
    }
    h.fs.copy(&format!("{dir}/{out}"), dest)
        .map_err(|e| Thrown(e.to_string()))
}

/// What a finished render answers: the mixdown's name in the job's folder, and that folder.
pub struct Rendered {
    pub mix: String,
    pub dir: String,
}

/// `POST /verb-host/render {doc, stems?, base?}`: the job's mix, or the route's failure.
pub fn render(h: &mut Host, doc: &song::doc::ProjectDoc) -> Result<Result<Rendered, Failed>, Thrown> {
    let doc = serde_json::to_value(doc).map_err(|e| Thrown(e.to_string()))?;
    let value = match h.call(RENDER, json!({ "doc": doc }))? {
        Ok(v) => v,
        Err(f) => return Ok(Err(f)),
    };
    let mix = value
        .pointer("/manifest/mix")
        .and_then(Json::as_str)
        .ok_or_else(|| Thrown("the render's manifest names no mix".to_owned()))?
        .to_owned();
    Ok(Ok(Rendered {
        mix,
        dir: job_dir(&value)?,
    }))
}

/// `renderToWav`: the doc rendered and its mixdown copied to exactly `dest`; a failure is the route's sentence.
pub fn render_to(h: &mut Host, doc: &song::doc::ProjectDoc, dest: &str) -> Result<Result<(), String>, Thrown> {
    match render(h, doc)? {
        Ok(r) => {
            adopt(h, &r.dir, &r.mix, dest)?;
            Ok(Ok(()))
        }
        Err(f) => Ok(Err(f.error)),
    }
}
