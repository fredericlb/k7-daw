//! The rulings built on the recommended side (F1, F3): a rename waits for the journal's gate however long, and a folder
//! renamed above the open project leaves the inner project open at its new path, in the marker.

use std::time::Duration;

use k7_file_service::journal::Journals;
use k7_file_service::paths::resolve_all;
use serde_json::json;

use super::projects::Kind;
use super::tests::scratch;
use super::tests_projects::{go, make, marker, proj, root_of};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_rename_behind_a_record_waits_for_it_and_never_answers_409() {
    let dir = scratch("p-wait");
    let (p, _) = proj(&dir, "p-wait");
    make(&dir, "a.k7").await;
    // A record, which holds the gate shared: a rename that took it shared too would not wait.
    let alone = Journals::global()
        .enter(&resolve_all(&[&root_of(&dir)]))
        .await;
    let (asking, d2) = (p.clone(), dir.clone());
    let client = tokio::spawn(async move {
        go(
            &asking,
            Kind::Rename,
            &d2,
            json!({ "path": "a.k7", "name": "b.k7" }),
        )
        .await
    });
    // The shrunk bound of a restored 409 is 100 ms: past it, the rename must still be waiting.
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(!client.is_finished());
    assert!(dir.join("home/workspace/songs/a.k7").exists());
    drop(alone);
    let (status, body) = client.await.unwrap();
    assert_eq!((status, body.as_str()), (200, r#"{"path":"b.k7"}"#));
    assert!(dir.join("home/workspace/songs/b.k7").exists());
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_folder_renamed_above_the_open_project_keeps_the_inner_project_open() {
    let dir = scratch("p-carry");
    let (p, _) = proj(&dir, "p-carry");
    for n in ["outer.k7", "b.k7"] {
        make(&dir, n).await;
    }
    let songs = dir.join("home/workspace/songs");
    std::fs::rename(songs.join("b.k7"), songs.join("outer.k7/b.k7")).unwrap();
    let open = json!({ "path": "outer.k7/b.k7" });
    assert_eq!(go(&p, Kind::Open, &dir, open).await.0, 200);
    let rename = json!({ "path": "outer.k7", "name": "o2.k7" });
    assert_eq!(go(&p, Kind::Rename, &dir, rename).await.0, 200);
    assert_eq!(marker(&dir), "o2.k7/b.k7\n");
    std::fs::remove_dir_all(dir).unwrap();
}
