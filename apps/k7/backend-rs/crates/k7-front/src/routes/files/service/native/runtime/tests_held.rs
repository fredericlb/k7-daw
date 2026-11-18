//! The account's pieces through `Services::held` (2606, 2640, 2671): runtime off builds none, the capture's feed reaches the
//! module watch, and the stop ends the prefetch and the link. What a reader gets wrong: the account is the real one here.

use crate::routes::files::service::native::tests::slashed;
use std::time::{Duration, Instant};

use serde_json::json;

use super::tests_stop::{Rig, rig_native, rig_with};

const ALL: &str = "reads,entries,revision,journal,listings,workspaces,projects,capture,engine,engine-jobs,media,shares,docs";

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_accounts_stop_ends_its_prefetch_and_its_link() {
    let rig = rig_native("held-stop-lines", &format!("{ALL},runtime"));
    let Rig {
        dir,
        svc,
        instances,
        ..
    } = &rig;
    let h = svc.held("a@x.org", instances).await.unwrap();
    let (m, e) = (h.media.clone().unwrap(), h.engine.as_ref().unwrap());
    assert!(m.prefetch.started(), "the prefetch never started");
    let _ = e.link.session_named("s");
    assert!(!e.link.sessions().is_empty());
    h.stop(None, Some(Duration::from_secs(30))).await;
    assert!(!m.prefetch.started(), "the stop left the prefetch running");
    assert!(e.link.sessions().is_empty(), "the stop left the link open");
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_captures_feed_moves_the_module_watch_with_no_watcher_of_its_own() {
    let rig = rig_with(
        "held-feed",
        &format!("{ALL},runtime"),
        &[("K7_NO_FS_WATCH", "1")],
    );
    let Rig {
        dir,
        svc,
        instances,
        ..
    } = &rig;
    let env = instances.environment("a@x.org", "s").await.unwrap().env;
    let studio = k7_file_service::libraries::Home::of(&env, false).studio;
    let root = std::path::Path::new(&studio).join("workspace/songs");
    let project = root.join("p.k7");
    std::fs::create_dir_all(project.join("nodes")).unwrap();
    std::fs::write(project.join("project.k7x"), "project({})\n").unwrap();
    let h = svc.held("a@x.org", instances).await.unwrap();
    let rt = h.runtime.clone().expect("runtime answers in this rig");
    assert!(h.native_capture, "the rig's capture is not native");
    let args = json!({ "dir": slashed(&project), "roots": [slashed(&project.join("nodes"))], "feed": slashed(&root) });
    rt.ask("watch.join", &args).await.unwrap().unwrap();
    let p = slashed(&project);
    assert_eq!(rt.watch.joined(), 1);
    // The attach is an order the capture takes later; a write before it is in the baseline, not a change.
    let acct = k7_capture::Captures::global().account(&studio).unwrap();
    let root_s = slashed(&root);
    let at = Instant::now();
    while acct.attached_at(&root_s).is_none() {
        assert!(
            at.elapsed() < Duration::from_secs(6),
            "the workspace never attached"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(!rt.watch.still(&p), "the first look walks");
    assert!(rt.watch.still(&p), "nothing moved");
    std::fs::write(project.join("nodes/n.k7x"), "node({})\n").unwrap();
    // No watcher exists (K7_NO_FS_WATCH): only the feed can say it, well before the walk-anyway ceiling.
    let at = Instant::now();
    while rt.watch.still(&p) {
        assert!(
            at.elapsed() < Duration::from_secs(6),
            "the capture's feed never reached the watch"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    svc.stop().await;
    let _ = std::fs::remove_dir_all(dir);
}
