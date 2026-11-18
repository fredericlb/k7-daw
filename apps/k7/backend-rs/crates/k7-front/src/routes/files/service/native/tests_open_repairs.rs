//! The open's repairs (3j M3): they wait for the journal's gate, and they never write a library project the account cannot write.

use std::time::Duration;

use serde_json::json;

use super::projects::Kind;
use super::tests::scratch;
use super::tests_projects::{go, make, proj, root_of};
use super::tests_projects_open::{REC, files_of};

// A project of the library `k7-core`, with a recorder node the repairs would delete.
fn library_project(dir: &std::path::Path) -> std::path::PathBuf {
    let lib = dir.join("libraries/k7-core/demo.k7");
    let made = dir.join("home/workspace/songs/a.k7");
    std::fs::create_dir_all(&lib).unwrap();
    for e in std::fs::read_dir(&made).unwrap().flatten() {
        if e.path().is_file() {
            std::fs::copy(e.path(), lib.join(e.file_name())).unwrap();
        }
    }
    std::fs::create_dir_all(lib.join("chains")).unwrap();
    std::fs::write(lib.join("chains/rec.k7x"), REC).unwrap();
    lib
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_open_of_a_library_project_the_account_cannot_write_repairs_nothing() {
    let dir = scratch("po-library");
    let (mut p, lines) = proj(&dir, "po-library");
    p.demos_from.1 = format!("{}/../../../..", env!("CARGO_MANIFEST_DIR"));
    make(&dir, "a.k7").await;
    let lib = library_project(&dir);
    let before = files_of(&lib);
    let (status, body) = go(
        &p,
        Kind::Open,
        &dir,
        json!({ "path": "_libraries/k7-core/demo.k7" }),
    )
    .await;
    // Node's answer: the open succeeds, its info naming no active project, and the library is untouched.
    assert_eq!(status, 200, "{body}");
    assert!(body.contains("\"activeProject\":null"), "{body}");
    assert_eq!(files_of(&lib), before, "a library project was rewritten");
    let said = lines.lock().unwrap().join("\n");
    assert!(said.contains("repairs skipped"), "{said}");
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_open_waits_for_the_journals_gate_before_it_repairs() {
    let dir = scratch("po-gate");
    let (mut p, _) = proj(&dir, "po-gate");
    p.demos_from.1 = format!("{}/../../../..", env!("CARGO_MANIFEST_DIR"));
    make(&dir, "a.k7").await;
    let rec = dir.join("home/workspace/songs/a.k7/chains/rec.k7x");
    std::fs::write(&rec, REC).unwrap();
    let held = k7_file_service::journal::Journals::global()
        .exclusive(&k7_file_service::paths::resolve_all(&[&root_of(&dir)]))
        .await;
    let (p2, dir2) = (p.clone(), dir.clone());
    let task =
        tokio::spawn(async move { go(&p2, Kind::Open, &dir2, json!({ "path": "a.k7" })).await });
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!task.is_finished(), "the open ran inside a held gate");
    assert!(rec.exists(), "a repair landed under a held gate");
    drop(held);
    assert_eq!(task.await.unwrap().0, 200);
    assert!(!rec.exists());
    std::fs::remove_dir_all(dir).unwrap();
}
