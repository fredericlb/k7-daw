//! account_attach.rs — the account starting captures: attach, attach by name, the start's sweep and the feed (2102, 2150).
//! What a reader gets wrong: no workspace joins a relay, so an attach writes no link record and reads none.

use std::any::Any;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use k7_capture::account::{Account, BoxFuture, Bundle, Host};
use k7_capture::disk::log_dir_of;

use crate::rig::{put, settler};
use crate::support::Scratch;
use crate::support::wait_for;
use crate::worker::{Fake, timing};

pub(crate) const WS: &str = "band";
pub(crate) const FILE: &str = "song.k7/song.k7x";
pub(crate) const V1: &str = "tempo(120)\n";

/// What the host was asked, in order.
#[derive(Default)]
pub(crate) struct Asks {
    pub(crate) let_go: Mutex<Vec<String>>,
    folders: AtomicUsize,
    fed: Mutex<Vec<String>>,
}

impl Bundle for Asks {
    fn feed(&self, _: &str, paths: Vec<String>, _: u64) {
        self.fed.lock().unwrap().extend(paths);
    }

    fn migrate_namespaces(&self) -> BoxFuture<Result<(), String>> {
        Box::pin(async { Ok(()) })
    }

    fn let_go_under(&self, root: &str) -> BoxFuture<Result<(), String>> {
        self.let_go.lock().unwrap().push(root.to_string());
        Box::pin(async { Ok(()) })
    }

    fn lock(&self) -> BoxFuture<Box<dyn Any + Send>> {
        Box::pin(async { Box::new(()) as Box<dyn Any + Send> })
    }

    fn folders_changed(&self) {
        self.folders.fetch_add(1, Ordering::SeqCst);
    }
}

pub(crate) struct Rig {
    pub(crate) acct: Arc<Account>,
    pub(crate) fake: Arc<Fake>,
    pub(crate) asks: Arc<Asks>,
    pub(crate) host_home: String,
    pub(crate) workspaces: String,
    _dir: Scratch,
}

/// A host over the given folders, with a host part that answers nothing; a caller swaps the parts it watches.
pub(crate) fn host_of(home: &str, workspaces: &str, library: &str) -> Host {
    Host {
        home: home.into(),
        workspaces: workspaces.into(),
        library: library.into(),
        source: Fake::new(),
        k7x: settler(),
        bundle: Arc::new(Asks::default()),
        timing: timing(true),
    }
}

pub(crate) fn rig(name: &str) -> Rig {
    rig_with(name, None)
}

/// A rig whose history rooms roll a segment and a snapshot at the given entry counts.
pub(crate) fn rig_with(name: &str, cadence: Option<(f64, f64)>) -> Rig {
    let dir = Scratch::new(name);
    let (home, workspaces) = (dir.at("home"), dir.at("home/workspace"));
    std::fs::create_dir_all(&workspaces).unwrap();
    let mut acct = Account::new(Arc::new(|m| eprintln!("{m}")));
    if let Some((roll, snap)) = cadence {
        acct = acct.with_cadence(roll, snap);
    }
    let acct = Arc::new(acct);
    let asks = Arc::new(Asks::default());
    let fake = Fake::new();
    let host = host_of(&home, &workspaces, &dir.at("home/library"));
    acct.bind(Host {
        source: fake.clone(),
        bundle: asks.clone(),
        ..host
    });
    Rig {
        acct,
        fake,
        asks,
        host_home: home,
        workspaces,
        _dir: dir,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn an_attach_captures_a_workspace_once_and_never_the_examples() {
    let r = rig("attach-once");
    let root = format!("{}/{WS}", r.workspaces);
    put(&root, FILE, V1);
    let a = r.acct.attach(&root, WS).await.unwrap().unwrap();
    let again = r.acct.attach(&root, WS).await.unwrap().unwrap();
    assert!(
        Arc::ptr_eq(&a, &again),
        "a second attach made a second capture"
    );
    let examples = format!("{}/Examples", r.workspaces);
    assert!(
        r.acct
            .attach(&examples, "Examples")
            .await
            .unwrap()
            .is_none()
    );
    r.acct.stop_all().await;
}

// 3146: a device with no server leaves no `.sync/` link record, whatever the edits.
#[tokio::test(flavor = "multi_thread")]
async fn a_workspace_edited_on_a_device_with_no_server_leaves_no_link_record() {
    let r = rig("attach-no-record");
    let root = format!("{}/{WS}", r.workspaces);
    put(&root, FILE, V1);
    let a = r.acct.attach(&root, WS).await.unwrap().unwrap();
    put(&root, "song.k7/b.md", "b");
    a.handle.capture(vec!["song.k7/b.md".to_string()]).await;
    a.handle.flush().await;
    let log = a.log.tail(10.0).unwrap();
    assert!(!log.is_empty(), "the capture wrote no batch");
    let mut left: Vec<String> = std::fs::read_dir(&a.log_dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    left.sort();
    assert!(
        !left.iter().any(|n| n == "server.json" || n == "local-only"),
        "a link record or a mark was written: {left:?}"
    );
    r.acct.stop_all().await;
}

// A record an older k7 left is neither read nor written: it cannot stop an attach, whatever it holds.
#[tokio::test(flavor = "multi_thread")]
async fn an_old_link_record_is_left_as_it_is_and_never_stops_an_attach() {
    let r = rig("attach-old-record");
    let root = format!("{}/{WS}", r.workspaces);
    put(&root, FILE, V1);
    let log_dir = log_dir_of(&r.host_home, &root);
    std::fs::create_dir_all(&log_dir).unwrap();
    let old = r#"{"server":"https://gone.example","link":"gone","room":"band"}"#;
    std::fs::write(format!("{log_dir}/server.json"), old).unwrap();
    let a = r.acct.attach(&root, WS).await.unwrap().unwrap();
    put(&root, "song.k7/b.md", "b");
    a.handle.flush().await;
    assert_eq!(
        std::fs::read_to_string(format!("{log_dir}/server.json")).unwrap(),
        old
    );
    // A folder where the record was is no reason to refuse either.
    r.acct.detach(&root).await;
    std::fs::remove_file(format!("{log_dir}/server.json")).unwrap();
    std::fs::create_dir_all(format!("{log_dir}/server.json")).unwrap();
    assert!(r.acct.attach(&root, WS).await.is_ok());
    r.acct.stop_all().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_workspace_attached_by_name_is_captured_and_a_name_with_no_folder_makes_none() {
    let r = rig("attach-named");
    let root = format!("{}/Banc", r.workspaces);
    put(&root, "live.k7/live.k7x", V1);
    r.acct.attach_named("Banc").await;
    assert!(r.acct.attached_at(&root).is_some(), "the bench is captured");
    r.acct.attach_named("fresh").await;
    let fresh = format!("{}/fresh", r.workspaces);
    assert!(!std::path::Path::new(&fresh).exists());
    assert!(r.acct.attached_at(&fresh).is_none());
    assert_eq!(r.asks.folders.load(Ordering::SeqCst), 0);
    r.acct.stop_all().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_start_attaches_every_folder_and_leaves_a_dot_named_one_as_it_is() {
    let r = rig("attach-sweep");
    let left = format!("{}/.k7-fork-{WS} - mac", r.workspaces);
    put(&left, FILE, V1);
    let kept = format!("{}/{WS}", r.workspaces);
    put(&kept, FILE, V1);
    r.acct.attach_all().await;
    assert!(
        std::path::Path::new(&left).is_dir(),
        "the start removed a folder"
    );
    assert!(
        r.acct.attached_at(&left).is_none(),
        "a dot-named folder was captured"
    );
    assert!(std::path::Path::new(&kept).is_dir());
    assert!(
        r.acct.attached_at(&kept).is_some(),
        "the start attaches all"
    );
    r.acct.stop_all().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_paths_a_watcher_heard_reach_the_bundle_as_one_feed() {
    let r = rig("attach-feed");
    let root = format!("{}/{WS}", r.workspaces);
    put(&root, FILE, V1);
    r.acct.attach(&root, WS).await.unwrap().unwrap();
    put(&root, "notes.md", "heard\n");
    r.fake.report(&["notes.md"]);
    wait_for("the feed", || !r.asks.fed.lock().unwrap().is_empty()).await;
    // The watcher's report and the capture's own publish after its batch are two posts, as in the TypeScript: never a third.
    tokio::time::sleep(Duration::from_millis(300)).await;
    let fed = r.asks.fed.lock().unwrap().clone();
    assert!(
        fed.len() <= 2 && fed.iter().all(|p| p == "notes.md"),
        "{fed:?}"
    );
    r.acct.stop_all().await;
}
