//! machine.rs — what the machine does for a server's jobs, apart from the tunnel: the log seeded by its tail and
//! continued by offset, read a bounded slice at a time; the month after which undo audio goes; and a render
//! through a child or through the running engine, files copied out only where the job may write.

use std::path::Path;
use std::time::{Duration, SystemTime};

use k7_supervisor::machine::history::{KEPT_HISTORY_TTL, purge_kept_history};
use k7_supervisor::machine::log_slice::{LOG_READ_CAP, read_log};
use k7_supervisor::machine::render::{Request, Via, ask_engine, render};
use k7_supervisor::paths::Paths;
use serde_json::{Value, json};

use crate::scratch::Scratch;

fn bed() -> (Scratch, Paths) {
    let t = Scratch::new("machine");
    let paths = Paths::at(t.path(), None);
    (t, paths)
}

#[test]
fn the_log_is_seeded_then_continued() {
    let (_t, paths) = bed();
    std::fs::write(paths.merged_log(), "a\nb\nc\n").unwrap();
    let seed = read_log(&paths, None, Some(2));
    assert_eq!(
        (seed["offset"].clone(), seed["lines"].clone()),
        (json!(6), json!(["b", "c"]))
    );
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .append(true)
        .open(paths.merged_log())
        .unwrap();
    f.write_all(b"d\npartial").unwrap();
    let next = read_log(&paths, Some(6), None);
    assert_eq!(
        (next["offset"].clone(), next["lines"].clone()),
        (json!(8), json!(["d"]))
    );
}

#[test]
fn a_stale_offset_seeds_from_the_tail_and_an_absent_log_is_empty() {
    let (_t, paths) = bed();
    assert_eq!(
        read_log(&paths, None, Some(10)),
        json!({"offset": 0, "lines": []})
    );
    std::fs::write(paths.merged_log(), "x\ny\n").unwrap();
    // An offset from before a rotation: past the end of the file that replaced it.
    assert_eq!(
        read_log(&paths, Some(5000), None),
        json!({"offset": 4, "lines": ["x", "y"]})
    );
    // The engine-only log answers when there is no merged one.
    std::fs::remove_file(paths.merged_log()).unwrap();
    std::fs::write(paths.engine_log(), "e\n").unwrap();
    assert_eq!(
        read_log(&paths, None, None),
        json!({"offset": 2, "lines": ["e"]})
    );
}

#[test]
fn a_large_log_is_read_by_its_tail_only() {
    let (_t, paths) = bed();
    let row = "r".repeat(99);
    let mut text = format!("{row}\n").repeat(30_000);
    text.push_str("last\n");
    std::fs::write(paths.merged_log(), &text).unwrap();
    let size = text.len() as u64;
    assert!(size > 2 * LOG_READ_CAP);
    let seed = read_log(&paths, None, Some(100_000));
    assert_eq!(seed["offset"], size);
    let lines = seed["lines"].as_array().unwrap();
    // Whole lines only, and no more than the cap holds: the cut at the start is dropped.
    assert_eq!(
        (lines.last().unwrap(), &lines[0]),
        (&json!("last"), &json!(row))
    );
    assert!(lines.len() as u64 <= LOG_READ_CAP / 100 + 1);
    // Far behind, a reader is handed one cap's worth and catches up on the next polls.
    let behind = read_log(&paths, Some(0), None);
    let to = behind["offset"].as_u64().unwrap();
    assert!(to <= LOG_READ_CAP && to.is_multiple_of(100));
    assert_eq!(behind["lines"].as_array().unwrap().len() as u64, to / 100);
}

#[test]
fn a_line_longer_than_the_cap_is_skipped_not_waited_on() {
    let (_t, paths) = bed();
    std::fs::write(paths.merged_log(), "z".repeat(LOG_READ_CAP as usize + 10)).unwrap();
    assert_eq!(
        read_log(&paths, Some(0), None),
        json!({"offset": LOG_READ_CAP, "lines": []})
    );
}

fn age(p: &Path, by: Duration) {
    let f = std::fs::File::options().write(true).open(p).unwrap();
    f.set_modified(SystemTime::now() - by).unwrap();
}

#[test]
fn kept_history_goes_after_a_month() {
    let (_t, paths) = bed();
    let remote = paths.studio_dir().join("remote");
    let kept = remote.join("server/history-fred/song-0123/blobs");
    let other = remote.join("server/ws-fred");
    std::fs::create_dir_all(&kept).unwrap();
    std::fs::create_dir_all(&other).unwrap();
    for f in [kept.join("old"), kept.join("new"), other.join("clip.wav")] {
        std::fs::write(f, "x").unwrap();
    }
    let old = KEPT_HISTORY_TTL + Duration::from_secs(3600);
    age(&kept.join("old"), old);
    age(&other.join("clip.wav"), old);
    assert_eq!(purge_kept_history(&remote, KEPT_HISTORY_TTL), 1);
    assert!(!kept.join("old").exists() && kept.join("new").exists());
    // Only what a server asked to KEEP: a workspace copy is not this purge's.
    assert!(other.join("clip.wav").exists());
}

fn request(out: &str, out_dir: &str, stems: bool) -> Request {
    Request {
        doc: json!({"tracks": []}),
        out: out.into(),
        out_dir: out_dir.into(),
        stems,
    }
}

/// A `k7-runtime` that reads the doc on stdin, writes its files into `--out` and prints the manifest last.
#[cfg(unix)]
fn fake_runtime(dir: &Path, script: &str) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let p = dir.join("k7-runtime");
    std::fs::write(&p, format!("#!/bin/sh\n{script}\n")).unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}

fn inside(root: &Path) -> impl Fn(&str) -> bool + Send + Sync + use<> {
    let root = root.to_string_lossy().into_owned();
    move |p| p.starts_with(&root)
}

#[cfg(unix)]
#[tokio::test]
async fn a_render_runs_the_engine_binary_and_copies_the_manifest_out() {
    let (t, _) = bed();
    let exe = fake_runtime(
        t.path(),
        r#"out=$4; cat > "$out/doc.json"; echo RIFF > "$out/mix.wav"; echo RIFF > "$out/a.wav"; echo noise; echo '{"mix":"mix.wav","stems":[{"name":"a","file":"a.wav"}]}'"#,
    );
    let dest = t.path().join("out");
    let said = std::sync::atomic::AtomicBool::new(false);
    let reply = render(
        &request("", &dest.to_string_lossy(), true),
        &Via::Child(exe.clone()),
        &inside(t.path()),
        &|| said.store(true, std::sync::atomic::Ordering::SeqCst),
    )
    .await
    .unwrap();
    assert!(said.load(std::sync::atomic::Ordering::SeqCst));
    assert!(dest.join("mix.wav").exists() && dest.join("a.wav").exists());
    assert_eq!(
        reply["mix"],
        dest.join("mix.wav").to_string_lossy().as_ref()
    );
    assert_eq!(
        reply["stems"],
        json!([{"name": "a", "file": dest.join("a.wav").to_string_lossy()}])
    );
    // `out` takes the mix alone, named as a file.
    let one = t.path().join("one.wav");
    let reply = render(
        &request(&one.to_string_lossy(), "", false),
        &Via::Child(exe),
        &inside(t.path()),
        &|| {},
    )
    .await
    .unwrap();
    assert_eq!(
        reply,
        json!({"type": "rendered", "file": one.to_string_lossy()})
    );
    assert!(one.exists());
}

#[cfg(unix)]
#[tokio::test]
async fn a_failing_render_says_its_exit_and_a_stem_outside_the_roots_is_refused() {
    let (t, _) = bed();
    let bad = fake_runtime(t.path(), "exit 3");
    let why = render(
        &request(&t.path().join("m.wav").to_string_lossy(), "", false),
        &Via::Child(bad),
        &inside(t.path()),
        &|| {},
    )
    .await
    .unwrap_err();
    assert_eq!(why, "engine render failed (exit 3)");
    let exe = fake_runtime(
        t.path(),
        r#"out=$4; echo R > "$out/mix.wav"; echo '{"mix":"mix.wav"}'"#,
    );
    let elsewhere = tempfile_free_dir();
    let why = render(
        &request(&elsewhere.join("m.wav").to_string_lossy(), "", false),
        &Via::Child(exe.clone()),
        &inside(t.path()),
        &|| {},
    )
    .await
    .unwrap_err();
    assert!(
        why.contains("outside this server's writable folders"),
        "{why}"
    );
    assert!(!elsewhere.join("m.wav").exists());
    // A manifest naming a file that climbs out of the render folder copies nothing.
    let climb = fake_runtime(t.path(), r#"echo '{"mix":"../k7-runtime"}'"#);
    let why = render(
        &request(&t.path().join("m.wav").to_string_lossy(), "", false),
        &Via::Child(climb),
        &inside(t.path()),
        &|| {},
    )
    .await
    .unwrap_err();
    assert!(why.contains("outside its render folder"), "{why}");
}

fn tempfile_free_dir() -> std::path::PathBuf {
    std::env::temp_dir().join("k7-never-made-by-the-test")
}

/// An engine that answers one `render` command as the test says, after a stray frame.
async fn engine_answering(
    answer: Value,
    write: &'static [&'static str],
) -> (u16, tokio::task::JoinHandle<Value>) {
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::Message;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        let mut ws = tokio_tungstenite::accept_async(tcp).await.unwrap();
        while let Some(Ok(Message::Text(t))) = ws.next().await {
            let cmd: Value = serde_json::from_str(t.as_str()).unwrap();
            if cmd["type"] != "render" {
                continue;
            }
            for f in write {
                std::fs::write(Path::new(cmd["outDir"].as_str().unwrap()).join(f), "RIFF").unwrap();
            }
            let mut reply = answer.clone();
            reply["type"] = json!("rendered");
            reply["id"] = cmd["id"].clone();
            // A stray frame first: the job waits for ITS `rendered`, not the first frame.
            ws.send(Message::text(r#"{"type":"whoami"}"#))
                .await
                .unwrap();
            ws.send(Message::text(reply.to_string())).await.unwrap();
            return cmd;
        }
        Value::Null
    });
    (port, seen)
}

fn via_engine(t: &Path, port: u16, counts: &std::sync::Arc<std::sync::Mutex<Vec<i32>>>) -> Via {
    let c = counts.clone();
    Via::Engine {
        port,
        scratch_under: t.join("render-tmp"),
        rendering: std::sync::Arc::new(move |d| c.lock().unwrap().push(d)),
    }
}

#[tokio::test]
async fn a_phone_renders_through_the_running_engine_and_counts_it_busy() {
    let (t, _) = bed();
    let (port, seen) = engine_answering(
        json!({"ok": true, "manifest": {"mix": "mix.wav", "stems": [{"name": "a", "file": "a.wav"}]}}),
        &["mix.wav", "a.wav"],
    )
    .await;
    let counts = Default::default();
    let dest = t.path().join("out");
    let reply = render(
        &request("", &dest.to_string_lossy(), true),
        &via_engine(t.path(), port, &counts),
        &inside(t.path()),
        &|| {},
    )
    .await
    .unwrap();
    assert!(dest.join("mix.wav").exists() && dest.join("a.wav").exists());
    assert_eq!(reply["stems"].as_array().unwrap().len(), 1);
    // The engine rendered into a folder of its write roots, with the doc and the stems flag.
    let cmd = seen.await.unwrap();
    assert_eq!(cmd["stems"], true);
    assert!(cmd["doc"].is_object());
    assert!(
        cmd["outDir"]
            .as_str()
            .unwrap()
            .starts_with(&*t.path().join("render-tmp").to_string_lossy())
    );
    assert_eq!(*counts.lock().unwrap(), vec![1, -1]);
    // The scratch is gone.
    assert_eq!(
        std::fs::read_dir(t.path().join("render-tmp"))
            .unwrap()
            .count(),
        0
    );
}

#[tokio::test]
async fn a_render_the_engine_refuses_is_an_error_with_its_reason_counted_back() {
    let (t, _) = bed();
    let (port, _seen) = engine_answering(
        json!({"ok": false, "busy": true, "error": "a render is already running on this engine"}),
        &[],
    )
    .await;
    let counts = Default::default();
    let why = render(
        &request(&t.path().join("m.wav").to_string_lossy(), "", false),
        &via_engine(t.path(), port, &counts),
        &inside(t.path()),
        &|| {},
    )
    .await
    .unwrap_err();
    assert!(why.contains("already running"), "{why}");
    assert_eq!(*counts.lock().unwrap(), vec![1, -1]);
    assert_eq!(
        std::fs::read_dir(t.path().join("render-tmp"))
            .unwrap()
            .count(),
        0
    );
}

#[tokio::test]
async fn a_phone_render_with_no_engine_fails_at_once_and_counts_nothing() {
    let (t, _) = bed();
    let counts = std::sync::Arc::default();
    let why = render(
        &request(&t.path().join("m.wav").to_string_lossy(), "", false),
        &via_engine(t.path(), 0, &counts),
        &inside(t.path()),
        &|| {},
    )
    .await
    .unwrap_err();
    assert_eq!(why, "the engine is not running");
    assert!(counts.lock().unwrap().is_empty());
    // A port nobody serves is the engine not answering, said as the reply.
    let free = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let reply = ask_engine(
        free,
        &json!({"type": "render", "id": "x"}),
        Duration::from_secs(1),
    )
    .await;
    assert_eq!(
        (reply["ok"].clone(), reply["id"].clone()),
        (json!(false), json!("x"))
    );
}
