//! The model registry's machine (2440, 2442): the refusals the TypeScript answers, one job per destination, a body held to
//! its announced size, and a finished job kept ten minutes on a clock the test turns.

use std::time::{Duration, Instant};

use k7_file_service::media::KEEP;
use k7_file_service::media::models::{Fetch, FetchEvent, Job, Models, uuid_v4};
use k7_file_service::media::names::model_cache_path;

fn models() -> Models {
    let mut n = 0u8;
    Models::new(Box::new(move || {
        n += 1;
        [n; 16]
    }))
}

fn start(m: &mut Models, file: &str, url: &str, now: Instant) -> (Job, Option<Fetch>) {
    m.start(
        file,
        url,
        model_cache_path("/m", file).as_deref(),
        false,
        now,
    )
}

#[test]
fn a_job_id_is_a_version_4_uuid() {
    let id = uuid_v4([0xff; 16]);
    assert_eq!(id, "ffffffff-ffff-4fff-bfff-ffffffffffff");
    assert_eq!(uuid_v4([0; 16]), "00000000-0000-4000-8000-000000000000");
}

#[test]
fn a_download_runs_to_a_rename_and_one_destination_has_one_job() {
    let t0 = Instant::now();
    let mut m = models();
    let (job, fetch) = start(&mut m, "sub/m.onnx", "https://h/m", t0);
    let fetch = fetch.expect("a fetch");
    assert_eq!(
        (fetch.id.as_str(), fetch.url.as_str(), fetch.dest.as_str()),
        (job.id.as_str(), "https://h/m", "/m/sub/m.onnx")
    );
    // The same file under another spelling is the same destination: the job in flight is answered, nothing else fetched.
    let (again, none) = start(&mut m, "sub/./m.onnx", "https://h/m", t0);
    assert!(none.is_none());
    assert_eq!(again.id, job.id);
    m.on(&job.id, FetchEvent::Headers { total: Some(10) }, t0);
    m.on(&job.id, FetchEvent::Bytes(4), t0);
    m.on(&job.id, FetchEvent::Bytes(6), t0);
    assert_eq!(m.complete(&job.id), Ok(()));
    m.on(&job.id, FetchEvent::Renamed, t0);
    let done = m.job(&job.id, t0).unwrap();
    assert_eq!(
        (done.received, done.total, done.done, done.error),
        (10, Some(10), true, None)
    );
    // A finished job is not run again: the next ask makes a new one.
    let (next, fetch) = start(&mut m, "sub/m.onnx", "https://h/m", t0);
    assert!(fetch.is_some());
    assert_ne!(next.id, job.id);
}

#[test]
fn a_short_body_is_a_failure_not_a_small_model() {
    let t0 = Instant::now();
    let mut m = models();
    let (job, _) = start(&mut m, "m.onnx", "https://h/m", t0);
    m.on(&job.id, FetchEvent::Headers { total: Some(100) }, t0);
    m.on(&job.id, FetchEvent::Bytes(40), t0);
    let why = m.complete(&job.id).unwrap_err();
    assert_eq!(why, "the download stopped at 40 of 100 bytes");
    m.on(&job.id, FetchEvent::Failed(why), t0);
    let j = m.job(&job.id, t0).unwrap();
    assert_eq!(
        j.error.as_deref(),
        Some("could not download m.onnx: the download stopped at 40 of 100 bytes")
    );
    // With no announced size nothing can be short.
    let (b, _) = start(&mut m, "b.onnx", "https://h/m", t0);
    m.on(&b.id, FetchEvent::Bytes(3), t0);
    assert_eq!(m.complete(&b.id), Ok(()));
}

#[test]
fn events_after_the_end_are_ignored_and_the_first_end_wins() {
    let t0 = Instant::now();
    let mut m = models();
    let (job, _) = start(&mut m, "m.onnx", "https://h/m", t0);
    m.on(&job.id, FetchEvent::Failed("reset".into()), t0);
    m.on(&job.id, FetchEvent::Renamed, t0);
    m.on(&job.id, FetchEvent::Bytes(9), t0);
    let j = m.job(&job.id, t0).unwrap();
    assert_eq!(j.error.as_deref(), Some("could not download m.onnx: reset"));
    assert_eq!(j.received, 0);
    m.on("no such job", FetchEvent::Renamed, t0);
}

#[test]
fn a_finished_job_is_kept_ten_minutes_on_the_clock_it_is_given() {
    let t0 = Instant::now();
    let mut m = models();
    let (job, _) = start(&mut m, "m.onnx", "https://h/m", t0);
    // A running job never expires, however late it is read.
    assert!(m.job(&job.id, t0 + KEEP * 3).is_some());
    let end = t0 + Duration::from_secs(5);
    m.on(&job.id, FetchEvent::Renamed, end);
    assert!(
        m.job(&job.id, end + KEEP - Duration::from_millis(1))
            .is_some()
    );
    assert!(m.job(&job.id, end + KEEP).is_none());
    // The registry lets go of it when it is next touched.
    start(&mut m, "other.onnx", "https://h/m", end + KEEP);
    assert_eq!(m.len(), 1);
}

#[test]
fn a_refused_job_is_kept_like_any_other() {
    let t0 = Instant::now();
    let mut m = models();
    let (job, _) = start(&mut m, "../x", "https://h/m", t0);
    assert!(job.done && job.error.is_some());
    assert!(m.job(&job.id, t0 + KEEP / 2).is_some());
    assert!(m.job(&job.id, t0 + KEEP).is_none());
}
