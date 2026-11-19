//! tests_render.rs — the local renderer against fake engines (shell scripts, so Unix only): arguments, environment, bounds, the kill, the staging and the hostile manifests.
//! What a reader gets wrong: a fake engine is told its folder as `$4` (`--render <doc> --out <dir>`), and the real engine is the live test's, not these.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use super::{LocalRenderer, NO_MIX, NOT_WAV, OUTSIDE, RenderOpts, push_tail, render_ready};

/// A fresh folder under the system's temp, removed by the test through `gone`.
fn folder(name: &str) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = crate::routes::files::service::native::tests::tmp_root()
        .join(format!("k7-jr-{name}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::canonicalize(dir).unwrap()
}

fn gone(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

/// A script that stands for the engine.
fn engine(dir: &Path, body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let file = dir.join("fake-engine.sh");
    std::fs::write(&file, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
    file
}

/// Sets a path's mtime `minutes` back, as a render killed that long ago left it.
fn set_back(path: &Path, minutes: u64) {
    let when = std::time::SystemTime::now() - Duration::from_secs(minutes * 60);
    std::fs::File::open(path)
        .unwrap()
        .set_modified(when)
        .unwrap();
}

fn account() -> Vec<(String, String)> {
    vec![
        ("PATH".into(), "/usr/bin:/bin".into()),
        ("HOME".into(), "/acct/home".into()),
    ]
}

fn renderer(bin: PathBuf) -> LocalRenderer {
    LocalRenderer::new(bin, account())
}

const OK: &str = r#"printf RIFFfake > "$4/mix.wav"; echo '{"mix":"mix.wav","stems":[]}'"#;

async fn pid_in(dir: &Path) -> String {
    let file = dir.join("pid");
    let until = Instant::now() + Duration::from_secs(15);
    while Instant::now() < until {
        if let Ok(t) = std::fs::read_to_string(&file)
            && !t.trim().is_empty()
        {
            return t.trim().to_owned();
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("the fake engine never wrote its pid");
}

fn alive(pid: &str) -> bool {
    std::process::Command::new("kill")
        .args(["-0", pid])
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

async fn until_gone(pid: &str) {
    let until = Instant::now() + Duration::from_secs(15);
    while alive(pid) {
        assert!(Instant::now() < until, "process {pid} is still running");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn a_render_passes_the_arguments_and_answers_the_manifest() {
    let dir = folder("args");
    let bin = engine(
        &dir,
        r#"echo "$@" > "$4/args"; echo noise; printf RIFFa > "$4/mix.wav"; printf RIFFb > "$4/kick.wav"; echo '{"mix":"mix.wav","stems":[{"name":"kick","file":"kick.wav"},{"name":3},"x"]}'"#,
    );
    let out = dir.join("out");
    let opts = RenderOpts {
        stems: true,
        base: Some("/the/base".into()),
    };
    let moved = renderer(bin).render("{}", &out, &opts).await.unwrap();
    assert_eq!(moved.mix, "mix.wav");
    assert_eq!(moved.stems, [("kick".to_owned(), "kick.wav".to_owned())]);
    let args = std::fs::read_to_string(out.join("args")).unwrap();
    let doc = out.join("doc.json");
    assert_eq!(
        args.trim(),
        format!(
            "--render {} --out {} --stems --base /the/base",
            doc.display(),
            out.display()
        )
    );
    assert_eq!(std::fs::read_to_string(doc).unwrap(), "{}");
    gone(&dir);
}

#[tokio::test]
async fn the_child_sees_the_accounts_environment_alone() {
    let dir = folder("env");
    let bin = engine(
        &dir,
        &format!(r#"echo "${{CARGO_MANIFEST_DIR-unset}}:$HOME:$PATH" > "$4/seen"; {OK}"#),
    );
    let out = dir.join("out");
    renderer(bin)
        .render("{}", &out, &RenderOpts::default())
        .await
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(out.join("seen")).unwrap().trim(),
        "unset:/acct/home:/usr/bin:/bin"
    );
    gone(&dir);
}

#[tokio::test]
async fn a_failed_child_and_a_missing_binary_are_the_typescripts_sentences() {
    let dir = folder("fail");
    let out = dir.join("out");
    let r = |body: &str| renderer(engine(&dir, body));
    let go = |r: LocalRenderer| {
        let out = out.clone();
        async move { r.render("{}", &out, &RenderOpts::default()).await }
    };
    assert_eq!(
        go(r("exit 3")).await.unwrap_err(),
        "engine render failed (exit 3)"
    );
    assert_eq!(
        go(r("echo hello")).await.unwrap_err(),
        "could not read the engine's render manifest:\nhello\n"
    );
    assert_eq!(
        go(renderer("/nonexistent/k7-runtime".into()))
            .await
            .unwrap_err(),
        "spawn /nonexistent/k7-runtime ENOENT"
    );
    gone(&dir);
}

#[tokio::test]
async fn a_manifest_naming_a_file_it_should_not_is_refused_in_one_sentence_each() {
    let dir = folder("manifest");
    std::fs::write(dir.join("elsewhere.wav"), "x").unwrap();
    let out = dir.join("inner/out");
    std::fs::create_dir_all(out.parent().unwrap()).unwrap();
    std::fs::write(out.parent().unwrap().join("x.wav"), "x").unwrap();
    let elsewhere = dir.join("elsewhere.wav");
    std::fs::create_dir_all(&out).unwrap();
    std::os::unix::fs::symlink(&elsewhere, out.join("link.wav")).unwrap();
    let cases: Vec<(String, String)> = vec![
        (r#"{"mix":"../x.wav","stems":[]}"#.into(), OUTSIDE.into()),
        (
            format!(r#"{{"mix":"{}","stems":[]}}"#, elsewhere.display()),
            OUTSIDE.into(),
        ),
        (r#"{"mix":"link.wav","stems":[]}"#.into(), OUTSIDE.into()),
        (
            r#"{"mix":"mix.wav","stems":[{"name":"s","file":"../x.wav"}]}"#.into(),
            OUTSIDE.into(),
        ),
        (r#"{"mix":"mix.txt","stems":[]}"#.into(), NOT_WAV.into()),
        (r#"{"mix":"","stems":[]}"#.into(), NO_MIX.into()),
        ("[]".into(), NO_MIX.into()),
        (
            r#"{"mix":"none.wav","stems":[]}"#.into(),
            "the engine named a file it did not write: 'none.wav'".into(),
        ),
    ];
    for (manifest, want) in cases {
        let bin = engine(
            &dir,
            &format!(r#"printf RIFFfake > "$4/mix.wav"; echo '{manifest}'"#),
        );
        let got = renderer(bin)
            .render("{}", &out, &RenderOpts::default())
            .await;
        assert_eq!(got.unwrap_err(), want, "{manifest}");
    }
    gone(&dir);
}

#[tokio::test]
async fn a_child_that_never_exits_is_killed_at_the_bound_and_no_process_is_left() {
    let dir = folder("hang");
    let bin = engine(&dir, r#"echo $$ > "$4/pid"; exec sleep 600"#);
    let out = dir.join("out");
    let r = renderer(bin).with_timeout(Duration::from_secs(2));
    let task = {
        let out = out.clone();
        tokio::spawn(async move { r.render("{}", &out, &RenderOpts::default()).await })
    };
    // The pid is awaited beside the render, so a slow shell start never races the bound.
    let pid = pid_in(&out).await;
    let got = task.await.unwrap();
    assert_eq!(got.unwrap_err(), "engine render failed (exit ?)");
    // The kill is awaited before the answer: nothing to wait for here.
    assert!(!alive(&pid));
    gone(&dir);
}

#[tokio::test]
async fn a_dropped_request_kills_its_child() {
    let dir = folder("dropped");
    let bin = engine(&dir, r#"echo $$ > "$4/pid"; exec sleep 600"#);
    let out = dir.join("out");
    let r = Arc::new(renderer(bin));
    let task = {
        let (r, out) = (r.clone(), out.clone());
        tokio::spawn(async move { r.render("{}", &out, &RenderOpts::default()).await })
    };
    let pid = pid_in(&out).await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    until_gone(&pid).await;
    gone(&dir);
}

#[tokio::test]
async fn four_mib_of_stdout_is_drained_and_the_manifest_after_it_read() {
    let dir = folder("chatty");
    let bin = engine(
        &dir,
        &format!("head -c 4194304 /dev/zero | tr '\\0' a; echo; {OK}"),
    );
    let out = dir.join("out");
    let moved = renderer(bin)
        .with_timeout(Duration::from_secs(30))
        .render("{}", &out, &RenderOpts::default())
        .await
        .unwrap();
    assert_eq!(moved.mix, "mix.wav");
    // The tail keeps its last bytes only.
    let mut tail = Vec::new();
    for _ in 0..3 {
        push_tail(&mut tail, &vec![b'x'; 1 << 20]);
    }
    push_tail(&mut tail, b"end");
    assert_eq!(tail.len(), 1 << 20);
    assert!(tail.ends_with(b"xend"));
    gone(&dir);
}

#[tokio::test]
async fn a_document_of_sixteen_mib_reaches_the_engine_whole() {
    let dir = folder("bigdoc");
    let bin = engine(&dir, &format!(r#"wc -c < "$2" > "$4/size"; {OK}"#));
    let out = dir.join("out");
    let doc = "x".repeat(16 << 20);
    renderer(bin)
        .render(&doc, &out, &RenderOpts::default())
        .await
        .unwrap();
    let size = std::fs::read_to_string(out.join("size")).unwrap();
    assert_eq!(size.trim(), (16usize << 20).to_string());
    gone(&dir);
}

#[tokio::test]
async fn a_render_folder_that_cannot_be_written_is_the_sentence_of_the_open() {
    use std::os::unix::fs::PermissionsExt;
    let dir = folder("readonly");
    let bin = engine(&dir, OK);
    let out = dir.join("out");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::set_permissions(&out, std::fs::Permissions::from_mode(0o555)).unwrap();
    // The superuser writes anywhere; its run proves nothing here.
    if std::fs::write(out.join("probe"), "").is_err() {
        let err = renderer(bin)
            .render("{}", &out, &RenderOpts::default())
            .await
            .unwrap_err();
        assert!(err.starts_with("EACCES: permission denied, open"), "{err}");
    }
    std::fs::set_permissions(&out, std::fs::Permissions::from_mode(0o755)).unwrap();
    gone(&dir);
}

#[tokio::test]
async fn a_render_to_a_wav_lands_whole_by_one_rename_and_leaves_nothing_beside_it() {
    let dir = folder("wav");
    let bin = engine(
        &dir,
        r#"printf RIFFwhole > "$4/mix.wav"; echo '{"mix":"mix.wav","stems":[]}'"#,
    );
    let renders = dir.join("renders");
    std::fs::create_dir_all(&renders).unwrap();
    let wav = renders.join("fp.wav");
    // A killed render's leftovers, and an older file under the name.
    std::fs::create_dir_all(renders.join("fp.wav.part-old")).unwrap();
    std::fs::write(renders.join("fp.wav.part-old/mix.wav"), "half").unwrap();
    std::fs::write(renders.join("fp.wav.part-stale"), "half").unwrap();
    std::fs::write(&wav, "old").unwrap();
    for left in ["fp.wav.part-old", "fp.wav.part-stale"] {
        set_back(&renders.join(left), 11);
    }
    renderer(bin).render_to_wav("{}", &wav).await.unwrap();
    assert_eq!(std::fs::read_to_string(&wav).unwrap(), "RIFFwhole");
    let names: Vec<String> = std::fs::read_dir(&renders)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["fp.wav"]);
    gone(&dir);
}

#[tokio::test]
async fn a_killed_render_leaves_no_file_under_the_name_and_no_staging() {
    let dir = folder("killed");
    // Half a mix is on the disk when the engine is killed.
    let bin = engine(&dir, r#"printf RIFFhalf > "$4/mix.wav"; exec sleep 600"#);
    let renders = dir.join("renders");
    let wav = renders.join("fp.wav");
    let r = renderer(bin).with_timeout(Duration::from_millis(1500));
    assert_eq!(
        r.render_to_wav("{}", &wav).await.unwrap_err(),
        "engine render failed (exit ?)"
    );
    assert!(!wav.exists(), "a render appears only whole (2313)");
    assert_eq!(std::fs::read_dir(&renders).unwrap().count(), 0);
    gone(&dir);
}

#[test]
fn a_render_is_ready_when_it_is_there_not_ready_when_missing_and_an_error_when_the_stat_fails() {
    use std::os::unix::fs::PermissionsExt;
    let dir = folder("ready");
    let sub = dir.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join("a.wav"), "x").unwrap();
    assert_eq!(render_ready(&sub.join("a.wav")), Ok(true));
    assert_eq!(render_ready(&sub.join("b.wav")), Ok(false));
    std::fs::set_permissions(&sub, std::fs::Permissions::from_mode(0o000)).unwrap();
    // The superuser stats anything; a permission error is not "missing" for everyone else.
    if std::fs::metadata(sub.join("a.wav")).is_err() {
        let err = render_ready(&sub.join("a.wav")).unwrap_err();
        assert!(err.starts_with("EACCES: permission denied, stat"), "{err}");
    }
    std::fs::set_permissions(&sub, std::fs::Permissions::from_mode(0o755)).unwrap();
    gone(&dir);
}

#[tokio::test]
async fn a_helper_holding_the_pipe_does_not_turn_a_finished_render_into_a_timeout() {
    let dir = folder("helper");
    // The engine exits 0 with its manifest written; a process it started keeps stdout open for ten minutes.
    let bin = engine(&dir, &format!(r#"sleep 600 & echo $! > "$4/pid"; {OK}"#));
    let out = dir.join("out");
    let started = Instant::now();
    let got = renderer(bin)
        .with_timeout(Duration::from_secs(30))
        .render("{}", &out, &RenderOpts::default())
        .await;
    let helper = pid_in(&out).await;
    let _ = std::process::Command::new("kill").arg(&helper).status();
    assert_eq!(got.unwrap().mix, "mix.wav");
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "waited for the pipe"
    );
    gone(&dir);
}

#[tokio::test]
async fn a_render_in_progress_beside_the_target_survives_a_sweep() {
    let dir = folder("live");
    let bin = engine(&dir, OK);
    let renders = dir.join("renders");
    std::fs::create_dir_all(renders.join("fp.wav.part-live")).unwrap();
    std::fs::create_dir_all(renders.join("fp.wav.part-dead")).unwrap();
    set_back(&renders.join("fp.wav.part-dead"), 11);
    renderer(bin)
        .render_to_wav("{}", &renders.join("fp.wav"))
        .await
        .unwrap();
    assert!(
        renders.join("fp.wav.part-live").exists(),
        "a live render's folder was swept"
    );
    assert!(!renders.join("fp.wav.part-dead").exists());
    gone(&dir);
}

#[tokio::test]
async fn the_staging_folder_sits_beside_the_render_so_the_rename_never_crosses_volumes() {
    let dir = folder("beside");
    // The engine is told its folder as `$4`; it writes that path where the test can read it.
    let bin = engine(
        &dir,
        &format!(r#"echo "$4" > "$(dirname "$0")/staging"; {OK}"#),
    );
    let renders = dir.join("renders");
    renderer(bin)
        .render_to_wav("{}", &renders.join("fp.wav"))
        .await
        .unwrap();
    let staging = std::fs::read_to_string(dir.join("staging")).unwrap();
    let staging = Path::new(staging.trim());
    assert_eq!(staging.parent(), Some(renders.as_path()));
    assert!(
        staging
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("fp.wav.part-")
    );
    gone(&dir);
}

/// The helper a fake engine started, its pid written before the engine's own so the engine's pid means both are up.
async fn helper_of(out: &Path) -> String {
    pid_in(out).await;
    std::fs::read_to_string(out.join("helper"))
        .unwrap()
        .trim()
        .to_owned()
}

const WITH_HELPER: &str = r#"sleep 600 & echo $! > "$4/helper"; echo $$ > "$4/pid"; wait"#;

#[tokio::test]
async fn a_render_killed_at_the_bound_takes_the_helpers_of_its_plugins_with_it() {
    let dir = folder("group-bound");
    let out = dir.join("out");
    let r = renderer(engine(&dir, WITH_HELPER)).with_timeout(Duration::from_secs(2));
    let task = {
        let out = out.clone();
        tokio::spawn(async move { r.render("{}", &out, &RenderOpts::default()).await })
    };
    let helper = helper_of(&out).await;
    assert!(alive(&helper));
    assert!(task.await.unwrap().is_err());
    until_gone(&helper).await;
    gone(&dir);
}

#[tokio::test]
async fn a_dropped_request_takes_the_helpers_of_its_plugins_with_it() {
    let dir = folder("group-drop");
    let out = dir.join("out");
    let r = Arc::new(renderer(engine(&dir, WITH_HELPER)));
    let task = {
        let (r, out) = (r.clone(), out.clone());
        tokio::spawn(async move { r.render("{}", &out, &RenderOpts::default()).await })
    };
    let helper = helper_of(&out).await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    until_gone(&helper).await;
    gone(&dir);
}
