//! A take kept in the project's record directory (2450 to 2453): the record directory's real paths decide what is read,
//! and a mirrored take must land first. What a reader gets wrong: nothing is pushed anywhere; the files stay.

use std::path::{Path, PathBuf};
use std::time::Duration;

use k7_file_service::media::names::outside_sentence;

use super::Counts;
use super::take::{Cfg, Request, Saved, save_take};
use super::testkit::scratch;

struct Rig {
    rec: PathBuf,
    cfg: Cfg,
    counts: Counts,
}

/// A project folder holding a record directory.
fn rig(name: &str) -> Rig {
    let proj = scratch(name);
    let rec = proj.join("rec");
    std::fs::create_dir_all(&rec).unwrap();
    let cfg = Cfg {
        roots: vec![proj],
        landing: Duration::from_millis(400),
        look: Duration::from_millis(25),
    };
    Rig {
        rec,
        cfg,
        counts: Counts::default(),
    }
}

fn ask(rec: &Path, file: Option<&Path>, video: Option<&Path>) -> Request {
    let s = |p: &Path| p.display().to_string();
    Request {
        record_dir: s(rec),
        file: file.map(s),
        video: video.map(s),
    }
}

fn put(path: &Path, bytes: &[u8]) {
    std::fs::write(path, bytes).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_take_stays_in_the_record_directory_and_is_named_and_counted() {
    let r = rig("take-keep");
    let take = r.rec.join("take one.wav");
    put(&take, &[7u8; 5000]);
    let saved = save_take(&r.cfg, &r.counts, &ask(&r.rec, Some(&take), None))
        .await
        .unwrap();
    assert_eq!(
        saved,
        Saved {
            path: "take one.wav".into(),
            bytes: 5000
        }
    );
    assert!(take.exists());
    assert_eq!(r.counts.takes.load(std::sync::atomic::Ordering::Relaxed), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_picture_stays_beside_the_take_and_alone_it_is_the_first_file() {
    let r = rig("take-video");
    let (take, pic) = (r.rec.join("a.wav"), r.rec.join("a.webm"));
    put(&take, b"sound");
    put(&pic, b"picture!");
    let saved = save_take(&r.cfg, &r.counts, &ask(&r.rec, Some(&take), Some(&pic)))
        .await
        .unwrap();
    assert_eq!(saved.path, "a.wav");
    assert!(pic.exists() && take.exists());
    let alone = save_take(&r.cfg, &r.counts, &ask(&r.rec, None, Some(&pic)))
        .await
        .unwrap();
    assert_eq!((alone.path.as_str(), alone.bytes), ("a.webm", 8));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_file_outside_the_record_directory_is_refused_with_the_typescripts_sentence() {
    let r = rig("take-outside");
    let other = scratch("take-outside-other").join("secret.wav");
    put(&other, b"secret");
    let long = "x".repeat(64 * 1024);
    for file in [
        format!("{}/../x.wav", r.rec.display()),
        other.display().to_string(),
        "..".into(),
        format!("{}/..", r.rec.display()),
        "a\0b".into(),
        long,
    ] {
        let req = Request {
            record_dir: r.rec.display().to_string(),
            file: Some(file.clone()),
            video: None,
        };
        let err = save_take(&r.cfg, &r.counts, &req).await.unwrap_err();
        assert_eq!(err, outside_sentence(&file));
    }
    let req = Request {
        record_dir: r.rec.display().to_string(),
        file: None,
        video: Some(other.display().to_string()),
    };
    assert_eq!(
        save_take(&r.cfg, &r.counts, &req).await.unwrap_err(),
        outside_sentence(&other.display().to_string())
    );
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_link_in_the_record_directory_never_uploads_what_it_names() {
    let r = rig("take-link");
    let out = scratch("take-link-out").join("secret.wav");
    put(&out, b"the server's own file");
    let (take, pic) = (r.rec.join("link.wav"), r.rec.join("link.webm"));
    std::os::unix::fs::symlink(&out, &take).unwrap();
    std::os::unix::fs::symlink(&out, &pic).unwrap();
    let err = save_take(&r.cfg, &r.counts, &ask(&r.rec, Some(&take), None))
        .await
        .unwrap_err();
    assert_eq!(err, outside_sentence(&take.display().to_string()));
    let err = save_take(&r.cfg, &r.counts, &ask(&r.rec, None, Some(&pic)))
        .await
        .unwrap_err();
    assert_eq!(err, outside_sentence(&pic.display().to_string()));
    // A record directory that is itself a link out of the project's roots reads nothing either.
    let linked = r.rec.parent().unwrap().join("linked");
    let elsewhere = scratch("take-link-dir");
    put(&elsewhere.join("t.wav"), b"x");
    std::os::unix::fs::symlink(&elsewhere, &linked).unwrap();
    let err = save_take(
        &r.cfg,
        &r.counts,
        &ask(&linked, Some(&linked.join("t.wav")), None),
    )
    .await
    .unwrap_err();
    assert!(err.contains("is not one of this project's takes"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_mirrored_take_is_filed_once_it_has_landed_and_one_still_growing_is_refused() {
    let mut r = rig("take-landing");
    // Looks wider than the writer's pause, so a half file is never held between two of them.
    r.cfg.look = Duration::from_millis(100);
    r.cfg.landing = Duration::from_millis(700);
    let mirror = PathBuf::from("/mirror/of/the/project/take.wav");
    let there = r.rec.join("take.wav");
    let p = there.clone();
    let w = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(80)).await;
        put(&p, b"half");
        tokio::time::sleep(Duration::from_millis(60)).await;
        put(&p, b"the whole take");
    });
    let saved = save_take(&r.cfg, &r.counts, &ask(&r.rec, Some(&mirror), None))
        .await
        .unwrap();
    w.await.unwrap();
    assert_eq!(saved.bytes, 14);
    // A take that never stops growing is never held between two looks: the wait ends in the refusal.
    let grow = r.rec.join("grow.wav");
    let g = grow.clone();
    let writer = tokio::spawn(async move {
        for n in 1..400 {
            put(&g, &vec![1u8; n]);
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    });
    let want = PathBuf::from("/mirror/grow.wav");
    let err = save_take(&r.cfg, &r.counts, &ask(&r.rec, Some(&want), None))
        .await
        .unwrap_err();
    assert_eq!(err, outside_sentence("/mirror/grow.wav"));
    writer.abort();
    let absent = PathBuf::from("/mirror/absent.wav");
    assert_eq!(
        save_take(&r.cfg, &r.counts, &ask(&r.rec, Some(&absent), None))
            .await
            .unwrap_err(),
        outside_sentence("/mirror/absent.wav")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_save_with_no_file_says_nothing_was_recorded() {
    let r = rig("take-none");
    let none = Request {
        record_dir: r.rec.display().to_string(),
        file: None,
        video: None,
    };
    assert_eq!(
        save_take(&r.cfg, &r.counts, &none).await.unwrap_err(),
        "nothing was recorded"
    );
}
