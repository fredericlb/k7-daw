//! tests_bundle.rs — the native `Bundle` the captures call back through: a folder renamed under the account's lock.
//! What a reader gets wrong: the rename waits on the SAME lock the front's writers take, keyed by the studio home.

use std::sync::Arc;
use std::time::Duration;

use k7_capture::account::{Account, Host};
use k7_capture::worker::Timing;
use k7_capture::{native, source::Process};

use super::capture_bundle::FrontBundle;
use super::locks;
use super::tests::{scratch, slashed};

fn host(home: &str, bundle: FrontBundle) -> Host {
    Host {
        home: home.to_string(),
        workspaces: format!("{home}/workspace"),
        library: format!("{home}/library"),
        source: Arc::new(Process),
        k7x: native(),
        bundle: Arc::new(bundle),
        timing: Timing::default(),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_folder_renamed_by_the_capture_waits_for_the_front_s_write_lock() {
    let dir = scratch("bundle-lock");
    let home = slashed(&dir.join("home"));
    let (from, to) = (
        format!("{home}/workspace/band"),
        format!("{home}/workspace/band2"),
    );
    std::fs::create_dir_all(&from).unwrap();
    std::fs::write(format!("{from}/a.md"), "a").unwrap();
    let acct = Arc::new(Account::new(Arc::new(|_| {})));
    acct.bind(host(&home, FrontBundle::new(&home, "acct", None, None)));
    let held = locks::write(&home).await;
    let renaming = {
        let acct = acct.clone();
        tokio::spawn(async move { acct.rename("band", "band2").await })
    };
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!renaming.is_finished(), "the rename ran under a held lock");
    assert!(std::path::Path::new(&from).exists() && !std::path::Path::new(&to).exists());
    drop(held);
    renaming.await.unwrap().unwrap();
    assert!(std::path::Path::new(&to).exists() && !std::path::Path::new(&from).exists());
    std::fs::remove_dir_all(dir).unwrap();
}
