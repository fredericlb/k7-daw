//! `transcribe` behind the frame (3f, M7): fake whisper CLIs on a PATH of the test's own, over a real audio extract.
//! What a reader gets wrong: the fakes are scripts, so these hold on Unix; the CLIs' arguments and the SRT are the vectors'.

#![cfg(unix)]

use std::path::PathBuf;

use serde_json::{Value, json};

use super::Member;
use super::drive;
use super::tests_drive::{Rig, bound, rig, wav};

/// Writes an SRT where openai-whisper does; `MODE` makes it fail, hear nothing or write nothing.
const WHISPER: &str = r#"#!/bin/sh
[ "$1" = "--help" ] && exit 0
echo whisper >> "$LOGDIR/tried"
dir=""; wav="$1"
while [ $# -gt 0 ]; do [ "$1" = "--output_dir" ] && dir="$2"; shift; done
cp "$wav" "$LOGDIR/heard.wav"
case "$MODE" in
  fail) echo "line one" >&2; echo "line two" >&2; echo "line three" >&2; echo "boom: out of memory" >&2; exit 2 ;;
  silent) printf '' > "$dir/audio.srt" ;;
  nothing) ;;
  *) printf '1\n00:00:01,000 --> 00:00:02,000\nhello\n\n2\n00:00:02,500 --> 00:00:03,000\nthere\n' > "$dir/audio.srt" ;;
esac
"#;

/// Writes whisper.cpp's file, `transcribe.srt`, and notes the model it was given.
const WHISPER_CLI: &str = r#"#!/bin/sh
[ "$1" = "--help" ] && exit 0
echo whisper-cli >> "$LOGDIR/tried"
while [ $# -gt 0 ]; do [ "$1" = "-of" ] && out="$2"; [ "$1" = "-m" ] && echo "$2" > "$LOGDIR/model"; shift; done
printf '1\n00:00:00.480 --> 00:00:01.120\nbonjour\n' > "$out.srt"
"#;

fn script(dir: &std::path::Path, name: &str, body: &str) {
    use std::os::unix::fs::PermissionsExt;
    let p = dir.join(name);
    std::fs::write(&p, body).unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// A rig with a sound at `/w/p.k7/a.wav` and a PATH folder holding the CLIs named.
async fn bed(name: &str, clis: &[&str]) -> (Rig, PathBuf, PathBuf) {
    let r = rig(name, bound()).await;
    wav(&r.b.dir.join("home/workspace/songs/p.k7/a.wav"), 8000, 1000);
    let (bin, logs) = (r.b.dir.join("bin"), r.b.dir.join("logs"));
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::create_dir_all(&logs).unwrap();
    // The scripts copy the sound they hear; the PATH holds nothing else, so the CLIs the test names are the only ones there.
    std::os::unix::fs::symlink("/bin/cp", bin.join("cp")).unwrap();
    for c in clis {
        script(
            &bin,
            c,
            if *c == "whisper" {
                WHISPER
            } else {
                WHISPER_CLI
            },
        );
    }
    (r, bin, logs)
}

async fn go(
    r: &Rig,
    bin: &std::path::Path,
    logs: &std::path::Path,
    extra: &[(&str, &str)],
    body: Value,
) -> (u16, Value) {
    let mut ctx = r.b.ctx(Member::Transcribe, false);
    ctx.account = "ma".into();
    ctx.verbs = drive::production(Some(r.jobs()));
    let env = &mut ctx.docs.as_mut().unwrap().env;
    env.insert("PATH".into(), bin.display().to_string());
    env.insert("LOGDIR".into(), logs.display().to_string());
    env.insert(
        "FFMPEG_BIN".into(),
        r.b.dir.join("no-ffmpeg-here").display().to_string(),
    );
    for (k, v) in extra {
        env.insert((*k).into(), (*v).into());
    }
    r.b.send(Member::Transcribe, "", body, Some(ctx)).await
}

/// Starts a transcription of `file` and polls it to its end.
async fn heard(
    r: &Rig,
    bin: &std::path::Path,
    logs: &std::path::Path,
    extra: &[(&str, &str)],
    file: &str,
) -> Value {
    let (s, a) = go(
        r,
        bin,
        logs,
        extra,
        json!({ "video": format!("/w/p.k7/{file}") }),
    )
    .await;
    assert_eq!(s, 202, "{a}");
    r.ended("ma", a["job"].as_str().unwrap()).await
}

fn tried(logs: &std::path::Path) -> Vec<String> {
    std::fs::read_to_string(logs.join("tried"))
        .unwrap_or_default()
        .lines()
        .map(String::from)
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_srt_is_whispers_read_and_written_again_and_the_sound_it_heard_is_sixteen_kilohertz_mono()
 {
    let (r, bin, logs) = bed("vh-tr-ok", &["whisper"]).await;
    let end = heard(&r, &bin, &logs, &[], "a.wav").await;
    assert_eq!(end["state"], "done", "{end}");
    assert_eq!(
        end["value"]["srt"],
        "1\n00:00:01,000 --> 00:00:02,000\nhello\n\n2\n00:00:02,500 --> 00:00:03,000\nthere\n"
    );
    let head = std::fs::read(logs.join("heard.wav")).unwrap();
    assert_eq!(u16::from_le_bytes([head[22], head[23]]), 1, "mono");
    assert_eq!(
        u32::from_le_bytes(head[24..28].try_into().unwrap()),
        16_000,
        "16 kHz"
    );
    // The job's folder goes with the answer: nothing is adopted from a transcription.
    let top = r.b.dir.join("home/.job-out");
    let left = std::fs::read_dir(&top).map(|d| d.count()).unwrap_or(0);
    assert_eq!(left, 0, "the work folder is removed");
    let _ = std::fs::remove_dir_all(&r.b.dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_video_with_no_speech_answers_null_and_one_that_wrote_no_file_is_a_failure() {
    let (r, bin, logs) = bed("vh-tr-silent", &["whisper"]).await;
    let end = heard(&r, &bin, &logs, &[("MODE", "silent")], "a.wav").await;
    assert_eq!(
        (end["state"].as_str(), &end["value"]["srt"]),
        (Some("done"), &Value::Null),
        "{end}"
    );
    let end = heard(&r, &bin, &logs, &[("MODE", "nothing")], "a.wav").await;
    assert_eq!(end["state"], "failed", "{end}");
    assert!(
        end["error"]
            .as_str()
            .unwrap()
            .starts_with("whisper failed: exit code 0"),
        "{end}"
    );
    let _ = std::fs::remove_dir_all(&r.b.dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_cli_that_ran_and_failed_is_the_error_as_it_said_it_and_no_other_dialect_is_tried() {
    let (r, bin, logs) = bed("vh-tr-fail", &["whisper", "whisper-cli"]).await;
    let end = heard(
        &r,
        &bin,
        &logs,
        &[("MODE", "fail"), ("STUDIO_WHISPER_MODEL", "/m/x.bin")],
        "a.wav",
    )
    .await;
    assert_eq!(end["state"], "failed");
    assert_eq!(
        end["error"],
        "whisper failed: line two · line three · boom: out of memory"
    );
    assert_eq!(
        tried(&logs),
        ["whisper"],
        "whisper.cpp is not asked after a failure"
    );
    assert!(
        !end["error"]
            .as_str()
            .unwrap()
            .contains(&r.b.dir.display().to_string())
    );
    let _ = std::fs::remove_dir_all(&r.b.dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_cli_not_installed_is_stepped_over_and_whisper_cpp_is_handed_its_model() {
    let (r, bin, logs) = bed("vh-tr-cpp", &["whisper-cli"]).await;
    let end = heard(
        &r,
        &bin,
        &logs,
        &[("STUDIO_WHISPER_MODEL", "/m/ggml.bin")],
        "a.wav",
    )
    .await;
    assert_eq!(
        end["value"]["srt"], "1\n00:00:00,480 --> 00:00:01,120\nbonjour\n",
        "{end}"
    );
    assert_eq!(
        std::fs::read_to_string(logs.join("model")).unwrap().trim(),
        "/m/ggml.bin"
    );
    // With no model it is present and refuses, and the refusal is the answer.
    let end = heard(&r, &bin, &logs, &[("STUDIO_WHISPER_MODEL", "")], "a.wav").await;
    assert_eq!(end["state"], "failed", "{end}");
    assert!(
        end["error"]
            .as_str()
            .unwrap()
            .starts_with("whisper-cli failed: whisper-cli needs a model"),
        "{end}"
    );
    let _ = std::fs::remove_dir_all(&r.b.dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn with_no_whisper_at_all_the_error_names_both_and_how_to_install_one() {
    let (r, bin, logs) = bed("vh-tr-none", &[]).await;
    let end = heard(&r, &bin, &logs, &[], "a.wav").await;
    assert_eq!(end["state"], "failed");
    let said = end["error"].as_str().unwrap();
    assert!(
        said.starts_with("no whisper on this machine (tried whisper, whisper-cli) — install one:"),
        "{said}"
    );
    let _ = std::fs::remove_dir_all(&r.b.dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_container_the_decoder_cannot_open_needs_ffmpeg_and_a_path_outside_the_roots_is_refused()
{
    let (r, bin, logs) = bed("vh-tr-refuse", &["whisper"]).await;
    std::fs::write(r.b.dir.join("home/workspace/songs/p.k7/x.mp4"), "junk").unwrap();
    let end = heard(&r, &bin, &logs, &[], "x.mp4").await;
    assert_eq!(
        (end["state"].as_str(), end["code"].as_str()),
        (Some("failed"), Some("no-ffmpeg")),
        "{end}"
    );
    assert!(tried(&logs).is_empty(), "whisper never heard it");
    for (video, want) in [("/etc/passwd", 403), ("/w/p.k7/none.mp4", 404)] {
        let (s, a) = go(&r, &bin, &logs, &[], json!({ "video": video })).await;
        assert_eq!(s, want, "{video}: {a}");
    }
    let _ = std::fs::remove_dir_all(&r.b.dir);
}
