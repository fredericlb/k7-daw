//! `picture` behind the frame (3f, M8): the Worker's scene rendered by a fake browser, its media gated, the film written whole.
//! What a reader gets wrong: the browser is a fake that records its job; the real one is `k7_picture`, proven by the bed's chromium.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use k7_file_service::verb_host::jobs::Table;
use serde_json::{Value, json};

use super::tests_drive::{Rig, bound, rig};
use super::{Member, drive, now};
use crate::routes::files::service::native::shares::BoxFut;
use crate::routes::files::service::native::shares::picture::Browser;

/// What the browser was handed: the job's text, the mixdown and the media by key.
struct Seen {
    text: String,
    mix: Option<PathBuf>,
    media: BTreeMap<String, PathBuf>,
}

/// A browser that records its job, waits while `hold` is set, then writes a fixed film where the real one would.
#[derive(Default)]
struct Fake {
    seen: Mutex<Vec<Seen>>,
    hold: Arc<AtomicBool>,
    fail: Option<String>,
}

impl Browser for Fake {
    fn render(&self, job: k7_picture::Job) -> BoxFut<'_, Result<k7_picture::Rendered, String>> {
        Box::pin(async move {
            self.seen.lock().unwrap().push(Seen {
                text: job.job.clone(),
                mix: job.mix.clone(),
                media: job
                    .media
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
            });
            while self.hold.load(Ordering::SeqCst) {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            if let Some(e) = &self.fail {
                return Err(e.clone());
            }
            let part = k7_picture::part_for(&job.out);
            std::fs::write(&part, b"WEBMfilm").unwrap();
            Ok(k7_picture::Rendered::new(part, "video/webm".into()))
        })
    }
}

fn song(r: &Rig, rel: &str) -> PathBuf {
    r.b.dir.join("home/workspace/songs/p.k7").join(rel)
}

const SCENE: &str = r#"{"viewport":[1280,720],"fps":30,"durationSec":2,"clips":[{"url":"media/%2Fw%2Fp.k7%2Fa.mp4"},{"url":"media/%2Fetc%2Fpasswd"},{"url":"media/..%2F..%2Fx"},{"url":"media/a.mp4"}]}"#;

fn ask(extra: Value) -> Value {
    let mut b = json!({ "at": "p.k7", "scene": serde_json::from_str::<Value>(SCENE).unwrap() });
    for (k, v) in extra.as_object().unwrap() {
        b[k] = v.clone();
    }
    b
}

fn films(r: &Rig) -> PathBuf {
    let dir = r.b.dir.join("films");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

async fn go(r: &Rig, account: &str, browser: Arc<Fake>, mut body: Value) -> (u16, Value) {
    body["into"] = json!(films(r).display().to_string());
    let mut ctx = r.b.ctx(Member::Picture, false);
    ctx.account = account.into();
    ctx.verbs = drive::production_with(Some(r.jobs()), None, browser);
    let env = &mut ctx.docs.as_mut().unwrap().env;
    let page = r.b.dir.join("page.js");
    let chromium = r.b.dir.join("chromium");
    std::fs::write(&page, "").unwrap();
    std::fs::write(&chromium, "").unwrap();
    env.insert("K7_RENDER_PAGE".into(), page.display().to_string());
    env.insert("STUDIO_CHROMIUM".into(), chromium.display().to_string());
    r.b.send(Member::Picture, "", body, Some(ctx)).await
}

async fn done(r: &Rig, account: &str, a: &Value) -> Value {
    let job = a["job"].as_str().unwrap_or_else(|| panic!("no job: {a}"));
    r.ended(account, job).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_scene_is_rendered_into_the_chosen_folder_with_only_the_media_the_person_reads() {
    let r = rig("vh-pic-film", bound()).await;
    std::fs::write(song(&r, "a.mp4"), "film").unwrap();
    let fake = Arc::new(Fake::default());
    let (s, a) = go(&r, "pa", fake.clone(), ask(json!({}))).await;
    assert_eq!(s, 202, "{a}");
    let end = done(&r, "pa", &a).await;
    assert_eq!(end["state"], "done", "{end}");
    assert_eq!(
        end["value"]["path"],
        films(&r).join("p.webm").display().to_string()
    );
    assert_eq!(
        std::fs::read(films(&r).join("p.webm")).unwrap(),
        b"WEBMfilm"
    );
    let seen = fake.seen.lock().unwrap();
    // The job is the Worker's scene whole, with the range the settings resolve to.
    let scene: Value = serde_json::from_str(SCENE).unwrap();
    let want = format!(
        "{{\"scene\":{},\"format\":\"webm\",\"audio\":false,\"from\":0,\"frameCount\":60}}",
        scene
    );
    assert_eq!(seen[0].text, want);
    assert!(seen[0].mix.is_none());
    // A virtual path of the project maps to its real file; a path outside every root, a climb and a relative path are left out.
    let keys: Vec<&str> = seen[0].media.keys().map(String::as_str).collect();
    assert_eq!(keys, ["media/%2Fw%2Fp.k7%2Fa.mp4"], "{keys:?}");
    let real = std::fs::canonicalize(song(&r, "a.mp4")).unwrap();
    assert_eq!(
        std::fs::canonicalize(&seen[0].media["media/%2Fw%2Fp.k7%2Fa.mp4"]).unwrap(),
        real
    );
    let _ = std::fs::remove_dir_all(&r.b.dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_format_and_the_settings_reach_the_job() {
    let r = rig("vh-pic-settings", bound()).await;
    let fake = Arc::new(Fake::default());
    let body = ask(json!({
        "format": "mp4",
        "settings": { "size": { "height": 360 }, "quality": "max", "from": 0.5, "to": 1.5 }
    }));
    let (_, a) = go(&r, "pb", fake.clone(), body).await;
    assert_eq!(done(&r, "pb", &a).await["state"], "done");
    let text = fake.seen.lock().unwrap()[0].text.clone();
    for part in [
        "\"format\":\"mp4\"",
        "\"from\":0.5",
        "\"frameCount\":30",
        "\"size\":[640,360]",
        "\"quality\":\"max\"",
    ] {
        assert!(text.contains(part), "{part} in {text}");
    }
    let _ = std::fs::remove_dir_all(&r.b.dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_sound_is_the_mixdown_of_the_open_project_only_when_asked() {
    let r = rig("vh-pic-mix", bound()).await;
    let fake = Arc::new(Fake::default());
    let (_, a) = go(&r, "pc", fake.clone(), ask(json!({ "audio": true }))).await;
    let end = done(&r, "pc", &a).await;
    assert_eq!(end["state"], "done", "{end}");
    {
        let seen = fake.seen.lock().unwrap();
        let mix = seen[0].mix.as_ref().expect("the mixdown is handed over");
        assert!(mix.ends_with("mix.wav"), "{mix:?}");
        assert!(seen[0].text.contains("\"audio\":true"), "{}", seen[0].text);
    }
    let silent = Arc::new(Fake::default());
    let (_, a) = go(&r, "pc2", silent.clone(), ask(json!({ "audio": false }))).await;
    assert_eq!(done(&r, "pc2", &a).await["state"], "done");
    assert!(silent.seen.lock().unwrap()[0].mix.is_none());
    let _ = std::fs::remove_dir_all(&r.b.dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_mixdown_with_no_machine_is_a_503_and_a_silent_film_needs_none() {
    let r = rig("vh-pic-nomachine", super::tests_drive::Fake::default()).await;
    let (s, a) = go(
        &r,
        "pd",
        Arc::new(Fake::default()),
        ask(json!({ "audio": true })),
    )
    .await;
    assert_eq!((s, a["code"].as_str()), (503, Some("no-machine")), "{a}");
    let (s, a) = go(&r, "pd", Arc::new(Fake::default()), ask(json!({}))).await;
    assert_eq!(s, 202, "{a}");
    let _ = std::fs::remove_dir_all(&r.b.dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_shape_that_is_wrong_is_a_400_and_another_project_a_409() {
    let r = rig("vh-pic-shapes", bound()).await;
    let bad = |extra: Value| {
        let r = &r;
        async move { go(r, "pe", Arc::new(Fake::default()), ask(extra)).await.0 }
    };
    assert_eq!(bad(json!({ "scene": [] })).await, 400);
    assert_eq!(bad(json!({ "scene": "{}" })).await, 400);
    assert_eq!(bad(json!({ "format": "avi" })).await, 400);
    assert_eq!(bad(json!({ "audio": "yes" })).await, 400);
    assert_eq!(bad(json!({ "settings": [] })).await, 400);
    assert_eq!(
        bad(json!({ "settings": { "quality": "ultra" } })).await,
        400
    );
    assert_eq!(
        bad(json!({ "settings": { "size": { "height": 0 } } })).await,
        400
    );
    assert_eq!(
        bad(json!({ "settings": { "size": { "height": 99999 } } })).await,
        400
    );
    assert_eq!(
        bad(json!({ "settings": { "size": { "width": 640 } } })).await,
        400
    );
    assert_eq!(bad(json!({ "settings": { "from": -1 } })).await, 400);
    assert_eq!(
        bad(json!({ "settings": { "from": 2, "to": 1 } })).await,
        400
    );
    assert_eq!(bad(json!({ "settings": { "to": "9" } })).await, 400);
    assert_eq!(bad(json!({ "at": "other.k7" })).await, 409);
    let _ = std::fs::remove_dir_all(&r.b.dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_second_render_while_one_runs_is_refused_and_the_job_outlives_the_page() {
    let r = rig("vh-pic-busy", bound()).await;
    let hold = Arc::new(AtomicBool::new(true));
    let fake = Arc::new(Fake {
        hold: hold.clone(),
        ..Fake::default()
    });
    let (s, a) = go(&r, "pf", fake.clone(), ask(json!({}))).await;
    assert_eq!(s, 202, "{a}");
    let (s, b) = go(&r, "pf", Arc::new(Fake::default()), ask(json!({}))).await;
    assert_eq!(
        (s, b["error"].as_str()),
        (409, Some("a picture render is already running"))
    );
    // Nobody polls for ten minutes: a render is not stopped for it.
    Table::global().tick_of("pf", now() + Duration::from_secs(600));
    let job = a["job"].as_str().unwrap();
    let state = r.polled("pf", job).await;
    assert_eq!(state["state"], "running", "{state}");
    hold.store(false, Ordering::SeqCst);
    assert_eq!(done(&r, "pf", &a).await["state"], "done");
    let _ = std::fs::remove_dir_all(&r.b.dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_browser_that_fails_ends_the_job_with_its_sentence_and_leaves_no_film() {
    let r = rig("vh-pic-fail", bound()).await;
    let fake = Arc::new(Fake {
        fail: Some("the page threw".into()),
        ..Fake::default()
    });
    let (_, a) = go(&r, "pg", fake, ask(json!({}))).await;
    let end = done(&r, "pg", &a).await;
    assert_eq!(
        (end["state"].as_str(), end["error"].as_str()),
        (Some("failed"), Some("the page threw"))
    );
    assert!(!films(&r).join("p.webm").exists());
    // The slot is given back: the next ask renders.
    let (_, a) = go(&r, "pg", Arc::new(Fake::default()), ask(json!({}))).await;
    assert_eq!(done(&r, "pg", &a).await["state"], "done");
    let _ = std::fs::remove_dir_all(&r.b.dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_front_without_a_browser_on_the_machine_says_where_it_looked() {
    let r = rig("vh-pic-nochrome", bound()).await;
    let mut ctx = r.b.ctx(Member::Picture, false);
    ctx.account = "ph".into();
    ctx.verbs = drive::production_with(Some(r.jobs()), None, Arc::new(Fake::default()));
    let env = &mut ctx.docs.as_mut().unwrap().env;
    env.insert(
        "STUDIO_CHROMIUM".into(),
        r.b.dir.join("nope").display().to_string(),
    );
    let (s, a) =
        r.b.send(
            Member::Picture,
            "",
            ask(json!({ "into": films(&r).display().to_string() })),
            Some(ctx),
        )
        .await;
    assert_eq!(s, 202, "{a}");
    let end = done(&r, "ph", &a).await;
    assert_eq!(end["state"], "failed", "{end}");
    assert!(
        end["error"]
            .as_str()
            .unwrap()
            .contains("STUDIO_CHROMIUM names"),
        "{end}"
    );
    let _ = std::fs::remove_dir_all(&r.b.dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_film_with_no_folder_to_land_in_is_refused() {
    let r = rig("vh-pic-nofolder", bound()).await;
    let mut body = ask(json!({}));
    let mut ctx = r.b.ctx(Member::Picture, false);
    ctx.account = "pi".into();
    ctx.verbs = drive::production_with(Some(r.jobs()), None, Arc::new(Fake::default()));
    let (s, a) = r.b.send(Member::Picture, "", body.take(), Some(ctx)).await;
    assert_eq!(s, 400, "{a}");
    assert!(
        a["error"].as_str().unwrap().starts_with("into must be"),
        "{a}"
    );
    let _ = std::fs::remove_dir_all(&r.b.dir);
}
