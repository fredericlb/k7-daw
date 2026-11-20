//! `probe` and `extract` behind the frame (3f, M6): a fake ffprobe answers the facts, the real decoder writes a sound.
//! What a reader gets wrong: the fake ffprobe is a script, so these hold on Unix; the probe's reading is the vectors'.

use serde_json::json;

use super::Member;
use super::tests_drive::{Rig, bound, rig, wav};

#[cfg(unix)]
fn fake_ffprobe(dir: &std::path::Path) -> String {
    use std::os::unix::fs::PermissionsExt;
    let script = dir.join("ffprobe");
    // A file holding `film` is a film, one holding `still` has no video stream, anything else is unreadable.
    let body = r#"#!/bin/sh
for last; do :; done
f="${last#file:}"
case "$(cat "$f" 2>/dev/null)" in
  film) echo '{"streams":[{"codec_name":"h264","width":1280,"height":720,"pix_fmt":"yuv420p","avg_frame_rate":"25/1","side_data_list":[{"rotation":-90}]}],"format":{"duration":"2.5","format_name":"mov,mp4"}}' ;;
  still) echo '{"streams":[],"format":{"duration":"1"}}' ;;
  *) echo "$f: Invalid data found when processing input" >&2; exit 1 ;;
esac
"#;
    std::fs::write(&script, body).unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    script.display().to_string()
}

async fn go(
    r: &Rig,
    m: Member,
    body: serde_json::Value,
    ffprobe: Option<&str>,
) -> (u16, serde_json::Value) {
    let mut ctx = r.b.ctx(m, false);
    ctx.account = "ma".into();
    ctx.verbs = super::drive::production(Some(r.jobs()));
    if let (Some(d), Some(f)) = (ctx.docs.as_mut(), ffprobe) {
        d.env.insert("FFPROBE_BIN".into(), f.into());
    }
    r.b.send(m, "", body, Some(ctx)).await
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_probe_answers_the_facts_null_for_no_video_and_names_a_failure_by_the_browsers_path() {
    let r = rig("vh-probe", bound()).await;
    let ff = fake_ffprobe(&r.b.dir);
    let put = |name: &str, text: &str| {
        let p = r.b.dir.join("home/workspace/songs/p.k7").join(name);
        std::fs::write(p, text).unwrap();
    };
    put("a.mp4", "film");
    put("b.mp3", "still");
    put("c.bin", "junk");
    put("empty.mp4", "");
    let (s, a) = go(
        &r,
        Member::Probe,
        json!({ "file": "/w/p.k7/a.mp4" }),
        Some(&ff),
    )
    .await;
    assert_eq!(s, 200, "{a}");
    assert_eq!(
        a["facts"],
        json!({ "codec": "h264", "width": 1280, "height": 720, "pixFmt": "yuv420p", "fps": 25,
                "seconds": 2.5, "container": "mov,mp4", "rotation": 270 })
    );
    let (_, a) = go(
        &r,
        Member::Probe,
        json!({ "file": "/w/p.k7/b.mp3" }),
        Some(&ff),
    )
    .await;
    assert_eq!(a["facts"], serde_json::Value::Null);
    for (file, status) in [("c.bin", 500), ("empty.mp4", 500), ("none.mp4", 404)] {
        let shown = format!("/w/p.k7/{file}");
        let (s, a) = go(&r, Member::Probe, json!({ "file": shown }), Some(&ff)).await;
        assert_eq!(s, status, "{file}: {a}");
        assert!(
            !a.to_string().contains(&r.b.dir.display().to_string()),
            "{a}"
        );
    }
    let (s, _) = go(
        &r,
        Member::Probe,
        json!({ "file": "/etc/passwd" }),
        Some(&ff),
    )
    .await;
    assert_eq!(s, 403);
    let _ = std::fs::remove_dir_all(&r.b.dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_probe_with_no_ffprobe_reads_the_headers_or_says_unsupported() {
    let r = rig("vh-probe-none", bound()).await;
    std::fs::write(r.b.dir.join("home/workspace/songs/p.k7/a.mp4"), "film").unwrap();
    let missing = r.b.dir.join("no-ffprobe-here").display().to_string();
    let (s, a) = go(
        &r,
        Member::Probe,
        json!({ "file": "/w/p.k7/a.mp4" }),
        Some(&missing),
    )
    .await;
    assert_eq!((s, a["code"].as_str()), (415, Some("unsupported")), "{a}");
    assert!(a["error"].as_str().unwrap().contains("needs ffmpeg"));
    let film = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../k7-mediatek/tests/media/rotated.mp4"
    );
    std::fs::copy(film, r.b.dir.join("home/workspace/songs/p.k7/a.mp4")).unwrap();
    let (s, a) = go(
        &r,
        Member::Probe,
        json!({ "file": "/w/p.k7/a.mp4" }),
        Some(&missing),
    )
    .await;
    assert_eq!(
        (
            s,
            a["facts"]["codec"].as_str(),
            a["facts"]["rotation"].as_u64()
        ),
        (200, Some("h264"), Some(90)),
        "{a}"
    );
    let _ = std::fs::remove_dir_all(&r.b.dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_extract_writes_the_sound_into_the_jobs_folder_and_answers_its_facts_and_file() {
    let r = rig("vh-extract", bound()).await;
    wav(&r.b.dir.join("home/workspace/songs/p.k7/s.wav"), 800, 1000);
    let (s, a) = go(
        &r,
        Member::Extract,
        json!({ "video": "/w/p.k7/s.wav" }),
        None,
    )
    .await;
    assert_eq!(s, 202, "{a}");
    let end = r.ended("ma", a["job"].as_str().unwrap()).await;
    assert_eq!(end["state"], "done", "{end}");
    assert_eq!(end["value"]["facts"]["sampleRate"], 8000);
    assert_eq!(end["value"]["facts"]["frames"], 800);
    let file = &end["value"]["files"][0];
    assert_eq!(file["out"], "sound.wav");
    assert_eq!(file["len"], 44 + 800 * 4);
    assert!((file["seconds"].as_f64().unwrap() - 0.1).abs() < 1e-9);
    // A file that is no sound fails the job, and the folder goes with it.
    std::fs::write(r.b.dir.join("home/workspace/songs/p.k7/x.mp4"), "junk").unwrap();
    let (_, a) = go(
        &r,
        Member::Extract,
        json!({ "video": "/w/p.k7/x.mp4" }),
        None,
    )
    .await;
    let job = a["job"].as_str().unwrap().to_string();
    assert_eq!(r.ended("ma", &job).await["state"], "failed");
    let (s, _) = go(&r, Member::Extract, json!({ "video": "/etc/passwd" }), None).await;
    assert_eq!(s, 403);
    let _ = std::fs::remove_dir_all(&r.b.dir);
}
