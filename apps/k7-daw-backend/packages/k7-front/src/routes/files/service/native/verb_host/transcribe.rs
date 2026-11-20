//! transcribe.rs — `transcribe`: whisper listens to a video and the answer is its SRT, or null when it heard no speech (3f, M7; decision 3f-12).
//! What a reader gets wrong: a CLI that is not installed is stepped over, one that RAN and failed is the error, said as it said it.

use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::Ordering;
use std::time::Duration;

use axum::response::Response;
use k7_file_service::verb_host::gate::remove_out;
use k7_file_service::verb_host::jobs::{Cancel, Table};
use k7_mediatek::whisper::{BINS, cli_args, extract_audio_args, parse_srt, srt_of, srt_path};
use serde_json::{Value, json};
use tokio::process::Command;

use super::drive::{fail, gone};
use super::media::real_of;
use super::{Checked, now};

/// Decoding the sound of an hour of video is seconds of work: past this, the decoder is stuck.
const EXTRACT_MS: u64 = 10 * 60_000;

/// The most SRT text a job answers with.
pub const SRT_LIMIT: usize = 1 << 20;

/// What a CLI's attempt came to.
enum Try {
    Done(String),
    Missing,
    Failed(String),
}

/// A failed job: the sentence and the code a Worker maps to an error class.
type Failed = (String, Option<&'static str>);

fn plain(message: impl Into<String>) -> Failed {
    (message.into(), None)
}

/// The last three lines of a child's stderr, joined as the TypeScript joins them.
fn tail(err: &str) -> String {
    let lines: Vec<&str> = err.trim().split('\n').collect();
    lines[lines.len().saturating_sub(3)..].join(" · ")
}

/// Whether `bin` is on this machine's PATH, asked by running its `--help`.
async fn present(bin: &str, env: &std::collections::BTreeMap<String, String>) -> bool {
    let mut cmd = Command::new(bin);
    cmd.arg("--help").envs(env);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    match cmd.spawn() {
        Ok(mut child) => {
            let _ = child.wait().await;
            true
        }
        Err(e) => e.kind() != std::io::ErrorKind::NotFound,
    }
}

async fn attempt(
    bin: &str,
    wav: &str,
    out_dir: &str,
    env: &std::collections::BTreeMap<String, String>,
) -> Try {
    let model = env
        .get("STUDIO_WHISPER_MODEL")
        .cloned()
        .or_else(|| std::env::var("STUDIO_WHISPER_MODEL").ok());
    let args = match cli_args(bin, wav, out_dir, model.as_deref()) {
        Ok(a) => a,
        // A refusal the adapter itself raised only matters once the tool is there: else it is just missing.
        Err(e) => {
            return if present(bin, env).await {
                Try::Failed(e)
            } else {
                Try::Missing
            };
        }
    };
    let mut cmd = Command::new(bin);
    cmd.args(args).envs(env);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let out = match cmd.spawn() {
        Ok(child) => child.wait_with_output().await,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Try::Missing,
        Err(e) => return Try::Failed(e.to_string()),
    };
    let srt = srt_path(bin, wav, out_dir);
    match out {
        Ok(o) if o.status.success() && Path::new(&srt).exists() => Try::Done(srt),
        Ok(o) => {
            let said = tail(&String::from_utf8_lossy(
                &o.stderr[o.stderr.len().saturating_sub(4000)..],
            ));
            Try::Failed(if said.is_empty() {
                format!(
                    "exit code {}",
                    o.status.code().map_or("?".into(), |c| c.to_string())
                )
            } else {
                said
            })
        }
        Err(e) => Try::Failed(e.to_string()),
    }
}

/// The 16 kHz mono wav whisper reads: decoded in WebAssembly, and by ffmpeg only for a container that cannot open.
async fn extract_audio(
    video: &str,
    shown: &str,
    wav: &str,
    env: &std::collections::BTreeMap<String, String>,
    cancel: &Cancel,
) -> Result<(), Failed> {
    let args = json!({
        "pieces": [{ "source": video }], "dest": wav, "deadlineMs": EXTRACT_MS,
        "target": { "kind": "wav", "rate": 16_000, "channels": 1 },
    });
    let flag = cancel.clone();
    let done = tokio::task::spawn_blocking(move || {
        crate::routes::files::service::native::media::direct::run_direct(
            "media.transcode",
            &args,
            &|| flag.load(Ordering::Relaxed),
        )
    })
    .await
    .map_err(|_| plain("the audio extract failed"))?
    .map_err(plain)?;
    let Some(fail) = done.get("fail") else {
        return Ok(());
    };
    let message = fail["message"]
        .as_str()
        .unwrap_or("the audio extract failed");
    if fail["name"] != "WasmOpenError" {
        return Err(plain(
            message.replace(wav, "audio.wav").replace(video, shown),
        ));
    }
    let bin = env
        .get("FFMPEG_BIN")
        .filter(|b| !b.is_empty())
        .cloned()
        .unwrap_or_else(|| "ffmpeg".into());
    let mut cmd = Command::new(&bin);
    cmd.args(extract_audio_args(video, wav)).envs(env);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let name = shown.rsplit('/').next().unwrap_or(shown);
    let child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err((
                format!(
                    "{bin} is not installed — this video's container needs ffmpeg to be transcribed. Install it."
                ),
                Some("no-ffmpeg"),
            ));
        }
        Err(e) => return Err(plain(e.to_string())),
    };
    match tokio::time::timeout(Duration::from_millis(EXTRACT_MS), child.wait_with_output()).await {
        Ok(Ok(o)) if o.status.success() => Ok(()),
        Ok(Ok(o)) => {
            let err = String::from_utf8_lossy(&o.stderr)
                .replace(video, shown)
                .replace(wav, "audio.wav");
            let last: String = err
                .trim()
                .chars()
                .rev()
                .take(500)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            Err(plain(format!(
                "ffmpeg could not read {name}: ffmpeg failed: {}",
                if last.is_empty() {
                    format!("exit {}", o.status)
                } else {
                    last
                }
            )))
        }
        Ok(Err(e)) => Err(plain(format!("ffmpeg could not read {name}: {e}"))),
        Err(_) => Err(plain(format!(
            "ffmpeg could not read {name}: reading {name} took longer than {} s and was killed",
            EXTRACT_MS / 1000
        ))),
    }
}

/// Whisper's SRT for `video`, or `None` when it heard no speech.
async fn listen(
    video: &str,
    shown: &str,
    work: &str,
    env: &std::collections::BTreeMap<String, String>,
    cancel: &Cancel,
) -> Result<Option<String>, Failed> {
    if k7_mediatek::programs::NONE {
        return Err(plain(format!(
            "no whisper on this device: {}, so a video is transcribed on a computer's studio",
            k7_mediatek::programs::WHY
        )));
    }
    let wav = format!("{work}/audio.wav");
    extract_audio(video, shown, &wav, env, cancel).await?;
    let mut missing = Vec::new();
    for bin in BINS {
        match attempt(bin, &wav, work, env).await {
            Try::Done(path) => {
                let text = std::fs::read_to_string(&path).map_err(|e| plain(e.to_string()))?;
                let segments = parse_srt(&text);
                return Ok((!segments.is_empty()).then(|| srt_of(&segments)));
            }
            Try::Missing => missing.push(bin),
            Try::Failed(e) => return Err(plain(format!("{bin} failed: {}", e.replace(work, ".")))),
        }
    }
    Err(plain(format!(
        "no whisper on this machine (tried {}) — install one: pip install openai-whisper gives `whisper`; whisper.cpp gives `whisper-cli`",
        missing.join(", ")
    )))
}

/// `POST /verb-host/transcribe {video}`: a job whose value is `{srt}`, null for a video with no speech.
pub async fn transcribe(req: &Checked) -> Response {
    let (shown, real) = match real_of(req) {
        Ok(r) => r,
        Err(r) => return *r,
    };
    let run = match req.run() {
        Ok(r) => r,
        Err(r) => return *r,
    };
    let id = run.id.clone();
    // The work folder is the job's own, and goes whatever the end: the answer is text, no file is adopted.
    let work = match req.job_dir(&id) {
        Ok(d) => d,
        Err(r) => return *r,
    };
    let env = req
        .ctx
        .docs
        .as_ref()
        .map(|d| d.env.clone())
        .unwrap_or_default();
    let studio = req.ctx.home.studio.clone();
    let cancel = run.cancel.clone();
    tokio::spawn(async move {
        let stopped = gone(cancel.clone());
        let heard = tokio::select! {
            r = listen(&real, &shown, &work, &env, &cancel) => r,
            () = stopped => Err(plain("the transcription was stopped")),
        };
        match heard {
            Ok(Some(srt)) if srt.len() > SRT_LIMIT => {
                fail(&run.id, "the transcript is longer than 1 MiB", None)
            }
            Ok(srt) => {
                let value = json!({ "srt": srt.map_or(Value::Null, Value::String) });
                Table::global().finish(&run.id, value, now());
            }
            Err((e, code)) => fail(&run.id, &e, code),
        }
        remove_out(&studio, &run.id);
        drop(run);
    });
    Checked::accepted(&id)
}
