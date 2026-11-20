//! tests_freezer_timer.rs — `start_freezer` on a paused clock (the 3 s timer, `STUDIO_AUTO_FREEZE=off`) and a freeze through a machine that fails.
//! What a reader gets wrong: the clock is paused only after the rig is up, and each wait spins in real time, since a blocking thread's answer is not a timer.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use k7_file_service::jobs::FREEZE_EVERY;
use serde_json::json;

use super::freezer::FreezerTask;
use super::tests_freezer::Stub;
use super::{Driver, Jobs};
use crate::routes::files::service::native::docs::Docs;
use crate::routes::files::service::native::engine::testkit::{Rig, until};
use crate::routes::files::service::native::tests_docs::{code_root, env_of, world};

/// Lets the runtime and its threads run, without moving the paused clock.
async fn settle(ms: u64) {
    let end = Instant::now() + Duration::from_millis(ms);
    while Instant::now() < end {
        tokio::task::yield_now().await;
        std::thread::sleep(Duration::from_millis(2));
    }
}

async fn settled(what: &str, f: impl Fn() -> bool) {
    let end = Instant::now() + Duration::from_secs(10);
    while !f() {
        assert!(Instant::now() < end, "timed out waiting for {what}");
        settle(10).await;
    }
}

struct Timer {
    jobs: Arc<Jobs>,
    task: FreezerTask,
    calls: Arc<AtomicUsize>,
    rig: Rig,
    _keep: std::path::PathBuf,
}

/// A served, ready project and a freezer started over it, the clock paused first.
async fn timer(name: &str, off: bool) -> Timer {
    let rig = Rig::new(name).await;
    let (dir, _) = world(name).await;
    let jobs = Jobs::new(Some(Arc::new(Stub::default()) as Arc<dyn Driver>));
    let mut env = env_of(&dir);
    if off {
        env.insert("STUDIO_AUTO_FREEZE".into(), "off".into());
    }
    // No `with_thawed`: `start_freezer` adds it itself.
    let docs = Docs::of_env(&code_root(), &env);
    let session = rig.session();
    session.start(&rig.ws, &rig.project).await;
    until(10, "the session ready", || session.is_ready()).await;
    tokio::time::pause();
    let task = jobs.start_freezer(rig.link.clone(), docs, &tokio::runtime::Handle::current());
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let freezing = jobs.freezing.get().unwrap();
    *freezing.fake.lock().unwrap() = Some(Arc::new(move |_| {
        count.fetch_add(1, Ordering::Relaxed);
        Ok(vec![])
    }));
    Timer {
        jobs,
        task,
        calls,
        rig,
        _keep: dir,
    }
}

#[tokio::test]
async fn the_freezer_ticks_every_three_seconds_reads_the_thawed_list_and_stops_when_dropped() {
    let t = timer("fzt-on", false).await;
    let freezing = t.jobs.freezing.get().unwrap().clone();
    assert!(t.task.armed() && freezing.reads_the_jobs_thawed_list());
    // The interval's first tick is due at its start; a sleep lets the paused clock and its driver move.
    tokio::time::sleep(Duration::from_millis(1)).await;
    settled("the first pass", || {
        t.calls.load(Ordering::Relaxed) == 1 && !freezing.running(&t.rig.project)
    })
    .await;
    t.jobs.thaw(&t.rig.project, "m", true);
    tokio::time::advance(FREEZE_EVERY - Duration::from_millis(5)).await;
    settle(200).await;
    assert_eq!(t.calls.load(Ordering::Relaxed), 1, "a tick before 3 s");
    tokio::time::advance(Duration::from_millis(5)).await;
    settled("the second pass", || {
        t.calls.load(Ordering::Relaxed) == 2 && !freezing.running(&t.rig.project)
    })
    .await;
    let calls = t.calls.clone();
    let (jobs, project) = (t.jobs.clone(), t.rig.project.clone());
    drop(t.task);
    jobs.thaw(&project, "n", true);
    tokio::time::advance(FREEZE_EVERY * 3).await;
    settle(200).await;
    assert_eq!(calls.load(Ordering::Relaxed), 2, "a tick after the drop");
}

#[tokio::test]
async fn auto_freeze_off_arms_no_timer_and_render_all_still_works() {
    let t = timer("fzt-off", true).await;
    assert!(!t.task.armed());
    tokio::time::advance(FREEZE_EVERY * 5).await;
    settle(200).await;
    assert_eq!(t.calls.load(Ordering::Relaxed), 0);
    let (tx, rx) = tokio::sync::oneshot::channel();
    let freezing = t.jobs.freezing.get().unwrap().clone();
    freezing.render_all(&t.rig.project, Box::new(move |r| drop(tx.send(r))));
    assert_eq!(rx.await.unwrap().unwrap()["value"], json!([]));
    assert_eq!(t.calls.load(Ordering::Relaxed), 1);
}

#[cfg(unix)]
#[tokio::test]
async fn a_freeze_a_machine_half_wrote_is_impossible_and_leaves_no_part_file_in_the_renders() {
    use crate::routes::files::service::native::engine::fake::{FakeJobs, JobEnd};

    use super::driver::{DriverEnv, real_driver};
    use super::tests_freezer::{fixture_on, freeze, inject, render_all};
    let machine = FakeJobs::start().await;
    machine.ends(JobEnd::WroteThenError("the plugin crashed".into()));
    let url = machine.url.clone();
    let f = fixture_on("fzt-part", Arc::new(Stub::default()), move |rig, dir| {
        rig.link
            .bind(&json!({ "url": rig.engine.url, "engineId": "e1", "jobsUrl": url }));
        let home = crate::routes::files::service::native::tests::slashed(&dir.join("home"));
        real_driver(
            &rig.link,
            DriverEnv::new(dir.join("none"), vec![("STUDIO_HOME".into(), home)]),
        )
    })
    .await;
    let out = f.dir.join("renders");
    std::fs::create_dir_all(&out).unwrap();
    inject(&f, vec![freeze(&out, "gtr", true)]);
    let impossible = render_all(&f, "/p.k7").await.unwrap();
    assert_eq!(impossible, json!([{ "module": "gtr", "plugins": ["gtr"] }]));
    let names: Vec<_> = std::fs::read_dir(&out)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert!(names.is_empty(), "left behind: {names:?}");
}
