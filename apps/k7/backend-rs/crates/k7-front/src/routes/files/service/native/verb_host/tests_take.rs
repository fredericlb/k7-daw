//! `take/save` behind the frame (3f, M6): the record directory read through the gate, the take kept where it is.
//! What a reader gets wrong: nothing leaves the project, and a take outside the record directory is refused.

use serde_json::json;

use super::Member;
use super::tests_drive::{Rig, bound, rig};

async fn go(r: &Rig, body: serde_json::Value) -> (u16, serde_json::Value) {
    let mut ctx = r.b.ctx(Member::TakeSave, false);
    ctx.account = "ta".into();
    ctx.verbs = super::drive::production(Some(r.jobs()));
    r.b.send(Member::TakeSave, "", body, Some(ctx)).await
}

fn song(r: &Rig, rel: &str) -> std::path::PathBuf {
    r.b.dir.join("home/workspace/songs/p.k7").join(rel)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_take_and_its_picture_stay_in_the_record_directory() {
    let r = rig("vh-take", bound()).await;
    std::fs::create_dir_all(song(&r, "rec")).unwrap();
    std::fs::write(song(&r, "rec/t1.wav"), b"sound").unwrap();
    std::fs::write(song(&r, "rec/up.webm"), b"film").unwrap();
    let body = json!({ "at": "p.k7", "recordDir": "/w/p.k7/rec", "file": "t1.wav", "video": "/w/p.k7/rec/up.webm" });
    let (s, a) = go(&r, body).await;
    assert_eq!(s, 200, "{a}");
    assert_eq!(a["path"], "t1.wav");
    assert!(song(&r, "rec/up.webm").exists() && song(&r, "rec/t1.wav").exists());
    let _ = std::fs::remove_dir_all(&r.b.dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_take_outside_the_record_directory_and_a_foreign_record_directory_are_refused() {
    let r = rig("vh-take-out", bound()).await;
    std::fs::create_dir_all(song(&r, "rec")).unwrap();
    std::fs::write(song(&r, "other.wav"), b"sound").unwrap();
    let at = |dir: &str, file: &str| json!({ "at": "p.k7", "recordDir": dir, "file": file });
    let real_other = song(&r, "other.wav").display().to_string();
    for (dir, file, want) in [
        ("/w/p.k7/rec", real_other.as_str(), 500),
        ("/w/p.k7/rec", "../other.wav", 500),
        ("/etc", "passwd", 403),
        ("/w/p.k7/rec", "", 400),
    ] {
        let (s, a) = go(&r, at(dir, file)).await;
        assert_eq!(s, want, "{dir} {file}: {a}");
    }
    let (s, _) = go(
        &r,
        json!({ "at": "p.k7", "recordDir": "/w/p.k7/rec", "video": "/etc/passwd" }),
    )
    .await;
    assert_eq!(s, 403);
    let _ = std::fs::remove_dir_all(&r.b.dir);
}
