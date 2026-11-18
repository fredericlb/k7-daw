//! tests_picture.rs — the picture's job: media confined to the workspace, the job's text spliced, the film renamed in whole (2550 to 2556).

use std::path::{Path, PathBuf};

use k7_file_service::shares::picture::{Format, VideoSettings};
#[cfg(unix)]
use serde_json::Value;

use super::Shares;
use super::fakes::*;
#[cfg(unix)]
use super::picture::media_of;
use super::picture::{BUSY, Request, render_with};

/// The render with the workspace's own gate, which the media of the scene are mapped by.
async fn render_picture(
    shares: &std::sync::Arc<Shares>,
    acct: &super::Account,
    ports: &super::Ports,
    req: Request<'_>,
) -> Result<String, String> {
    render_with(shares, acct, ports, req, None).await
}

fn opts(audio: bool) -> VideoSettings {
    VideoSettings {
        format: Format::Webm,
        audio,
        size: None,
        quality: None,
        from: None,
        to: None,
    }
}

fn resolver(root: &Path) -> impl Fn(&str) -> Option<PathBuf> + Sync + use<> {
    let root = root.to_path_buf();
    move |rel: &str| (!rel.split('/').any(|s| s == "..")).then(|| root.join(rel))
}

const SCENE: &str = r#"{"viewport":[1280,720],"fps":30,"durationSec":2,"clips":[{"url":"media/a.mp4"},{"url":"media/dir%2Fb.png"},{"url":"media/gone.mp4"}],"n":1.0}"#;

fn rig(name: &str) -> (PathBuf, super::Account, PathBuf, PathBuf) {
    let base = scratch(name);
    let a = account(
        &base,
        &[(
            "STUDIO_CHROMIUM",
            &base.join("chromium").display().to_string(),
        )],
    );
    std::fs::write(base.join("chromium"), "").unwrap();
    let project = a.root.join("song.k7");
    std::fs::create_dir_all(project.join("dir")).unwrap();
    std::fs::write(project.join("a.mp4"), "a").unwrap();
    std::fs::write(project.join("dir/b.png"), "b").unwrap();
    let script = base.join("page.js");
    std::fs::write(&script, "").unwrap();
    (base, a, project, script)
}

#[tokio::test]
async fn the_job_is_spliced_the_media_found_and_the_film_renamed_in_whole() {
    let (base, a, project, script) = rig("job");
    let w = world();
    let shares = Shares::new();
    let res = resolver(&a.root);
    let scene = SCENE
        .replace("media/a.mp4", &format!("media/{}", "song.k7%2Fa.mp4"))
        .replace("media/dir%2Fb.png", "media/song.k7%2Fdir%2Fb.png");
    let o = opts(false);
    let req = Request {
        project: &project,
        scene: &scene,
        sound: "mix",
        opts: &o,
        script,
        debug: false,
        resolve: &res,
    };
    let rel = render_picture(&shares, &a, &w.ports, req).await.unwrap();
    assert_eq!(rel, "exports/song.webm");
    assert_eq!(std::fs::read(project.join(&rel)).unwrap(), b"WEBMfilm");
    let names: Vec<String> = std::fs::read_dir(project.join("exports"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["song.webm"], "no part file is left");
    let jobs = w.browser.jobs.lock().unwrap();
    let (text, mix, media) = &jobs[0];
    assert_eq!(
        text,
        &format!(
            "{{\"scene\":{scene},\"format\":\"webm\",\"audio\":false,\"from\":0,\"frameCount\":60}}"
        )
    );
    assert!(mix.is_none());
    assert_eq!(
        media,
        &[
            "media/song.k7%2Fa.mp4".to_string(),
            "media/song.k7%2Fdir%2Fb.png".to_string()
        ]
    );
    assert_eq!(
        shares
            .counts
            .pictures
            .load(std::sync::atomic::Ordering::Relaxed),
        1
    );
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test]
async fn sound_is_the_mixdown_only_when_asked_and_its_folder_is_removed() {
    let (base, a, project, script) = rig("mix");
    let w = world();
    let res = resolver(&a.root);
    let o = opts(true);
    let req = Request {
        project: &project,
        scene: SCENE,
        sound: "mix",
        opts: &o,
        script,
        debug: false,
        resolve: &res,
    };
    render_picture(&Shares::new(), &a, &w.ports, req)
        .await
        .unwrap();
    let jobs = w.browser.jobs.lock().unwrap();
    assert!(
        jobs[0].0.contains("\"audio\":true") && jobs[0].1.as_ref().unwrap().ends_with("mix.wav")
    );
    assert!(std::fs::read_dir(&a.tmp).unwrap().next().is_none());
    let _ = std::fs::remove_dir_all(&base);
}

/// 2555: a second render while one runs is refused, and the slot is free again after.
#[tokio::test]
async fn a_second_render_is_refused_while_one_runs() {
    let (base, a, project, script) = rig("busy");
    let w = world();
    let shares = Shares::new();
    let res = resolver(&a.root);
    let held = shares.begin_picture().unwrap();
    let o = opts(false);
    let req = Request {
        project: &project,
        scene: SCENE,
        sound: "none",
        opts: &o,
        script: script.clone(),
        debug: false,
        resolve: &res,
    };
    assert_eq!(
        render_picture(&shares, &a, &w.ports, req)
            .await
            .unwrap_err(),
        BUSY
    );
    drop(held);
    let req = Request {
        project: &project,
        scene: SCENE,
        sound: "none",
        opts: &o,
        script,
        debug: false,
        resolve: &res,
    };
    assert!(render_picture(&shares, &a, &w.ports, req).await.is_ok());
    let _ = std::fs::remove_dir_all(&base);
}

/// 2556: `exports` that is a link out of the workspace is refused and nothing is written through it.
#[cfg(unix)]
#[tokio::test]
async fn an_exports_that_is_a_link_is_refused() {
    let (base, a, project, script) = rig("exlink");
    let outside = base.join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, project.join("exports")).unwrap();
    let w = world();
    let res = resolver(&a.root);
    let o = opts(false);
    let req = Request {
        project: &project,
        scene: SCENE,
        sound: "none",
        opts: &o,
        script,
        debug: false,
        resolve: &res,
    };
    let e = render_picture(&Shares::new(), &a, &w.ports, req)
        .await
        .unwrap_err();
    assert_eq!(e, "exports is not a folder of the project");
    assert!(std::fs::read_dir(&outside).unwrap().next().is_none());
    assert!(w.browser.jobs.lock().unwrap().is_empty());
    let _ = std::fs::remove_dir_all(&base);
}

/// 2552: a medium that is a link out, a path with `..` and a name the project lacks are left out of the map.
#[cfg(unix)]
#[test]
fn media_that_leave_the_workspace_are_left_out() {
    let base = scratch("media");
    let root = base.join("ws");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("in.mp4"), "x").unwrap();
    std::fs::write(base.join("secret"), "s").unwrap();
    std::os::unix::fs::symlink(base.join("secret"), root.join("out.mp4")).unwrap();
    let scene: Value = serde_json::json!({ "a": ["media/in.mp4", { "b": "media/out.mp4" }, "media/%2e%2e%2fsecret", "media/none.mp4", "other/in.mp4"] });
    let got = media_of(&scene, &root, &resolver(&root));
    assert_eq!(got.keys().collect::<Vec<_>>(), ["media/in.mp4"]);
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test]
async fn a_failed_render_leaves_no_film_and_frees_the_slot() {
    let (base, a, project, script) = rig("fail");
    let w = world();
    let browser = std::sync::Arc::new(FakeBrowser {
        jobs: Default::default(),
        fail: Some("chromium did not start".into()),
    });
    let mut ports = w.ports;
    std::sync::Arc::get_mut(&mut ports).unwrap().browser = browser;
    let shares = Shares::new();
    let res = resolver(&a.root);
    let o = opts(false);
    let req = Request {
        project: &project,
        scene: SCENE,
        sound: "none",
        opts: &o,
        script,
        debug: false,
        resolve: &res,
    };
    assert_eq!(
        render_picture(&shares, &a, &ports, req).await.unwrap_err(),
        "chromium did not start"
    );
    assert!(shares.begin_picture().is_some());
    assert!(
        std::fs::read_dir(project.join("exports"))
            .unwrap()
            .next()
            .is_none()
    );
    let _ = std::fs::remove_dir_all(&base);
}
