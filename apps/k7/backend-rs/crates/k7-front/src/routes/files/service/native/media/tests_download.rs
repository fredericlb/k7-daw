//! The model downloads' client and registry (2440 to 2445) against a raw server on loopback: a model arrives whole or not at
//! all, redirects stay http(s), the encoding is identity, and a destination is fetched once however many ask.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use k7_file_service::media::models::Job;

use super::download::{Downloads, IDLE};
use super::testkit::{Reply, Seen, scratch, serve};
use super::{Clock, Counts};

fn downloads(root: &Path, idle: Duration) -> (Downloads, Arc<AtomicUsize>) {
    let moved = Arc::new(AtomicUsize::new(0));
    let m = moved.clone();
    let clock: Clock = Arc::new(std::time::Instant::now);
    let d = Downloads::new(
        root.to_path_buf(),
        idle,
        clock,
        Arc::new(Counts::default()),
        Arc::new(move || {
            m.fetch_add(1, Ordering::Relaxed);
        }),
    );
    (d, moved)
}

async fn finished(d: &Downloads, id: &str) -> Job {
    for _ in 0..400 {
        let j = d.job(id).expect("the job is held");
        if j.done {
            return j;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("the download never finished");
}

fn parts(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .map(|r| {
            r.flatten()
                .map(|e| e.path())
                .filter(|p| p.to_string_lossy().contains(".part-"))
                .collect()
        })
        .unwrap_or_default()
}

fn body(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i % 251) as u8).collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_model_arrives_whole_and_asking_twice_fetches_nothing() {
    let root = scratch("dl-whole");
    let want = body(100_000);
    let w = want.clone();
    let (url, seen) = serve(move |_| Reply::ok(&w)).await;
    let (d, moved) = downloads(&root, IDLE);
    let job = d.start("sub/m.onnx", &format!("{url}/m"));
    let done = finished(&d, &job.id).await;
    assert_eq!(
        (done.error, done.received, done.total),
        (None, 100_000, Some(100_000))
    );
    assert_eq!(std::fs::read(root.join("sub/m.onnx")).unwrap(), want);
    assert!(parts(&root.join("sub")).is_empty());
    assert_eq!(
        moved.load(Ordering::Relaxed),
        1,
        "the module watch is told once"
    );
    let again = d.start("sub/m.onnx", &format!("{url}/m"));
    assert!(again.done && again.error.is_none());
    assert_eq!(seen.lock().unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_request_asks_for_the_identity_encoding() {
    let root = scratch("dl-identity");
    let (url, _) = serve(|s: &Seen| {
        if s.head
            .to_ascii_lowercase()
            .contains("accept-encoding: identity")
        {
            Reply::ok(b"plain model")
        } else {
            Reply::with(200, &[("Content-Encoding", "gzip")], b"\x1f\x8b gzipped")
        }
    })
    .await;
    let (d, _) = downloads(&root, IDLE);
    let job = d.start("m.onnx", &format!("{url}/m"));
    let done = finished(&d, &job.id).await;
    assert_eq!(done.error, None);
    assert_eq!(std::fs::read(root.join("m.onnx")).unwrap(), b"plain model");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_server_that_sends_it_compressed_anyway_is_refused_and_writes_nothing() {
    let root = scratch("dl-gzip");
    let (url, _) = serve(|_| Reply::with(200, &[("Content-Encoding", "gzip")], b"\x1f\x8b")).await;
    let (d, _) = downloads(&root, IDLE);
    let job = d.start("m.onnx", &format!("{url}/m"));
    let done = finished(&d, &job.id).await;
    assert_eq!(
        done.error.as_deref(),
        Some("could not download m.onnx: the server sent it compressed (gzip)")
    );
    assert!(!root.join("m.onnx").exists() && parts(&root).is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_body_short_of_its_length_is_a_failed_download_never_a_small_model() {
    let root = scratch("dl-short");
    // Closed early, and chunked with a length that disagrees: the second is the check's own case.
    let (url, _) = serve(|s: &Seen| {
        if s.head.starts_with("GET /chunked") {
            let mut b = format!("{:x}\r\n", 400).into_bytes();
            b.extend(body(400));
            b.extend_from_slice(b"\r\n0\r\n\r\n");
            Reply::with(
                200,
                &[("Transfer-Encoding", "chunked"), ("Content-Length", "1000")],
                &b,
            )
        } else {
            Reply::Is {
                status: 200,
                headers: vec![],
                body: body(400),
                announce: Some(1000),
            }
        }
    })
    .await;
    let (d, _) = downloads(&root, IDLE);
    for path in ["closed", "chunked"] {
        let file = format!("{path}.onnx");
        let job = d.start(&file, &format!("{url}/{path}"));
        let done = finished(&d, &job.id).await;
        assert!(done.error.is_some(), "{path}: {done:?}");
        assert!(
            !root.join(&file).exists(),
            "{path}: no file under the final name"
        );
    }
    assert!(parts(&root).is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn redirects_are_followed_to_a_limit_and_only_to_http() {
    let root = scratch("dl-redirect");
    let (url, _) = serve(
        |s: &Seen| match s.head.split_whitespace().nth(1).unwrap_or("") {
            "/a" => Reply::with(302, &[("Location", "/b")], b""),
            "/b" => Reply::with(307, &[("Location", "/final")], b""),
            "/final" => Reply::ok(b"model"),
            "/loop" => Reply::with(302, &[("Location", "/loop")], b""),
            "/file" => Reply::with(302, &[("Location", "file:///etc/passwd")], b""),
            "/ftp" => Reply::with(302, &[("Location", "ftp://example.com/m")], b""),
            _ => Reply::with(404, &[], b"no"),
        },
    )
    .await;
    let (d, _) = downloads(&root, IDLE);
    let run = |file: &'static str, path: &'static str| {
        let job = d.start(file, &format!("{url}/{path}"));
        (job.id, file)
    };
    let (ok, _) = run("ok.onnx", "a");
    assert_eq!(finished(&d, &ok).await.error, None);
    assert_eq!(std::fs::read(root.join("ok.onnx")).unwrap(), b"model");
    for (file, path, says) in [
        ("loop.onnx", "loop", "redirected more than 20 times"),
        ("file.onnx", "file", "is not an http(s) URL"),
        ("ftp.onnx", "ftp", "is not an http(s) URL"),
        ("gone.onnx", "gone", "answered 404"),
    ] {
        let (id, _) = run(file, path);
        let err = finished(&d, &id).await.error.unwrap_or_default();
        assert!(
            err.starts_with(&format!("could not download {file}: ")) && err.contains(says),
            "{err}"
        );
        assert!(!root.join(file).exists());
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_server_that_never_answers_or_stops_halfway_fails_the_job_and_leaves_nothing() {
    let root = scratch("dl-stall");
    let (url, _) = serve(|s: &Seen| {
        if s.head.starts_with("GET /half") {
            Reply::Stall {
                announce: 1000,
                body: body(500),
            }
        } else {
            Reply::Hang
        }
    })
    .await;
    let (d, _) = downloads(&root, Duration::from_millis(400));
    for (file, path) in [("never.onnx", "never"), ("half.onnx", "half")] {
        let job = d.start(file, &format!("{url}/{path}"));
        let done = finished(&d, &job.id).await;
        assert!(done.error.is_some(), "{path}");
        assert!(!root.join(file).exists());
    }
    assert!(
        parts(&root).is_empty(),
        "the part of a stalled body is removed"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_name_that_leaves_the_cache_is_refused_without_a_request() {
    let root = scratch("dl-names");
    let (url, seen) = serve(|_| Reply::ok(b"x")).await;
    let (d, _) = downloads(&root, IDLE);
    for file in ["../x", "/abs/m.onnx", "a/../../x", ""] {
        let job = d.start(file, &format!("{url}/m"));
        assert!(job.done, "{file}");
        assert_eq!(
            job.error,
            Some(format!(
                "'{file}' is not a name a model can have in the cache"
            ))
        );
    }
    assert!(seen.lock().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_thousand_asks_for_one_file_are_one_job_and_one_request() {
    let root = scratch("dl-once");
    let (url, seen) = serve(|_| Reply::Stall {
        announce: 10,
        body: vec![0; 3],
    })
    .await;
    let (d, _) = downloads(&root, Duration::from_secs(30));
    let first = d.start("m.onnx", &format!("{url}/m")).id;
    for _ in 0..1000 {
        assert_eq!(d.start("m.onnx", &format!("{url}/m")).id, first);
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(seen.lock().unwrap().len(), 1);
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_models_folder_that_is_a_link_out_of_the_root_is_never_written_into() {
    let root = scratch("dl-link");
    let out = scratch("dl-link-out");
    std::os::unix::fs::symlink(&out, root.join("sub")).unwrap();
    let (url, _) = serve(|_| Reply::ok(b"model")).await;
    let (d, _) = downloads(&root, IDLE);
    let job = d.start("sub/deeper/m.onnx", &format!("{url}/m"));
    let done = finished(&d, &job.id).await;
    assert!(
        done.error
            .is_some_and(|e| e.contains("outside the models root"))
    );
    assert_eq!(
        std::fs::read_dir(&out).unwrap().count(),
        0,
        "nothing was made out there"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_part_left_by_a_killed_front_goes_when_a_download_starts_in_its_folder() {
    let root = scratch("dl-parts");
    let old = root.join("m.onnx.part-00000000-0000-4000-8000-000000000000");
    let fresh = root.join("n.onnx.part-11111111-1111-4111-8111-111111111111");
    let other = root.join("keep.part-notauuid");
    for f in [&old, &fresh, &other] {
        std::fs::write(f, b"half").unwrap();
    }
    let ago = std::time::SystemTime::now() - Duration::from_secs(2 * 3600);
    std::fs::File::options()
        .write(true)
        .open(&old)
        .unwrap()
        .set_modified(ago)
        .unwrap();
    std::fs::File::options()
        .write(true)
        .open(&other)
        .unwrap()
        .set_modified(ago)
        .unwrap();
    let (url, _) = serve(|_| Reply::ok(b"model")).await;
    let (d, _) = downloads(&root, IDLE);
    let job = d.start("m.onnx", &format!("{url}/m"));
    finished(&d, &job.id).await;
    assert!(!old.exists() && fresh.exists() && other.exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_accounts_naming_one_model_fetch_it_once_and_never_poll_each_others_job() {
    let root = scratch("dl-two");
    let (url, seen) = serve(|_| Reply::ok(&body(50_000))).await;
    let (a, _) = downloads(&root, IDLE);
    let (b, moved_b) = downloads(&root, IDLE);
    let (ja, jb) = (
        a.start("m.onnx", &format!("{url}/m")),
        b.start("m.onnx", &format!("{url}/m")),
    );
    assert_ne!(ja.id, jb.id);
    assert!(
        a.job(&jb.id).is_none() && b.job(&ja.id).is_none(),
        "an id of another account is not found"
    );
    assert_eq!(finished(&a, &ja.id).await.error, None);
    assert_eq!(finished(&b, &jb.id).await.error, None);
    assert_eq!(
        seen.lock().unwrap().len(),
        1,
        "one download for the destination"
    );
    assert_eq!(moved_b.load(Ordering::Relaxed), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_account_with_a_download_running_is_busy_and_is_not_one_once_it_ended() {
    let root = scratch("dl-busy");
    let (url, _) = serve(|_: &Seen| Reply::Hang).await;
    let (d, _) = downloads(&root, Duration::from_millis(300));
    assert_eq!(d.running(), 0);
    let job = d.start("slow.onnx", &format!("{url}/x"));
    assert_eq!(d.running(), 1, "a reap must keep the runtime that holds it");
    finished(&d, &job.id).await;
    assert_eq!(d.running(), 0);
}
