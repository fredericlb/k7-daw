//! tests_git_history.rs — what git leaves in the local history of a captured workspace: a commit names a checkpoint with its
//! subject and a pull's landing is one entry `pull <short id>`.

use k7_capture::account::Ask;
use serde_json::json;

use super::git::Kind;
use super::tests_device::device as capture_device;
use super::tests_git::{bare, device, post, url, who};

#[tokio::test(flavor = "multi_thread")]
async fn a_commit_names_a_checkpoint_and_a_pull_is_one_history_entry() {
    let remote_dev = device("hist-remote");
    let remote = remote_dev.dir.join("bare.git");
    bare(&remote);
    let (p, mut a) = (device("hist-p"), device("hist-a"));
    let captured = capture_device("hist").await;
    a.ctx.account = Some(captured.acct.clone());
    a.ctx.workspaces = format!("{}/workspace", captured.home.studio);
    who(&p).await;
    who(&a).await;
    let theirs = p.dir.join("workspace/band");
    std::fs::create_dir_all(&theirs).unwrap();
    std::fs::write(theirs.join("n.md"), "one\n").unwrap();
    let body = json!({ "workspace": "band", "url": url(&remote) });
    post(&p, Kind::Connect, "/git/connect", body).await;
    let m = json!({ "workspace": "band", "message": "first" });
    post(&p, Kind::Commit, "/git/commit", m).await;
    post(&p, Kind::Push, "/git/push", json!({ "workspace": "band" })).await;

    let c = json!({ "url": url(&remote), "name": "band" });
    let (s, v, _) = post(&a, Kind::Clone, "/git/clone", c).await;
    assert_eq!(s, 200, "{v}");
    let mine = format!("{}/workspace/band", captured.home.studio);
    std::fs::write(format!("{mine}/mine.md"), "mine\n").unwrap();
    let m = json!({ "workspace": "band", "message": "my notes\n\nlong body" });
    let (s, v, _) = post(&a, Kind::Commit, "/git/commit", m).await;
    assert_eq!(s, 200, "{v}");
    let page = || Ask::Page {
        before: None,
        limit: None,
    };
    let page_now = captured.acct.history_ask(&mine, page()).await.unwrap();
    assert_eq!(page_now["checkpoints"][0]["name"], "my notes", "{page_now}");

    std::fs::write(theirs.join("n.md"), "two\n").unwrap();
    let m = json!({ "workspace": "band", "message": "second" });
    post(&p, Kind::Commit, "/git/commit", m).await;
    post(&p, Kind::Push, "/git/push", json!({ "workspace": "band" })).await;
    let (_, v, _) = post(&a, Kind::Pull, "/git/pull", json!({ "workspace": "band" })).await;
    assert_eq!(v["state"], "merged", "{v}");
    let after = captured.acct.history_ask(&mine, page()).await.unwrap();
    let label = after["entries"][0]["label"].as_str().unwrap();
    assert!(label.starts_with("pull ") && label.len() == 12, "{after}");
    let files = after["entries"][0]["files"].as_array().unwrap();
    assert!(files.contains(&json!("n.md")), "{after}");
}
