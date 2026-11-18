//! history_device.rs — a workspace no relay orders keeps its own history on its device (`docs/files/history.md`): a first
//! entry holding its files, an entry per edit, what moved while k7 was down as one more, and none for a relay-synced one.
//! What a reader gets wrong: a restart finds the room it left, so it must not seed a second "starts here".

use std::sync::Arc;
use std::time::Duration;

use k7_capture::account::{Account, Host};

use crate::account_attach::{FILE, Rig, V1, WS, host_of, rig};
use crate::rig::{put, settler};
use crate::support::wait_for;
use crate::worker::Fake;

const V2: &str = "tempo(140)\n";

/// What the device's room holds: its head and the text of one file.
pub(crate) fn room(a: &Account, log_dir: &str, file: &str) -> (f64, Option<String>) {
    let live = a.histories().room(log_dir).unwrap();
    let l = live.lock();
    (l.room().head, l.room().files().get(file).cloned())
}

/// A live edit as an editor's atomic save makes it: a capture never sees the file half written.
pub(crate) fn swap(root: &str, rel: &str, text: &str) {
    let tmp = format!("{root}/../.swap-{}", std::process::id());
    std::fs::write(&tmp, text).unwrap();
    std::fs::rename(tmp, format!("{root}/{rel}")).unwrap();
}

/// A second account over the same folders, as after a restart.
pub(crate) fn again(r: &Rig) -> Arc<Account> {
    let acct = Arc::new(Account::new(Arc::new(|m| eprintln!("{m}"))));
    let lib = format!("{}/library", r.host_home);
    let host = host_of(&r.host_home, &r.workspaces, &lib);
    acct.bind(Host {
        source: Fake::new(),
        k7x: settler(),
        ..host
    });
    acct
}

#[tokio::test(flavor = "multi_thread")]
async fn a_local_workspace_orders_its_files_and_each_edit_in_its_own_room() {
    let r = rig("history-local");
    let root = format!("{}/{WS}", r.workspaces);
    put(&root, FILE, V1);
    let a = r.acct.attach(&root, WS).await.unwrap().unwrap();
    assert_eq!(room(&r.acct, &a.log_dir, FILE), (1.0, Some(V1.into())));
    swap(&root, FILE, V2);
    a.handle.capture(vec![FILE.to_string()]).await;
    wait_for("the edit is ordered", || {
        room(&r.acct, &a.log_dir, FILE).0 == 2.0
    })
    .await;
    assert_eq!(room(&r.acct, &a.log_dir, FILE).1.as_deref(), Some(V2));
    r.acct.stop_all().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn what_moved_while_k7_was_down_is_one_entry_and_the_room_is_not_seeded_again() {
    let r = rig("history-restart");
    let root = format!("{}/{WS}", r.workspaces);
    put(&root, FILE, V1);
    let a = r.acct.attach(&root, WS).await.unwrap().unwrap();
    let log_dir = a.log_dir.clone();
    r.acct.stop_all().await;
    r.acct.histories().release(&log_dir);
    put(&root, FILE, V2);
    let second = again(&r);
    second.attach(&root, WS).await.unwrap().unwrap();
    wait_for("the edit made while down", || {
        room(&second, &log_dir, FILE).0 == 2.0
    })
    .await;
    assert_eq!(room(&second, &log_dir, FILE).1.as_deref(), Some(V2));
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(room(&second, &log_dir, FILE).0, 2.0, "an entry too many");
    second.stop_all().await;
}

use k7_capture::account::Ask;
use serde_json::json;

#[tokio::test(flavor = "multi_thread")]
async fn the_page_a_checkpoint_a_diff_and_a_restore_answer_from_the_device() {
    let r = rig("history-asks");
    let root = format!("{}/{WS}", r.workspaces);
    put(&root, FILE, V1);
    let a = r.acct.attach(&root, WS).await.unwrap().unwrap();
    swap(&root, FILE, V2);
    a.handle.capture(vec![FILE.to_string()]).await;
    wait_for("the edit", || room(&r.acct, &a.log_dir, FILE).0 == 2.0).await;
    let page = Ask::Page {
        before: None,
        limit: None,
    };
    let page = r.acct.history_ask(&root, page).await.unwrap();
    assert_eq!(page["source"].as_str(), Some("device"));
    assert_eq!(page["head"], 2.0);
    assert_eq!(page["entries"][0]["seq"], 2.0);
    let named = Ask::Checkpoint {
        body: json!({ "name": "tempo 120", "seq": 1 }),
        by: "ann".into(),
    };
    let cp = r.acct.history_ask(&root, named).await.unwrap();
    assert_eq!(
        (cp["seq"].as_f64(), cp["by"].as_str()),
        (Some(1.0), Some("ann"))
    );
    let diff = r
        .acct
        .history_ask(
            &root,
            Ask::Diff {
                from: Some(json!(1)),
                to: Some(json!(2)),
            },
        )
        .await
        .unwrap();
    assert_eq!(diff["files"][0]["before"], V1);
    assert_eq!(diff["files"][0]["after"], V2);
    // The restore writes the old text back and is one entry; the capture does not take it for a second edit.
    let back = r
        .acct
        .history_ask(
            &root,
            Ask::Restore {
                body: json!({ "seq": 1 }),
            },
        )
        .await
        .unwrap();
    assert_eq!(back["entry"]["seq"], 3.0);
    assert_eq!(
        std::fs::read_to_string(format!("{root}/{FILE}")).unwrap(),
        V1
    );
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(room(&r.acct, &a.log_dir, FILE), (3.0, Some(V1.into())));
    r.acct.stop_all().await;
}
