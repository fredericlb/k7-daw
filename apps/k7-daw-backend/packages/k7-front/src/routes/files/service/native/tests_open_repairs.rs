//! An open never writes a library project the account cannot write.

use serde_json::json;

use super::projects::Kind;
use super::tests::scratch;
use super::tests_projects::{go, make, proj};
use super::tests_projects_open::files_of;

// A project of the library `k7-core`, copied from a made one.
fn library_project(dir: &std::path::Path) -> std::path::PathBuf {
    let lib = dir.join("libraries/k7-core/demo.k7");
    let made = dir.join("home/workspace/songs/a.k7");
    std::fs::create_dir_all(&lib).unwrap();
    for e in std::fs::read_dir(&made).unwrap().flatten() {
        if e.path().is_file() {
            std::fs::copy(e.path(), lib.join(e.file_name())).unwrap();
        }
    }
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
