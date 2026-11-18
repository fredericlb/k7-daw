//! account_folders.rs — what the account does to a workspace's folder: a rename that moves its log, its history and its
//! marker (2153, 2154). What a reader gets wrong: the capture goes on under the new name and the move is no edit.

use std::time::Duration;

use k7_capture::disk::log_dir_of;

use crate::account_attach::{FILE, V1, WS, host_of, rig};
use crate::rig::{get, put};
use crate::support::wait_for;

#[tokio::test(flavor = "multi_thread")]
async fn a_rename_moves_the_folder_its_history_and_marker_and_the_capture_goes_on_under_the_new_name()
 {
    let r = rig("attach-rename");
    let root = format!("{}/{WS}", r.workspaces);
    put(&root, FILE, V1);
    let a = r.acct.attach(&root, WS).await.unwrap().unwrap();
    wait_for("the history's first entry", || {
        r.acct
            .histories()
            .room(&a.log_dir)
            .is_ok_and(|l| l.lock().room().head >= 1.0)
    })
    .await;
    let old_log = a.log_dir.clone();
    let marker = format!("{}/.active-projects/{WS}", r.workspaces);
    put(&format!("{}/.active-projects", r.workspaces), WS, "song.k7");
    let batches = std::fs::read_to_string(format!("{old_log}/batches.jsonl")).unwrap_or_default();
    r.acct.rename(WS, "renamed").await.unwrap();
    let new_root = format!("{}/renamed", r.workspaces);
    assert!(!std::path::Path::new(&root).exists());
    assert_eq!(get(&new_root, FILE), V1);
    let new_log = log_dir_of(&r.host_home, &new_root);
    assert!(std::path::Path::new(&new_log).exists() && !std::path::Path::new(&old_log).exists());
    assert!(!std::path::Path::new(&marker).exists());
    assert!(std::path::Path::new(&format!("{}/.active-projects/renamed", r.workspaces)).exists());
    assert_eq!(
        r.asks.let_go.lock().unwrap().as_slice(),
        std::slice::from_ref(&root)
    );
    assert!(r.acct.attached_at(&root).is_none());
    assert!(
        r.acct.attached_at(&new_root).is_some(),
        "attached under the new name"
    );
    // The move is no edit: nothing was captured for it.
    tokio::time::sleep(Duration::from_millis(400)).await;
    let after = std::fs::read_to_string(format!("{new_log}/batches.jsonl")).unwrap_or_default();
    assert_eq!(after, batches, "the move was captured as edits");
    r.acct.stop_all().await;
}

#[test]
fn a_windows_home_spells_its_roots_markers_and_log_folders_with_forward_slashes() {
    let home = "C:/Users/x/home";
    let host = host_of(
        home,
        &format!("{home}/workspace"),
        &format!("{home}/library"),
    );
    let root = host.root_of("band");
    assert_eq!(root, "C:/Users/x/home/workspace/band");
    assert_eq!(
        host.marker_of(&root),
        "C:/Users/x/home/workspace/.active-projects/band"
    );
    // The log folder's hash is of the backslash spelling, as `liveLogFor` takes it.
    assert_eq!(
        log_dir_of(home, &root),
        "C:/Users/x/home/.sync/band-5efee6b47349"
    );
}
