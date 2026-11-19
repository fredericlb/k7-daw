//! tests_capture.rs — a real account of captures under the front: the window a native write takes, a removal that ends
//! the capture, and a stop that never reaches the account that replaced it. Also the host the other tests start from.

use std::any::Any;
use std::sync::Arc;

use k7_capture::account::{BoxFuture, Bundle, Host};
use k7_capture::worker::Timing;
use k7_capture::{Captures, native, source::Process};

use crate::routes::files::service::Window;
use serde_json::json;

pub(super) struct Quiet;

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

pub(super) fn host(home: &str) -> Host {
    Host {
        home: home.to_string(),
        workspaces: format!("{home}/workspace"),
        library: format!("{home}/library"),
        source: Arc::new(Process),
        k7x: native(),
        bundle: Arc::new(Quiet),
        timing: Timing::default(),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_native_write_takes_the_captures_window_without_the_actor() {
    let dir = crate::routes::files::service::native::tests::tmp_root()
        .join(format!("k7-front-window-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let home = crate::routes::files::service::native::tests::slashed(&dir.join("home"));
    let root = format!("{home}/workspace/band");
    std::fs::create_dir_all(&root).unwrap();
    let rt = tokio::runtime::Handle::current();
    let acct = Captures::global()
        .start_account(Arc::new(|_| {}), host(&home), &rt)
        .await;
    acct.attach(&root, "band").await.unwrap().unwrap();
    let w = Window::open(&root, "edit a").await;
    assert!(w.is_native());
    w.close().await;
    let stranger = Window::open(&format!("{home}/workspace/other"), "").await;
    assert!(!stranger.is_native());
    stranger.close().await;
    Captures::global().stop_account(&home).await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_removal_ends_the_capture_in_the_registrys_account() {
    use super::tests::scratch;
    use super::tests_workspaces::{go, ws_acct};
    use super::workspaces::Kind;
    let dir = scratch("w-end");
    let (a, _) = ws_acct(&dir, "w-end");
    let home = crate::routes::files::service::native::tests::slashed(&dir.join("home"));
    let root = format!("{home}/workspace/doomed");
    std::fs::create_dir_all(format!("{root}/p.k7")).unwrap();
    std::fs::write(format!("{root}/a.md"), "a").unwrap();
    let rt = tokio::runtime::Handle::current();
    let held = Captures::global()
        .start_account(Arc::new(|_| {}), host(&home), &rt)
        .await;
    held.attach(&root, "doomed").await.unwrap().unwrap();
    let (status, _) = go(&a, Kind::Remove, json!({ "name": "doomed" })).await;
    assert_eq!(status, 200);
    assert!(held.attached_at(&root).is_none(), "ended and forgotten");
    Captures::global().stop_account(&home).await;
    std::fs::remove_dir_all(dir).unwrap();
}
