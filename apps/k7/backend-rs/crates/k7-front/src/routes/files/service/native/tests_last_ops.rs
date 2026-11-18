//! The three steps of an open (`openPrepare`, `startEngine`, `push`) answered natively: the first half, the engine's start
//! and a push through the link.

use std::sync::Arc;

use k7_capture::account::Account as Capture;
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::engine::fake::until;
use super::engine::tests_real::{Account, SESSION, account};
use super::open_prepare::prepare;
use super::projects::Proj;
use super::tests_account::text;
use super::tests_projects::proj;

fn projects(a: &Account) -> usize {
    a.kinds().iter().filter(|k| *k == "project").count()
}

/// The documents sent once the first has come and none has followed for a while.
async fn settled(a: &Account) -> usize {
    until(30, "a document", || projects(a) >= 1).await;
    let mut last = projects(a);
    loop {
        tokio::time::sleep(std::time::Duration::from_millis(600)).await;
        let now = projects(a);
        if now == last {
            return now;
        }
        last = now;
    }
}

/// A hub on loopback that keeps what it hears and answers `status`.
async fn hub(status: u16) -> (String, Arc<std::sync::Mutex<Vec<String>>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let heard = Arc::new(std::sync::Mutex::new(Vec::new()));
    let h = heard.clone();
    tokio::spawn(async move {
        while let Ok((mut sock, _)) = listener.accept().await {
            let h = h.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 8192];
                let n = sock.read(&mut buf).await.unwrap_or(0);
                h.lock()
                    .unwrap()
                    .push(String::from_utf8_lossy(&buf[..n]).into_owned());
                let body = "{\"notes\":[]}";
                let reply = format!(
                    "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = sock.write_all(reply.as_bytes()).await;
            });
        }
    });
    (url, heard)
}

/// The account's `/open` parts: its engine link, a native capture with nothing linked, and the hub at `url`.
fn wired(a: &Account, url: &str) -> Proj {
    let (mut p, _) = proj(&a.dir, "last-ops");
    p.engine = a.engine.clone();
    p.capture = Some(Arc::new(Capture::new(Arc::new(|_| {}))));
    p.hub = Some((url.to_string(), "sec".to_string()));
    p
}

fn opened(a: &Account) {
    a.open("p.k7");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_open_starts_the_engine_and_a_tree_open_starts_nothing() {
    let a = account("last-start").await;
    opened(&a);
    let e = a.engine.clone().unwrap();
    let tree = e.start_engine(&a.ws, false).await;
    assert!(tree.as_str().is_some_and(|r| !r.is_empty()), "{tree}");
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert_eq!(projects(&a), 0, "{:?}", a.kinds());
    let rev = e.start_engine(&a.ws, true).await;
    assert_eq!(rev, tree, "the revision is the workspace's, started or not");
    settled(&a).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_push_recompiles_what_changed_and_sends_it() {
    let a = account("last-push").await;
    opened(&a);
    let e = a.engine.clone().unwrap();
    e.start_engine(&a.ws, true).await;
    let before = settled(&a).await;
    let dir = format!("{}/p.k7", a.ws);
    std::fs::write(format!("{dir}/nodes/vox.k7x"), "extends(\"@meta/track\")\n").unwrap();
    std::fs::write(
        format!("{dir}/graph.k7x"),
        "modules({ gtr: [\"MIX\"], vox: [\"MIX\"] })\n",
    )
    .unwrap();
    e.push_active(&a.ws).await;
    until(30, "the second document", || projects(&a) > before).await;
    let docs: Vec<Value> = a
        .fake
        .of(SESSION)
        .into_iter()
        .filter(|f| f["type"] == "project")
        .collect();
    let (first, last) = (docs.first().unwrap(), docs.last().unwrap());
    assert_ne!(first["doc"], last["doc"], "the push sent what was on disk");
    // The marker moved to another project: the push points the session at it.
    let other = format!("{}/q.k7", a.ws);
    std::fs::create_dir_all(format!("{other}/nodes")).unwrap();
    std::fs::write(
        format!("{other}/nodes/gtr.k7x"),
        "extends(\"@meta/track\")\n",
    )
    .unwrap();
    std::fs::write(
        format!("{other}/graph.k7x"),
        "modules({ gtr: [\"MIX\"] })\n",
    )
    .unwrap();
    std::fs::write(format!("{other}/project.k7x"), "project({})\n").unwrap();
    a.open("q.k7");
    e.push_active(&a.ws).await;
    // The push answers once the start ran, so a revision read after it sees the project's own work.
    let session = e.link.session(Some(&a.ws));
    assert_eq!(session.serving().as_deref(), Some(other.as_str()));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_write_inside_the_project_pushes_through_the_link() {
    let a = account("last-write").await;
    opened(&a);
    let e = a.engine.clone().unwrap();
    e.start_engine(&a.ws, true).await;
    let before = settled(&a).await;
    let mut call = super::tests_account::call(&a.dir, "", json!({}), None);
    call.root = a.ws.clone();
    call.engine = Some(e);
    super::journal::pushed(&call).await;
    until(30, "the pushed document", || projects(&a) > before).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_first_half_of_an_open_asks_the_hub_and_pins_the_workspace() {
    let a = account("last-prepare").await;
    let (url, heard) = hub(200).await;
    let p = wired(&a, &url);
    assert!(
        prepare(&p, &a.ws).await.is_ok(),
        "the first half of an open failed"
    );
    let said = heard.lock().unwrap().join("\n").to_lowercase();
    assert!(said.contains("post /libraries/sync"), "{said}");
    assert!(said.contains("x-studio-hub-secret: sec"), "{said}");
    assert!(said.contains("{\"draft\":false}"), "{said}");
    let pins = std::fs::read_to_string(format!("{}/_library-pins.k7x", a.ws)).unwrap();
    assert_eq!(pins, "k7-core(1)\n");
    // A tree open pins too, and an admin's workspace is pinned to the draft.
    let mut admin = wired(&a, &url);
    admin.home.admin = true;
    let other = format!("{}/home/workspace/other", a.dir.display());
    std::fs::create_dir_all(&other).unwrap();
    assert!(prepare(&admin, &other).await.is_ok(), "a tree open failed");
    let pins = std::fs::read_to_string(format!("{other}/_library-pins.k7x")).unwrap();
    assert_eq!(pins, "k7-core(\"draft\")\n");
    assert!(
        heard
            .lock()
            .unwrap()
            .iter()
            .any(|h| h.contains("{\"draft\":true}"))
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_hub_that_refuses_costs_the_open_a_warning_and_nothing_else() {
    let a = account("last-refused").await;
    let (url, heard) = hub(503).await;
    let p = wired(&a, &url);
    let got = prepare(&p, &a.ws).await;
    assert!(got.is_ok());
    assert_eq!(heard.lock().unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_whole_open_is_answered_and_sends_the_document() {
    let a = account("last-open").await;
    let (url, _) = hub(200).await;
    let p = wired(&a, &url);
    let r = super::projects_open::open(&p, "songs", &a.ws, &json!({ "path": "p.k7" })).await;
    let (status, body) = text(r.expect("the open is answered")).await;
    assert_eq!(status, 200, "{body}");
    let v: Value = serde_json::from_str(&body).unwrap();
    assert!(v["rev"].as_str().is_some_and(|r| !r.is_empty()), "{body}");
    settled(&a).await;
}
