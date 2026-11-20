//! history_landing.rs — files a git pull landed through the capture are one entry of the workspace's history, named by the pull.
//! What a reader gets wrong: the landing is learned by the ledger, so only this call writes the entry; a watcher never would.

use k7_capture::account::Ask;

use crate::account_attach::{FILE, V1, WS, rig};
use crate::history_device::{room, swap};
use crate::rig::put;

const V2: &str = "tempo(140)\n";

#[tokio::test(flavor = "multi_thread")]
async fn a_pulls_landing_is_one_history_entry_named_by_the_pull() {
    let r = rig("history-landing");
    let root = format!("{}/{WS}", r.workspaces);
    put(&root, FILE, V1);
    let a = r.acct.attach(&root, WS).await.unwrap().unwrap();
    // The worker's first full scan runs after attach returns; a mailbox call waits for it.
    a.handle.run(|_| ()).await;
    swap(&root, FILE, V2);
    put(&root, "new.md", "from the remote\n");
    r.acct.record_landing(&root, "pull abc1234").await;
    assert_eq!(room(&r.acct, &a.log_dir, FILE), (2.0, Some(V2.into())));
    let page = Ask::Page {
        before: None,
        limit: None,
    };
    let page = r.acct.history_ask(&root, page).await.unwrap();
    let last = &page["entries"][0];
    assert_eq!(last["label"], "pull abc1234", "{page}");
    assert_eq!(last["files"].as_array().map(Vec::len), Some(2), "{page}");
    r.acct.record_landing(&root, "pull again").await;
    assert_eq!(
        room(&r.acct, &a.log_dir, FILE).0,
        2.0,
        "nothing moved, no entry"
    );
    r.acct.stop_all().await;
}
