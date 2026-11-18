//! account.rs — a workspace's end through the account, and a root found whichever way it is spelled. What a reader gets
//! wrong: an end stops the capture, lets go of its history and removes its log folder; the workspace's files are the host's.

use crate::account_attach::{FILE, V1, WS, rig};
use crate::rig::{get, put};

#[tokio::test(flavor = "multi_thread")]
async fn a_deleted_workspace_stops_its_capture_and_lets_go_of_its_log_and_history() {
    let r = rig("end-ws");
    let root = format!("{}/{WS}", r.workspaces);
    put(&root, FILE, V1);
    let a = r.acct.attach(&root, WS).await.unwrap().unwrap();
    let log = a.log_dir.clone();
    assert!(std::path::Path::new(&log).exists());
    assert!(r.acct.end_workspace(&root).await);
    assert!(r.acct.attached_at(&root).is_none());
    assert!(
        !std::path::Path::new(&log).exists(),
        "the log folder stayed"
    );
    assert_eq!(get(&root, FILE), V1, "the account touched the files");
    // Nothing captures it now: an end finds no capture and says so.
    assert!(!r.acct.end_workspace(&root).await);
    // The same name later starts afresh, with a first entry of its own.
    let again = r.acct.attach(&root, WS).await.unwrap().unwrap();
    assert_eq!(
        r.acct
            .histories()
            .room(&again.log_dir)
            .unwrap()
            .lock()
            .room()
            .head,
        1.0
    );
    r.acct.stop_all().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_root_is_found_whichever_way_the_caller_spells_it() {
    let r = rig("spelling");
    let root = format!("{}/{WS}", r.workspaces);
    put(&root, FILE, V1);
    r.acct.attach(&root, WS).await.unwrap().unwrap();
    for spelled in [
        format!("{root}/"),
        format!("{root}/."),
        format!("{root}/x/.."),
    ] {
        assert!(r.acct.attached_at(&spelled).is_some(), "{spelled}");
    }
    assert!(r.acct.attached_at(&format!("{root}x")).is_none());
    r.acct.stop_all().await;
}
