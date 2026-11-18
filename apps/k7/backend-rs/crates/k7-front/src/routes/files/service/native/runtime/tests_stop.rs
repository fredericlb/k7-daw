//! The awaited stop (2670 to 2673): members in order, a hung one named and left, every account's members awaited
//! before the process leaves, and no account made once the stop began.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use k7_hub::Log;
use k7_hub::account::Names;

use super::stop::{Member, drive};
use crate::routes::files::service::services::Services;

fn member(order: &Arc<Mutex<Vec<String>>>, name: &'static str, hang: bool) -> (String, Member) {
    let order = order.clone();
    (
        name.to_string(),
        Box::new(move || {
            Box::pin(async move {
                if hang {
                    std::future::pending::<()>().await;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
                order.lock().unwrap().push(name.to_string());
            })
        }),
    )
}

#[tokio::test]
async fn members_stop_in_order_one_after_the_other() {
    let order = Arc::new(Mutex::new(Vec::new()));
    let members = vec![
        member(&order, "jobs", false),
        member(&order, "captures", false),
        member(&order, "media", false),
    ];
    let plan = drive(members, None, Some(Duration::from_secs(30))).await;
    assert_eq!(*order.lock().unwrap(), ["jobs", "captures", "media"]);
    assert!(plan.left().is_empty());
}

#[tokio::test]
async fn a_member_that_never_stops_is_named_and_the_whole_ends_at_its_bound() {
    let order = Arc::new(Mutex::new(Vec::new()));
    let members = vec![member(&order, "jobs", true), member(&order, "media", false)];
    let t = Instant::now();
    let plan = drive(members, None, Some(Duration::from_millis(400))).await;
    assert!(t.elapsed() >= Duration::from_millis(400) && t.elapsed() < Duration::from_secs(5));
    assert_eq!(plan.left(), ["jobs", "media"]);
    assert!(order.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_member_bound_lets_the_next_one_run() {
    let order = Arc::new(Mutex::new(Vec::new()));
    let members = vec![member(&order, "jobs", true), member(&order, "media", false)];
    let plan = drive(
        members,
        Some(Duration::from_millis(200)),
        Some(Duration::from_secs(30)),
    )
    .await;
    assert_eq!(plan.left(), ["jobs"]);
    assert_eq!(*order.lock().unwrap(), ["media"]);
}

/// A `Services` over scratch homes, and the instances that make its accounts.
pub(super) struct Rig {
    pub dir: std::path::PathBuf,
    pub svc: Arc<Services>,
    pub instances: Arc<Names>,
    pub log: Log,
}

pub(super) fn rig(name: &str) -> Rig {
    rig_native(name, "reads")
}

/// `rig` with `native` naming the pieces on: `capture` or `all` makes the account's capture native.
pub(super) fn rig_native(name: &str, native: &str) -> Rig {
    rig_with(name, native, &[])
}

/// `rig_native` with `extra` in the accounts' base environment.
pub(super) fn rig_with(name: &str, native: &str, extra: &[(&str, &str)]) -> Rig {
    use std::collections::BTreeMap;

    let dir = crate::routes::files::service::native::tests::tmp_root()
        .join(format!("k7-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("demos/synth.k7")).unwrap();
    std::fs::write(dir.join("demos/synth.k7/project.k7x"), "project({})\n").unwrap();
    let data = dir.join("data");
    let mut o = k7_hub::account::Names::new(Arc::new(move |u: &str| {
        let slug = u.replace('@', "-");
        (slug.clone(), data.join(slug))
    }));
    // The host's own additions to an account's environment, which the front reads as the hub's overlay.
    let mut added = BTreeMap::from([(
        "STUDIO_DEMOS".to_string(),
        dir.join("demos").display().to_string(),
    )]);
    for (k, v) in extra {
        added.insert((*k).into(), (*v).into());
    }
    if native.contains("capture") || native == "all" {
        added.insert(
            crate::routes::files::service::services::FRONT_CAPTURE.into(),
            "1".into(),
        );
    }
    o.env = Arc::new(move |_, _| {
        let added = added.clone();
        Box::pin(async move { added })
    });
    let instances = Arc::new(o);
    let env = BTreeMap::new();
    let log: Log = Arc::new(|_: &str| {});
    let svc = Arc::new(Services::new(&env, log.clone()));
    Rig {
        dir,
        svc,
        instances,
        log,
    }
}

/// The process' stop through `Services`: every held account's members awaited, none made after, a hung member named.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_services_stop_awaits_every_account_and_makes_none_afterwards() {
    let Rig {
        dir,
        svc,
        instances,
        ..
    } = rig("stop");
    let a = svc.held("a@x.org", &instances).await.unwrap();
    let b = svc.held("b@x.org", &instances).await.unwrap();
    let ran = Arc::new(AtomicBool::new(false));
    let seen = ran.clone();
    b.members.add(
        "shares",
        Box::new(move || Box::pin(async move { seen.store(true, Ordering::SeqCst) })),
    );
    a.members
        .add("jobs", Box::new(|| Box::pin(std::future::pending())));
    let line = super::stop::stop_all(vec![b.clone()], Duration::from_secs(30)).await;
    assert!(line.is_none() && ran.load(Ordering::SeqCst), "{line:?}");
    // A's hung member is named at the bound, a shorter one than the process' five seconds.
    let t = Instant::now();
    let line = super::stop::stop_all(vec![a.clone()], Duration::from_millis(500)).await;
    assert!(t.elapsed() < Duration::from_secs(10));
    let line = line.expect("a hung member is named");
    assert!(
        line.contains("jobs") && line.contains(&a.account.slug),
        "{line}"
    );
    // The process' stop refuses to make an account once it began.
    svc.stop().await;
    let after = svc.held("c@x.org", &instances).await;
    assert!(after.err().is_some_and(|e| e == "stopping"));
    let _ = std::fs::remove_dir_all(dir);
}

/// F3: two accounts whose members meet at a barrier stop only if they stop at the same time.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_accounts_stop_in_parallel_not_one_after_the_other() {
    let Rig {
        dir,
        svc,
        instances,
        ..
    } = rig("stop-par");
    let a = svc.held("a@x.org", &instances).await.unwrap();
    let b = svc.held("b@x.org", &instances).await.unwrap();
    let barrier = Arc::new(tokio::sync::Barrier::new(2));
    let met = Arc::new(AtomicUsize::new(0));
    for h in [&a, &b] {
        let (barrier, met) = (barrier.clone(), met.clone());
        h.members.add(
            "jobs",
            Box::new(move || {
                Box::pin(async move {
                    barrier.wait().await;
                    met.fetch_add(1, Ordering::SeqCst);
                })
            }),
        );
    }
    let line = super::stop::stop_all(vec![a, b], Duration::from_secs(2)).await;
    assert!(line.is_none(), "{line:?}");
    assert_eq!(met.load(Ordering::SeqCst), 2);
    let _ = std::fs::remove_dir_all(dir);
}

/// F3 and the user's 1.5 s per member: a hung first member leaves the captures' save and the members after it their turn.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_hung_member_never_starves_the_members_after_it() {
    let Rig {
        dir,
        svc,
        instances,
        log,
    } = rig("stop-each");
    let a = svc.held("a@x.org", &instances).await.unwrap();
    let saved = Arc::new(AtomicBool::new(false));
    let seen = saved.clone();
    a.members
        .add("jobs", Box::new(|| Box::pin(std::future::pending())));
    a.members.add(
        "captures",
        Box::new(move || Box::pin(async move { seen.store(true, Ordering::SeqCst) })),
    );
    let before = Arc::strong_count(&log);
    let t = Instant::now();
    let line = super::stop::stop_all(vec![a.clone()], Duration::from_secs(5))
        .await
        .expect("the hung member is named");
    assert!(t.elapsed() < Duration::from_secs(4), "{:?}", t.elapsed());
    assert!(saved.load(Ordering::SeqCst), "captures never ran: {line}");
    assert!(
        line.contains("jobs") && !line.contains("captures"),
        "{line}"
    );
    assert!(
        Arc::strong_count(&log) < before,
        "the account was not let go"
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// F2: a `held` parked behind `making` when the stop begins makes no account, its own stop run whole.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_account_made_while_the_stop_runs_is_refused_and_nothing_of_it_stays_alive() {
    let Rig {
        dir,
        svc,
        instances,
        log,
    } = rig("stop-race");
    // The process-wide report line holds a clone of the first log it is given: start it before the count.
    super::report::start_once(&log, super::report::REPORT_EVERY);
    let before = Arc::strong_count(&log);
    let one = svc.making.lock().await;
    let (s2, i2) = (svc.clone(), instances.clone());
    let parked = tokio::spawn(async move { s2.held("a@x.org", &i2).await.map(|_| ()) });
    tokio::time::sleep(Duration::from_millis(300)).await;
    svc.stop().await;
    drop(one);
    let made = parked.await.unwrap();
    assert_eq!(made.err().as_deref(), Some("stopping"));
    assert!(svc.get("a@x.org").is_none());
    // The account's tasks end after the stop returns: the count settles within a moment.
    let end = Instant::now() + Duration::from_secs(5);
    while Arc::strong_count(&log) != before && Instant::now() < end {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(Arc::strong_count(&log), before, "an account was left alive");
    let _ = std::fs::remove_dir_all(dir);
}

/// F4: an account the reap let go a moment before the quit is still stopped, and awaited, by the process' stop.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_process_stop_awaits_the_stops_the_reap_spawned() {
    let Rig {
        dir,
        svc,
        instances,
        log,
    } = rig("stop-reaped");
    let a = svc.held("a@x.org", &instances).await.unwrap();
    let saved = Arc::new(AtomicBool::new(false));
    let seen = saved.clone();
    a.members.add(
        "captures",
        Box::new(move || {
            Box::pin(async move {
                tokio::time::sleep(Duration::from_millis(400)).await;
                seen.store(true, Ordering::SeqCst);
            })
        }),
    );
    drop(a);
    let before = Arc::strong_count(&log);
    svc.reap(Duration::ZERO, |_| false);
    assert!(svc.get("a@x.org").is_none());
    svc.stop().await;
    assert!(
        saved.load(Ordering::SeqCst),
        "the stop did not wait for the reaped account"
    );
    assert!(
        Arc::strong_count(&log) < before,
        "the account was not let go"
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// The stop recheck ends the captures the account's start began, and saves their ledger.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_account_refused_by_the_stop_leaves_no_native_capture_running() {
    let rig = rig_native("stop-capture", "all");
    let Rig {
        dir,
        svc,
        instances,
        ..
    } = &rig;
    let probe = svc.held("probe@x.org", instances).await.unwrap();
    assert!(probe.native_capture, "the rig's capture is not native");
    let studio = k7_file_service::libraries::Home::of(&probe.account.env, false).studio;
    assert!(k7_capture::Captures::global().account(&studio).is_some());
    drop(probe);
    let one = svc.making.lock().await;
    let (s2, i2) = (svc.clone(), instances.clone());
    let parked = tokio::spawn(async move { s2.held("a@x.org", &i2).await.map(|_| ()) });
    tokio::time::sleep(Duration::from_millis(300)).await;
    svc.stop().await;
    drop(one);
    assert_eq!(parked.await.unwrap().err().as_deref(), Some("stopping"));
    let studio = k7_file_service::libraries::Home::of(
        &instances.environment("a@x.org", "s").await.unwrap().env,
        false,
    )
    .studio;
    let left = k7_capture::Captures::global().account(&studio);
    assert!(left.is_none(), "the captures outlived the stop");
    let _ = std::fs::remove_dir_all(dir);
}

/// 2671: the captures are a member of the account's stop, awaited before it returns, their account gone.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_accounts_stop_awaits_its_captures_ledgers() {
    let rig = rig_native("stop-ledgers", "all");
    let Rig {
        dir,
        svc,
        instances,
        ..
    } = &rig;
    let h = svc.held("a@x.org", instances).await.unwrap();
    assert!(h.native_capture, "the rig's capture is not native");
    let studio = k7_file_service::libraries::Home::of(&h.account.env, false).studio;
    assert!(k7_capture::Captures::global().account(&studio).is_some());
    let line = super::stop::stop_all(vec![h.clone()], Duration::from_secs(30)).await;
    assert!(line.is_none(), "{line:?}");
    assert!(
        k7_capture::Captures::global().account(&studio).is_none(),
        "the stop returned before the captures ended"
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// A background job's lease holds the reap: an idle account with a running job stays, and goes when it ends.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_running_background_job_holds_the_reap() {
    let rig = rig_native(
        "reap-job",
        "reads,entries,revision,journal,listings,workspaces,projects,capture,engine,engine-jobs,media,shares,docs,runtime",
    );
    let Rig {
        dir,
        svc,
        instances,
        ..
    } = &rig;
    let h = svc.held("a@x.org", instances).await.unwrap();
    let rt = h.runtime.clone().expect("runtime answers in this rig");
    assert!(rt.bg_start("ffmpeg /m/a b.mp4"));
    drop(h);
    svc.reap(Duration::ZERO, |_| false);
    assert!(
        svc.get("a@x.org").is_some(),
        "a running job did not hold the account"
    );
    rt.dropped();
    // A call of the account's own may be in flight for a moment: the reap is tried until it lets go.
    for _ in 0..60 {
        svc.reap(Duration::ZERO, |_| false);
        if svc.get("a@x.org").is_none() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(
        svc.get("a@x.org").is_none(),
        "the account stayed after its job ended"
    );
    svc.stop().await;
    let _ = std::fs::remove_dir_all(dir);
}
