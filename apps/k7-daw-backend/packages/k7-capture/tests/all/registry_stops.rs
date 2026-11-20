//! registry_stops.rs — one account's stop and the start that replaces it run one after the other, per home.
//! What a reader gets wrong: the stop is held in flight by a source whose unpin waits, so the order is by latch, not by sleep.

use std::any::Any;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use k7_capture::Captures;
use k7_capture::account::{BoxFuture, Bundle, Host};
use k7_capture::source::Source;
use k7_file_service::watch::Sink;

use crate::rig::settler;
use crate::support::Scratch;
use crate::worker::timing;

/// A source whose unpin waits for the test, and says when it began and ended.
#[derive(Default)]
struct Slow {
    unpinning: AtomicBool,
    go: AtomicBool,
    log: Mutex<Vec<&'static str>>,
}

impl Source for Slow {
    fn pin(&self, _: &str, _: Sink) -> bool {
        true
    }
    fn unpin(&self, _: &str, _: &Sink) {
        self.unpinning.store(true, Ordering::SeqCst);
        let t = Instant::now();
        while !self.go.load(Ordering::SeqCst) && t.elapsed() < Duration::from_secs(10) {
            std::thread::sleep(Duration::from_millis(2));
        }
        let mut log = self.log.lock().unwrap();
        if log.is_empty() {
            log.push("first stopped");
        }
    }
    fn up(&self, _: &str) -> bool {
        true
    }
}

struct Quiet;

impl Bundle for Quiet {
    fn feed(&self, _: &str, _: Vec<String>, _: u64) {}
    fn migrate_namespaces(&self) -> BoxFuture<Result<(), String>> {
        Box::pin(async { Ok(()) })
    }
    fn let_go_under(&self, _: &str) -> BoxFuture<Result<(), String>> {
        Box::pin(async { Ok(()) })
    }
    fn lock(&self) -> BoxFuture<Box<dyn Any + Send>> {
        Box::pin(async { Box::new(()) as Box<dyn Any + Send> })
    }
    fn folders_changed(&self) {}
}

fn host(home: &str, source: Arc<dyn Source>) -> Host {
    Host {
        home: home.to_string(),
        workspaces: format!("{home}/workspace"),
        library: format!("{home}/library"),
        source,
        k7x: settler(),
        bundle: Arc::new(Quiet),
        timing: timing(false),
    }
}

async fn until(what: &str, ok: impl Fn() -> bool) {
    let t = Instant::now();
    while !ok() {
        assert!(t.elapsed() < Duration::from_secs(5), "never: {what}");
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_start_waits_for_the_stop_of_the_account_it_replaces() {
    let dir = Scratch::new("stops");
    let home = dir.at("home");
    std::fs::create_dir_all(format!("{home}/workspace/band")).unwrap();
    let rt = tokio::runtime::Handle::current();
    let slow = Arc::new(Slow::default());
    let first = Captures::global()
        .start_account(Arc::new(|_| {}), host(&home, slow.clone()), &rt)
        .await;
    until("the workspace is attached", || {
        first
            .attached_at(&format!("{home}/workspace/band"))
            .is_some()
    })
    .await;
    let (h, f) = (home.clone(), first.clone());
    let stop = tokio::spawn(async move { Captures::global().stop_this(&h, &f).await });
    until("the stop is in flight", || {
        slow.unpinning.load(Ordering::SeqCst)
    })
    .await;
    let done = Arc::new(AtomicBool::new(false));
    let (h, d, s, r) = (home.clone(), done.clone(), slow.clone(), rt.clone());
    let start = tokio::spawn(async move {
        let plain = Arc::new(Slow::default());
        plain.go.store(true, Ordering::SeqCst);
        let acct = Captures::global()
            .start_account(Arc::new(|_| {}), host(&h, plain), &r)
            .await;
        s.log.lock().unwrap().push("second started");
        d.store(true, Ordering::SeqCst);
        acct
    });
    tokio::time::sleep(Duration::from_millis(150)).await;
    let early = done.load(Ordering::SeqCst);
    slow.go.store(true, Ordering::SeqCst);
    assert!(
        !early,
        "the second started while the first was still stopping"
    );
    let second = start.await.unwrap();
    stop.await.unwrap();
    assert_eq!(
        *slow.log.lock().unwrap(),
        ["first stopped", "second started"]
    );
    let now = Captures::global().account(&home).expect("registered");
    assert!(Arc::ptr_eq(&now, &second));
    Captures::global().stop_account(&home).await;
}
