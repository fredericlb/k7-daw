//! `prepare` behind the frame (3f, M7): a fake ffmpeg records what it was asked and what the clip file held at that moment.
//! What a reader gets wrong: the fake is a script, so these hold on Unix; the copies' arguments are the vectors'.

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use super::Member;
use super::tests_drive::{Rig, bound, rig};
use crate::routes::files::service::native::callbacks::BoxFuture;
use crate::routes::files::service::native::runtime::Runtime;
use crate::routes::files::service::native::runtime::logs::{Heard, Machine};

use super::drive;

/// Counts each call, keeps its arguments and the clip file as it stood, waits while `hold` exists, then writes the output.
const FFMPEG: &str = r#"#!/bin/sh
for last; do :; done
n=$(cat "$LOGDIR/n" 2>/dev/null || echo 0); n=$((n+1)); echo $n > "$LOGDIR/n"
echo "$@" > "$LOGDIR/call-$n.args"
cp "$CLIP" "$LOGDIR/call-$n.clip"
while [ -f "$LOGDIR/hold" ]; do sleep 0.05; done
if [ -f "$LOGDIR/fail" ]; then echo "Conversion failed!" >&2; exit 1; fi
echo copy > "$last"
"#;

/// Each test is its own account: the registry of what runs is the process's.
fn account(r: &Rig) -> String {
    r.b.dir.file_name().unwrap().to_string_lossy().into_owned()
}

fn song(r: &Rig, rel: &str) -> PathBuf {
    r.b.dir.join("home/workspace/songs/p.k7").join(rel)
}

struct Silent;

impl Machine for Silent {
    fn ask(&self, _: Value, _: String, _: Heard) -> BoxFuture<Result<Option<Value>, String>> {
        Box::pin(async { Ok(None) })
    }
}

/// A project with one clip whose source is `media/a.mp4`, and a log folder the fake ffmpeg writes to.
async fn bed(name: &str) -> (Rig, PathBuf, String) {
    let r = rig(name, bound()).await;
    std::fs::create_dir_all(song(&r, "media")).unwrap();
    std::fs::create_dir_all(song(&r, "clips")).unwrap();
    std::fs::write(song(&r, "media/a.mp4"), "film").unwrap();
    std::fs::write(song(&r, "clips/c.k7x"), "source(\"media/a.mp4\")\n").unwrap();
    let logs = r.b.dir.join("ffmpeg-logs");
    std::fs::create_dir_all(&logs).unwrap();
    let bin = r.b.dir.join("ffmpeg");
    std::fs::write(&bin, FFMPEG).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    (r, logs, bin.display().to_string())
}

async fn go(
    r: &Rig,
    logs: &Path,
    ffmpeg: &str,
    runtime: Option<Arc<Runtime>>,
    body: Value,
) -> (u16, Value) {
    let mut ctx = r.b.ctx(Member::Prepare, false);
    ctx.account = account(r);
    ctx.verbs = drive::production_in(Some(r.jobs()), runtime);
    let env = &mut ctx.docs.as_mut().unwrap().env;
    env.insert("FFMPEG_BIN".into(), ffmpeg.into());
    env.insert("LOGDIR".into(), logs.display().to_string());
    env.insert("CLIP".into(), song(r, "clips/c.k7x").display().to_string());
    r.b.send(Member::Prepare, "", body, Some(ctx)).await
}

fn ask(optimized: Value, proxy: bool) -> Value {
    json!({ "at": "p.k7", "clip": "c", "source": "media/a.mp4", "optimized": optimized, "proxy": proxy })
}

async fn until(what: &str, ok: impl Fn() -> bool) {
    for _ in 0..200 {
        if ok() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("never: {what}");
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_proxy_is_made_from_the_optimized_copy_and_each_field_lands_only_after_its_copy() {
    let (r, logs, ffmpeg) = bed("vh-prep-order").await;
    let (s, a) = go(&r, &logs, &ffmpeg, None, ask(json!("transcode"), true)).await;
    assert_eq!((s, &a["started"]), (200, &json!(true)), "{a}");
    let clip = song(&r, "clips/c.k7x");
    until("both fields land", || read(&clip).contains("proxy")).await;
    let text = read(&clip);
    assert!(
        text.contains("optimized") && text.contains("media/a.optimized.mp4"),
        "{text}"
    );
    assert!(
        text.contains("proxy") && text.contains("media/a.proxy.mp4"),
        "{text}"
    );
    assert!(song(&r, "media/a.optimized.mp4").is_file());
    assert!(song(&r, "media/a.proxy.mp4").is_file());
    let nothing: Vec<_> = std::fs::read_dir(song(&r, "media"))
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().ends_with(".part"))
        .collect();
    assert!(nothing.is_empty(), "no .part is left");
    // The first copy starts with neither field set, the second with the optimized one and the copy it names.
    let (first, second) = (
        read(&logs.join("call-1.args")),
        read(&logs.join("call-2.args")),
    );
    let (before, between) = (
        read(&logs.join("call-1.clip")),
        read(&logs.join("call-2.clip")),
    );
    assert!(
        !before.contains("optimized") && !before.contains("proxy"),
        "{before}"
    );
    assert!(
        between.contains("media/a.optimized.mp4") && !between.contains("proxy"),
        "{between}"
    );
    assert!(
        first.contains("a.mp4")
            && first.contains("libx264")
            && first.contains("a.optimized.mp4.part"),
        "{first}"
    );
    assert!(
        second.contains("a.optimized.mp4")
            && second.contains("scale=-2:720")
            && second.contains("a.proxy.mp4.part"),
        "{second}"
    );
    assert!(
        !second.contains("a.mp4 "),
        "the proxy does not read the source: {second}"
    );
    let _ = std::fs::remove_dir_all(&r.b.dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_proxy_alone_is_made_from_the_source_or_from_a_copy_the_clip_already_holds() {
    let (r, logs, ffmpeg) = bed("vh-prep-proxy").await;
    let (s, _) = go(&r, &logs, &ffmpeg, None, ask(Value::Null, true)).await;
    assert_eq!(s, 200);
    let clip = song(&r, "clips/c.k7x");
    until("the proxy lands", || read(&clip).contains("proxy")).await;
    assert!(read(&logs.join("call-1.args")).contains("media/a.mp4 "));
    // With an optimized copy on disk and in the clip, the proxy reads that.
    std::fs::write(song(&r, "media/a.optimized.mp4"), "copy").unwrap();
    std::fs::write(
        &clip,
        "source(\"media/a.mp4\")\noptimized(\"media/a.optimized.mp4\")\n",
    )
    .unwrap();
    std::fs::remove_file(song(&r, "media/a.proxy.mp4")).unwrap();
    let (s, a) = go(&r, &logs, &ffmpeg, None, ask(Value::Null, true)).await;
    assert_eq!((s, &a["started"]), (200, &json!(true)), "{a}");
    until("the second call", || logs.join("call-2.args").exists()).await;
    assert!(
        read(&logs.join("call-2.args")).contains("a.optimized.mp4"),
        "{}",
        read(&logs.join("call-2.args"))
    );
    let _ = std::fs::remove_dir_all(&r.b.dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_second_prepare_of_a_source_being_prepared_says_it_runs_and_a_later_one_starts() {
    let (r, logs, ffmpeg) = bed("vh-prep-twice").await;
    std::fs::write(logs.join("hold"), "").unwrap();
    let (_, a) = go(&r, &logs, &ffmpeg, None, ask(json!("remux"), false)).await;
    assert_eq!(a["started"], true, "{a}");
    until("ffmpeg runs", || logs.join("call-1.args").exists()).await;
    let (s, a) = go(&r, &logs, &ffmpeg, None, ask(json!("remux"), false)).await;
    assert_eq!((s, &a["started"]), (200, &json!(false)), "{a}");
    std::fs::remove_file(logs.join("hold")).unwrap();
    let clip = song(&r, "clips/c.k7x");
    until("the field lands", || read(&clip).contains("optimized")).await;
    // Once this one ended, the source is free again.
    until("the registry is free", || {
        !super::prepare::busy(&account(&r))
    })
    .await;
    let (_, a) = go(&r, &logs, &ffmpeg, None, ask(json!("remux"), false)).await;
    assert_eq!(a["started"], true, "{a}");
    let _ = std::fs::remove_dir_all(&r.b.dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_registry_lists_a_prepare_and_a_tag_it_already_runs_says_started_false() {
    let (r, logs, ffmpeg) = bed("vh-prep-registry").await;
    let rt = Runtime::new(Arc::new(Silent));
    std::fs::write(logs.join("hold"), "").unwrap();
    let (_, a) = go(
        &r,
        &logs,
        &ffmpeg,
        Some(rt.clone()),
        ask(json!("remux"), false),
    )
    .await;
    assert_eq!(a["started"], true, "{a}");
    let list = rt.processes();
    assert_eq!(list.len(), 1, "{list:?}");
    assert_eq!(list[0]["state"], "running");
    assert_eq!(list[0]["label"], "picture prepare a.mp4");
    std::fs::remove_file(logs.join("hold")).unwrap();
    until("the job ends", || rt.processes()[0]["state"] != "running").await;
    assert_eq!(rt.processes()[0]["state"], "done", "{:?}", rt.processes());
    // A run of the same tag holds it: the new one starts nothing.
    let tag = format!(
        "picture prepare {}",
        std::fs::canonicalize(song(&r, "media/a.mp4"))
            .unwrap()
            .display()
    );
    assert!(rt.bg_start(&tag));
    let (_, a) = go(
        &r,
        &logs,
        &ffmpeg,
        Some(rt.clone()),
        ask(json!("remux"), false),
    )
    .await;
    assert_eq!(a["started"], false, "{a}");
    let _ = std::fs::remove_dir_all(&r.b.dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_failed_copy_lands_no_field_and_leaves_no_part_and_a_missing_ffmpeg_is_said() {
    let (r, logs, ffmpeg) = bed("vh-prep-fail").await;
    std::fs::write(logs.join("fail"), "").unwrap();
    let (_, a) = go(&r, &logs, &ffmpeg, None, ask(json!("transcode"), true)).await;
    assert_eq!(a["started"], true);
    until("it was tried", || logs.join("call-1.args").exists()).await;
    until("the log says it", || {
        r.b.lines
            .lock()
            .unwrap()
            .iter()
            .any(|l| l.contains("failed: ffmpeg failed: Conversion failed!"))
    })
    .await;
    let clip = read(&song(&r, "clips/c.k7x"));
    assert_eq!(clip, "source(\"media/a.mp4\")\n");
    assert!(!song(&r, "media/a.optimized.mp4.part").exists());
    assert!(
        !logs.join("call-2.args").exists(),
        "the proxy is not tried after a failed copy"
    );
    until("the registry is free", || {
        !super::prepare::busy(&account(&r))
    })
    .await;
    let missing = r.b.dir.join("no-ffmpeg-here").display().to_string();
    let (_, a) = go(&r, &logs, &missing, None, ask(json!("remux"), false)).await;
    assert_eq!(a["started"], true);
    until("the missing tool is said", || {
        r.b.lines
            .lock()
            .unwrap()
            .iter()
            .any(|l| l.contains("ffmpeg is not installed"))
    })
    .await;
    let _ = std::fs::remove_dir_all(&r.b.dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_prepare_names_what_it_refuses_and_starts_nothing() {
    let (r, logs, ffmpeg) = bed("vh-prep-refuse").await;
    std::fs::write(song(&r, "media/noext"), "film").unwrap();
    std::fs::create_dir_all(song(&r, "../q.k7")).unwrap();
    std::fs::write(song(&r, "../q.k7/b.mp4"), "film").unwrap();
    let with = |k: &str, v: Value| {
        let mut b = ask(json!("remux"), false);
        b[k] = v;
        b
    };
    for (body, want) in [
        (with("source", json!("/etc/passwd")), 403),
        (with("source", json!("../../../../etc/passwd")), 403),
        (with("source", json!("media/none.mp4")), 404),
        // A source of a sibling project reads, but its copy would land outside the open one.
        (with("source", json!("../q.k7/b.mp4")), 403),
        (with("source", json!("media/noext")), 400),
        (with("clip", json!("../c")), 400),
        (with("clip", json!(7)), 400),
        (with("optimized", json!("copy")), 400),
        (with("proxy", json!("yes")), 400),
        (ask(Value::Null, false), 400),
        (with("at", json!("other.k7")), 409),
    ] {
        let (s, a) = go(&r, &logs, &ffmpeg, None, body.clone()).await;
        assert_eq!(s, want, "{body}: {a}");
    }
    assert!(!logs.join("call-1.args").exists(), "no ffmpeg was started");
    // A source that links out of the project is refused through the link.
    std::os::unix::fs::symlink("/etc/passwd", song(&r, "media/link.mp4")).unwrap();
    let (s, _) = go(
        &r,
        &logs,
        &ffmpeg,
        None,
        with("source", json!("media/link.mp4")),
    )
    .await;
    assert_eq!(s, 403);
    let _ = std::fs::remove_dir_all(&r.b.dir);
}
