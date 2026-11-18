//! media.rs — `probe` and `extract`: what a video file is, and its sound written into the job's folder (3f, M6).
//! What a reader gets wrong: a machine with no ffprobe reads the headers (`video_read`) or answers 415 `unsupported`, never a guess; the facts are `video_probe`'s.

use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::Ordering;
use std::time::Duration;

use axum::response::Response;
use k7_file_service::verb_host::Error;
use k7_file_service::verb_host::files::job_file;
use k7_file_service::verb_host::jobs::Table;
use k7_mediatek::video_probe::{facts_of, probe_args};
use serde_json::{Value, json};
use tokio::process::Command;

use super::drive::fail;
use super::{Checked, failure, now};
use crate::routes::files::json_answer;
use crate::routes::files::service::machine::engine_binary;

/// A probe is answered within this, as the table says.
const PROBE_LIMIT: Duration = Duration::from_secs(30);

/// What the TypeScript says when nothing reads the file.
pub(super) const NO_FFMPEG: &str = "getting a video ready for the picture needs ffmpeg. Install it on the machine the file service runs on.";

/// What a video verb says of a missing ffmpeg: on iOS the reason is that no program starts there.
pub(super) fn no_ffmpeg() -> &'static str {
    if k7_mediatek::programs::NONE {
        "getting a video ready for the picture needs ffmpeg, and an iOS app cannot start a program: use a computer's studio for it."
    } else {
        NO_FFMPEG
    }
}

/// The sound of a film is written under this name in the job's folder.
const SOUND: &str = "sound.wav";

pub(super) fn real_of(req: &Checked) -> Result<(String, String), Box<Response>> {
    let shown = req.text("video").or_else(|_| req.text("file"))?.to_string();
    let real = req.read(&shown)?;
    if Path::new(&real).is_file() {
        Ok((shown, real))
    } else {
        Err(Box::new(failure(&Error::new(
            404,
            format!("no such file: {shown}"),
        ))))
    }
}

/// No ffprobe on this machine (the phone): the file's own headers, or the typed `unsupported` when they say nothing.
async fn read_without_ffprobe(real: String) -> Response {
    let read =
        tokio::task::spawn_blocking(move || k7_mediatek::video_read::read_facts(Path::new(&real)))
            .await;
    match read {
        Ok(Ok(facts)) => json_answer(200, &json!({ "facts": facts })),
        _ => failure(&Error::coded(
            415,
            no_ffmpeg(),
            k7_mediatek::video_read::UNSUPPORTED_CODE,
        )),
    }
}

/// `POST /verb-host/probe {file}`: the first video stream's facts, or null for a file with none.
pub async fn probe(req: &Checked) -> Response {
    let (shown, real) = match real_of(req) {
        Ok(r) => r,
        Err(r) => return *r,
    };
    if k7_mediatek::programs::NONE {
        return read_without_ffprobe(real).await;
    }
    let env = req
        .ctx
        .docs
        .as_ref()
        .map(|d| d.env.clone())
        .unwrap_or_default();
    let bin = env
        .get("FFPROBE_BIN")
        .filter(|b| !b.is_empty())
        .cloned()
        .unwrap_or("ffprobe".into());
    let mut cmd = Command::new(&bin);
    cmd.args(probe_args(&real)).envs(&env);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let spawned = cmd.spawn();
    let child = match spawned {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return read_without_ffprobe(real).await;
        }
        Err(_) => return failure(&Error::new(500, "ffprobe could not be started")),
    };
    let out = match tokio::time::timeout(PROBE_LIMIT, child.wait_with_output()).await {
        Ok(Ok(o)) => o,
        Ok(Err(_)) => return failure(&Error::new(500, "ffprobe failed")),
        Err(_) => {
            return failure(&Error::new(
                500,
                format!("ffprobe took longer than 30 s on {shown}"),
            ));
        }
    };
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let line = err
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("ffprobe failed");
        // The line names the file by its real path: the answer says it by the name the browser sent.
        return failure(&Error::new(500, line.replace(&real, &shown)));
    }
    match facts_of(&String::from_utf8_lossy(&out.stdout)) {
        Ok(facts) => json_answer(200, &json!({ "facts": facts })),
        Err(e) => failure(&Error::new(500, e)),
    }
}

/// `POST /verb-host/extract {video}`: a job that writes the film's first audio stream as a float wav.
pub async fn extract(req: &Checked) -> Response {
    let (_, real) = match real_of(req) {
        Ok(r) => r,
        Err(r) => return *r,
    };
    let run = match req.run() {
        Ok(r) => r,
        Err(r) => return *r,
    };
    let id = run.id.clone();
    let dir = match req.job_dir(&id) {
        Ok(d) => d,
        Err(r) => return *r,
    };
    let engine = req.ctx.docs.as_ref().and_then(|d| {
        engine_binary(
            d.env.get("STUDIO_ENGINE").map(String::as_str),
            Path::new(&d.code_root),
        )
    });
    let cancel = run.cancel.clone();
    tokio::spawn(async move {
        let dest = format!("{dir}/{SOUND}");
        let args = json!({ "source": real, "dest": dest, "engineBin": engine.map(|e| e.display().to_string()) });
        let flag = cancel.clone();
        let work = tokio::task::spawn_blocking(move || {
            crate::routes::files::service::native::media::direct::run_direct(
                "media.extract",
                &args,
                &|| flag.load(Ordering::Relaxed),
            )
        });
        match work.await {
            Ok(Ok(v)) => extracted(&run.id, &dir, &v),
            Ok(Err(e)) => fail(&run.id, &e, None),
            Err(_) => fail(&run.id, "the extract failed", None),
        }
        drop(run);
    });
    Checked::accepted(&id)
}

fn extracted(job: &str, dir: &str, v: &Value) {
    if let Some(m) = v["fail"]["message"].as_str() {
        return fail(job, m, None);
    }
    let facts = &v["facts"];
    if facts.is_null() {
        // A film with no sound is an answer, not a fault.
        Table::global().finish(job, json!({ "facts": null, "files": [] }), now());
        return;
    }
    match job_file(dir, SOUND) {
        Ok(file) => {
            let value = json!({
                "facts": { "frames": facts["frames"], "sampleRate": facts["sampleRate"], "channels": facts["channels"] },
                "files": [file],
            });
            Table::global().finish(job, value, now());
        }
        Err(e) => fail(job, &e.error, None),
    }
}
