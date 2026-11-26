//! `POST /picture` and `POST /picture/scene` through a real account and `dispatch`, over e2e's `video-lane.k7` on the disk.
//! What a reader would get wrong: the paths are the page's, workspace-relative, and a scene's are its `/media` URLs.

use std::path::Path;

use serde_json::{Value, json};

use super::Tag;
use super::docs::Docs;
use super::tests_account::{call, start, text};
use super::tests_docs::code_root;
use super::tests_journal::Bed;
use crate::routes::files::service::routes::{self, H};

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        let (src, dst) = (e.path(), to.join(e.file_name()));
        if src.is_dir() {
            copy_tree(&src, &dst);
        } else {
            std::fs::copy(&src, &dst).unwrap();
        }
    }
}

async fn ask(b: &Bed, tag: Tag, h: H, body: Value) -> (u16, Value) {
    let acct = start(&b.dir);
    let mut ctx = super::tests::ctx(&b.dir, tag, "", &b.lines);
    ctx.docs = Some(Docs::of(&acct, &code_root()));
    let c = call(&b.dir, "", body, Some(ctx));
    let (status, t) = text(routes::dispatch(h, &c).await).await;
    (status, serde_json::from_str(&t).unwrap_or(Value::Null))
}

async fn video_lane(name: &str) -> Bed {
    let b = Bed::new(name).await;
    copy_tree(
        &code_root().join("e2e/fixtures/video-lane.k7"),
        Path::new(&b.project),
    );
    b
}

// The document names each shot's file under the workspace, and the scene makes it the page's media URL.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_picture_and_its_scene_are_read_off_the_open_project() {
    let b = video_lane("picture-doc").await;
    let (status, d) = ask(&b, Tag::Picture, H::Picture, json!({})).await;
    assert_eq!(status, 200, "{d}");
    assert_eq!(d["warnings"], json!([]));
    assert_eq!(
        d["doc"]["videoTracks"][0]["clips"][0]["src"],
        "p.k7/clips/sample.mp4"
    );
    let body = json!({ "media": "https://h/media?token=t&path=", "use": "export" });
    let (status, s) = ask(&b, Tag::PictureScene, H::PictureScene, body).await;
    assert_eq!(status, 200, "{s}");
    assert_eq!(s["mixes"], json!(false));
    assert_eq!(
        s["scene"]["layers"][0]["clips"][0]["src"],
        "https://h/media?token=t&path=p.k7%2Fclips%2Fsample.mp4"
    );
    let (status, e) = ask(
        &b,
        Tag::PictureScene,
        H::PictureScene,
        json!({ "use": "x", "media": "" }),
    )
    .await;
    assert_eq!(
        (status, e["error"].clone()),
        (400, json!("use must be preview or export"))
    );
}

// A picture that does not build is the compile's sentence, as an answer.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_picture_that_does_not_build_answers_its_sentence() {
    let b = video_lane("picture-broken").await;
    let root = Path::new(&b.project).join("project.k7x");
    let text = std::fs::read_to_string(&root)
        .unwrap()
        .replace("viewport: [1280, 720, 30],", "");
    std::fs::write(&root, text).unwrap();
    let (status, d) = ask(&b, Tag::Picture, H::Picture, json!({})).await;
    assert_eq!(status, 200);
    assert!(
        d["error"].as_str().is_some_and(|e| e.contains("viewport")),
        "{d}"
    );
}
