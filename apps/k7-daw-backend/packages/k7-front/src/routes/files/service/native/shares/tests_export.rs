//! tests_export.rs — `/export`: the mixdown streamed from a private folder that is gone with the body (2520 to 2524).

use http_body_util::BodyExt;
use k7_file_service::shares::MP3_NEEDS_FFMPEG;

use super::export::{Format, export_mix};
use super::fakes::*;

fn left(a: &super::Account) -> Vec<String> {
    std::fs::read_dir(&a.tmp)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect()
}

#[tokio::test]
async fn a_wav_is_streamed_whole_and_its_folder_goes_with_the_body() {
    let base = scratch("wav");
    let a = account(&base, &[]);
    let w = world();
    let m = export_mix(&a, &w.ports, &a.root, Format::Wav, None)
        .await
        .unwrap();
    assert_eq!((m.filename.as_str(), m.mime), ("Song.wav", "audio/wav"));
    assert_eq!(left(&a).len(), 1);
    let (body, len) = m.into_body().await.unwrap();
    let bytes = body.collect().await.unwrap().to_bytes();
    assert_eq!((bytes.as_ref(), len), (&b"RIFFwav"[..], 7));
    assert!(left(&a).is_empty(), "{:?}", left(&a));
    let _ = std::fs::remove_dir_all(&base);
}

/// 2522: a client that never reads the body leaves no folder.
#[tokio::test]
async fn a_body_dropped_unread_removes_the_folder() {
    let base = scratch("drop");
    let a = account(&base, &[]);
    let w = world();
    let m = export_mix(&a, &w.ports, &a.root, Format::Wav, Some("t1"))
        .await
        .unwrap();
    assert_eq!(m.filename, "Song-t1.wav");
    let (body, _) = m.into_body().await.unwrap();
    drop(body);
    assert!(left(&a).is_empty(), "{:?}", left(&a));
    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test]
async fn a_refused_timeline_and_a_failed_render_leave_nothing() {
    let base = scratch("fail");
    let a = account(&base, &[]);
    let w = world();
    let e = export_mix(&a, &w.ports, &a.root, Format::Wav, Some("nope"))
        .await
        .err()
        .unwrap();
    assert_eq!(e, "no timeline \"nope\" to export");
    let mut p = w.ports;
    let ports = std::sync::Arc::get_mut(&mut p).unwrap();
    ports.mixdown = std::sync::Arc::new(FakeMixdown {
        calls: w.calls.clone(),
        fail: Some("engine died".into()),
    });
    let e = export_mix(&a, &p, &a.root, Format::Wav, None)
        .await
        .err()
        .unwrap();
    assert_eq!(e, "engine died");
    assert!(left(&a).is_empty(), "{:?}", left(&a));
    let _ = std::fs::remove_dir_all(&base);
}

/// 2523: no ffmpeg is the TypeScript's sentence, and the folder is still removed.
#[tokio::test]
async fn an_mp3_without_ffmpeg_says_so_and_leaves_nothing() {
    let base = scratch("noff");
    let a = account(&base, &[("PATH", "/nonexistent")]);
    let w = world();
    let e = export_mix(&a, &w.ports, &a.root, Format::Mp3, None)
        .await
        .err()
        .unwrap();
    assert_eq!(e, MP3_NEEDS_FFMPEG);
    assert!(left(&a).is_empty());
    let _ = std::fs::remove_dir_all(&base);
}

/// 2523: ffmpeg runs under the account's environment and none of the front's.
#[cfg(unix)]
#[tokio::test]
async fn an_mp3_is_encoded_by_the_accounts_ffmpeg_with_the_accounts_environment() {
    let base = scratch("mp3");
    let bin = fake_ffmpeg(&base);
    let path = bin.display().to_string();
    let a = account(&base, &[("PATH", &path), ("ACCOUNT_MARK", "yes")]);
    let w = world();
    let m = export_mix(&a, &w.ports, &a.root, Format::Mp3, None)
        .await
        .unwrap();
    assert_eq!((m.filename.as_str(), m.mime), ("Song.mp3", "audio/mpeg"));
    let seen = std::fs::read_to_string(base.join("ffmpeg-env.txt")).unwrap();
    assert!(seen.lines().any(|l| l == "ACCOUNT_MARK=yes"), "{seen}");
    assert!(!seen.lines().any(|l| l.starts_with("HOME=")), "{seen}");
    let (body, _) = m.into_body().await.unwrap();
    assert_eq!(
        body.collect().await.unwrap().to_bytes().as_ref(),
        b"mp3bytes"
    );
    let _ = std::fs::remove_dir_all(&base);
}

/// A driver that says when the asker gave up.
#[derive(Default)]
struct Hears(std::sync::Arc<std::sync::atomic::AtomicBool>);

impl crate::routes::files::service::native::jobs::Driver for Hears {
    fn renderer_available(&self) -> bool {
        true
    }
    fn bound(&self) -> bool {
        false
    }
    fn render_run(
        &self,
        _args: serde_json::Value,
        _reply: crate::routes::files::service::native::callbacks::Reply,
    ) {
    }
    fn render_run_until(
        &self,
        _args: serde_json::Value,
        _reply: crate::routes::files::service::native::callbacks::Reply,
        gone: crate::routes::files::service::native::jobs::Gone,
    ) {
        let heard = self.0.clone();
        tokio::spawn(async move {
            gone.await;
            heard.store(true, std::sync::atomic::Ordering::SeqCst);
        });
    }
    fn render_all(
        &self,
        _dir: String,
        _reply: crate::routes::files::service::native::callbacks::Reply,
    ) {
    }
    fn separate(&self, _id: &str, _generation: u64, _args: serde_json::Value) {}
}

#[tokio::test]
async fn a_dropped_mixdown_asks_its_render_to_stop() {
    use super::Mixdown;
    let hears = Hears::default();
    let heard = hears.0.clone();
    let jobs =
        crate::routes::files::service::native::jobs::Jobs::new(Some(std::sync::Arc::new(hears)));
    let mixdown = super::ports::JobsMixdown(jobs);
    let out = std::path::Path::new("/nowhere");
    let doc = serde_json::json!({});
    let ask = mixdown.render(&doc, out, false);
    let _ = tokio::time::timeout(std::time::Duration::from_millis(100), ask).await;
    for _ in 0..100 {
        if heard.load(std::sync::atomic::Ordering::SeqCst) {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    panic!("the render was never told to stop");
}
