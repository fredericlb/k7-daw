//! tests_driver.rs — the real driver over a rig: what it answers from memory, a local render, a machine's through a fake jobs path.
//! What a reader gets wrong: `render_run` answers from a task, so each test awaits its reply; the engine is a shell script (Unix only).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{Value, json};

use super::Driver;
use super::driver::{DriverEnv, real_driver};
use crate::routes::files::service::native::engine::fake::{FakeJobs, JobEnd};
use crate::routes::files::service::native::engine::testkit::Rig;

fn scratch(name: &str) -> PathBuf {
    let dir = crate::routes::files::service::native::tests::tmp_root()
        .join(format!("k7-drv-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::canonicalize(dir).unwrap()
}

/// The account's environment as the hub passes it, with the studio home in `dir`.
fn env_of(dir: &Path, more: &[(&str, &str)]) -> DriverEnv {
    let mut env = vec![
        ("PATH".to_string(), "/usr/bin:/bin".to_string()),
        (
            "STUDIO_HOME".to_string(),
            crate::routes::files::service::native::tests::slashed(&dir.join("home")),
        ),
    ];
    env.extend(more.iter().map(|(k, v)| (k.to_string(), v.to_string())));
    DriverEnv::new(dir.join("no-code-root"), env)
}

async fn run(
    d: &Arc<dyn Driver>,
    out: &Path,
    stems: bool,
    base: Option<&str>,
) -> Result<Value, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let args = json!({ "doc": { "tracks": [] }, "outDir": out.display().to_string(), "stems": stems, "base": base });
    d.render_run(args, Box::new(move |r| drop(tx.send(r))));
    rx.await.unwrap().map(|v| v["value"].clone())
}

fn staged(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir.join("home/.renders/jobs"))
        .map(|r| {
            r.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default()
}

#[tokio::test]
async fn availability_and_the_binding_are_answered_from_memory() {
    let rig = Rig::new("drv-mem").await;
    let dir = scratch("mem");
    let none = real_driver(&rig.link, env_of(&dir, &[]));
    assert!(!none.renderer_available() && !none.bound());
    let own = real_driver(
        &rig.link,
        env_of(&dir, &[("STUDIO_RENDER_JOBS_URL", "ws://127.0.0.1:1")]),
    );
    assert!(own.renderer_available() && !own.bound());
    rig.link
        .bind(&json!({ "url": rig.engine.url, "engineId": "e1", "jobsUrl": "ws://127.0.0.1:2" }));
    assert!(none.renderer_available() && none.bound());
    std::fs::remove_dir_all(dir).ok();
}

#[tokio::test]
async fn a_machines_render_is_copied_into_the_folder_and_its_staging_removed() {
    let rig = Rig::new("drv-machine").await;
    let jobs = FakeJobs::start().await;
    jobs.ends(JobEnd::Rendered {
        stems: vec!["gtr".into(), "bass".into()],
    });
    rig.link
        .bind(&json!({ "url": rig.engine.url, "engineId": "e1", "jobsUrl": jobs.url }));
    let dir = scratch("machine");
    let d = real_driver(&rig.link, env_of(&dir, &[]));
    let out = dir.join("out");
    let got = run(&d, &out, true, None).await.unwrap();
    assert_eq!(got["mix"], "mix.wav");
    assert_eq!(
        got["stems"],
        json!([{ "name": "gtr", "file": "gtr.wav" }, { "name": "bass", "file": "bass.wav" }])
    );
    assert_eq!(std::fs::read(out.join("gtr.wav")).unwrap(), b"RIFFstem");
    let asked = &jobs.asked()[0];
    assert_eq!(
        (asked["type"].as_str(), asked["stems"].as_bool()),
        (Some("render"), Some(true))
    );
    assert!(
        asked["outDir"].as_str().unwrap().contains(".renders/jobs/"),
        "{asked}"
    );
    assert!(staged(&dir).is_empty(), "{:?}", staged(&dir));
    std::fs::remove_dir_all(dir).ok();
}

#[tokio::test]
async fn a_machine_that_fails_hands_the_render_to_the_next_and_a_base_folder_is_refused() {
    let rig = Rig::new("drv-next").await;
    let (bad, good) = (FakeJobs::start().await, FakeJobs::start().await);
    bad.ends(JobEnd::Error("no vst".into()));
    good.ends(JobEnd::Rendered { stems: vec![] });
    rig.link
        .bind(&json!({ "url": rig.engine.url, "engineId": "e1", "jobsUrl": bad.url }));
    let dir = scratch("next");
    let d = real_driver(
        &rig.link,
        env_of(&dir, &[("STUDIO_RENDER_JOBS_URL", &good.url)]),
    );
    assert_eq!(
        run(&d, &dir.join("out"), false, None).await.unwrap()["mix"],
        "mix.wav"
    );
    assert_eq!(bad.asked().len(), 1);
    let refused = run(&d, &dir.join("o2"), false, Some("/base"))
        .await
        .unwrap_err();
    assert_eq!(
        refused,
        "a render with a base folder needs an engine binary on this machine"
    );
    assert_eq!(bad.asked().len(), 1, "a base folder asked a machine");
    std::fs::remove_dir_all(dir).ok();
}

#[tokio::test]
async fn nothing_to_render_on_is_the_typescripts_sentence() {
    let rig = Rig::new("drv-none").await;
    let dir = scratch("none");
    let d = real_driver(&rig.link, env_of(&dir, &[]));
    assert_eq!(
        run(&d, &dir.join("out"), false, None).await.unwrap_err(),
        "no engine machine to render on"
    );
    std::fs::remove_dir_all(dir).ok();
}

#[tokio::test]
async fn this_machines_engine_renders_alone_and_a_machine_is_not_asked() {
    let rig = Rig::new("drv-local").await;
    let jobs = FakeJobs::start().await;
    rig.link
        .bind(&json!({ "url": rig.engine.url, "engineId": "e1", "jobsUrl": jobs.url }));
    let dir = scratch("local");
    let bin = dir.join("engine.sh");
    let script = r#"#!/bin/sh
printf RIFFfake > "$4/mix.wav"; echo '{"mix":"mix.wav","stems":[]}'
"#;
    std::fs::write(&bin, script).unwrap();
    std::fs::set_permissions(&bin, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let d = real_driver(
        &rig.link,
        env_of(&dir, &[("STUDIO_ENGINE", &bin.display().to_string())]),
    );
    let got = run(&d, &dir.join("out"), false, None).await.unwrap();
    assert_eq!(got, json!({ "mix": "mix.wav", "stems": [] }));
    assert!(jobs.asked().is_empty());
    std::fs::remove_dir_all(dir).ok();
}

#[tokio::test]
async fn a_freeze_is_rendered_by_the_bound_machine_to_its_place_whole_and_a_failure_leaves_nothing()
{
    let rig = Rig::new("drv-freeze").await;
    let machine = FakeJobs::start().await;
    machine.ends(JobEnd::Rendered { stems: vec![] });
    rig.link
        .bind(&json!({ "url": rig.engine.url, "engineId": "e1", "jobsUrl": machine.url }));
    let dir = scratch("freeze");
    let d = real_driver(&rig.link, env_of(&dir, &[]));
    let out = dir.join("renders/gtr-abc.wav");
    std::fs::create_dir_all(out.parent().unwrap()).unwrap();
    d.render_freeze("{}".into(), out.clone()).await.unwrap();
    assert_eq!(std::fs::read(&out).unwrap(), b"RIFFfake");
    assert!(
        machine.asked()[0]["out"]
            .as_str()
            .unwrap()
            .contains(".part-")
    );
    let left = std::fs::read_dir(out.parent().unwrap()).unwrap().count();
    assert_eq!(left, 1, "a staging was left beside the render");
    machine.ends(JobEnd::Error("no vst".into()));
    let again = dir.join("renders/lead.wav");
    assert_eq!(
        d.render_freeze("{}".into(), again.clone())
            .await
            .unwrap_err(),
        "no vst"
    );
    assert!(!again.exists());
    std::fs::remove_dir_all(dir).ok();
}

#[tokio::test]
async fn a_freeze_the_machine_wrote_half_of_and_failed_leaves_no_part_file_behind() {
    let rig = Rig::new("drv-half").await;
    let machine = FakeJobs::start().await;
    machine.ends(JobEnd::WroteThenError("the plugin crashed".into()));
    rig.link
        .bind(&json!({ "url": rig.engine.url, "engineId": "e1", "jobsUrl": machine.url }));
    let dir = scratch("half");
    let d = real_driver(&rig.link, env_of(&dir, &[]));
    let out = dir.join("renders/gtr-abc.wav");
    std::fs::create_dir_all(out.parent().unwrap()).unwrap();
    let said = d.render_freeze("{}".into(), out.clone()).await.unwrap_err();
    assert_eq!(said, "the plugin crashed");
    let left: Vec<_> = std::fs::read_dir(out.parent().unwrap()).unwrap().collect();
    assert!(left.is_empty(), "left behind: {left:?}");
    std::fs::remove_dir_all(dir).ok();
}

fn alive(pid: &str) -> bool {
    std::process::Command::new("kill")
        .args(["-0", pid])
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

#[tokio::test]
async fn an_ask_given_up_kills_its_render_and_never_replies() {
    let rig = Rig::new("drv-gone").await;
    let dir = scratch("gone");
    let bin = dir.join("engine.sh");
    std::fs::write(&bin, "#!/bin/sh\necho $$ > \"$4/pid\"; exec sleep 600\n").unwrap();
    std::fs::set_permissions(&bin, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let d = real_driver(
        &rig.link,
        env_of(&dir, &[("STUDIO_ENGINE", &bin.display().to_string())]),
    );
    let jobs = super::Jobs::new(Some(d));
    let (stop, gone) = tokio::sync::oneshot::channel::<()>();
    let replied = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let out = dir.join("out");
    let seen = replied.clone();
    let args = json!({ "doc": { "tracks": [] }, "outDir": out.display().to_string() });
    let until_gone = Box::pin(async move {
        let _ = gone.await;
    });
    let reply = Box::new(move |_| seen.store(true, std::sync::atomic::Ordering::SeqCst));
    jobs.ask_until("render.run", args, reply, until_gone);
    let pid = loop {
        if let Ok(t) = std::fs::read_to_string(out.join("pid"))
            && !t.trim().is_empty()
        {
            break t.trim().to_owned();
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    };
    assert!(alive(&pid));
    // Dropping the sender is what a dropped request does to its end of the pair.
    drop(stop);
    let until = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while alive(&pid) {
        assert!(std::time::Instant::now() < until, "{pid} still runs");
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert!(!replied.load(std::sync::atomic::Ordering::SeqCst));
    std::fs::remove_dir_all(dir).ok();
}
