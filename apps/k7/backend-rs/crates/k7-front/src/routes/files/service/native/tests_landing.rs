//! `POST /changes` that creates a project folder (I1): the samples uploaded first stay when the set lands and go with it when it
//! does not, a folder that holds a project is taken and untouched, and the other rules of a workspace set stand.

use std::path::{Path, PathBuf};

use k7_file_service::revisions::Revisions;
use serde_json::{Value, json};

use super::tests_journal::Bed;

fn text_op(file: &str, text: &str) -> Value {
    json!({ "kind": "text", "file": file, "text": text, "was": null })
}

impl Bed {
    fn ws(&self, rel: &str) -> PathBuf {
        Path::new(&self.root).join(rel)
    }

    /// A sample as `/upload` leaves it: the folder made, the bytes in.
    fn upload(&self, rel: &str) {
        let at = self.ws(rel);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(at, b"RIFFxxxx").unwrap();
    }

    fn create(&self, name: &str, ops: Value) -> Value {
        let base = Revisions::global().token(&self.home(), &self.gate_root(), None, false);
        json!({ "at": "", "base": base, "label": "import a.als", "create": name, "ops": ops })
    }
}

// I1: the set writes the texts beside the samples that came first, and the shared texts it also writes; nothing is removed.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_new_folder_lands_beside_its_uploaded_samples() {
    let b = Bed::new("land-new").await;
    b.upload("imp.k7/clips/a.wav");
    let body = b.create(
        "imp.k7",
        json!([
            text_op("imp.k7/project.k7x", "project(1)\n"),
            text_op("imp.k7/clips/c1.k7x", "clip(1)\n"),
            text_op("_modules/audio/eq/module.k7x", "module(1)\n"),
        ]),
    );
    let (status, ans) = b.changes(false, body).await;
    assert_eq!(status, 200, "{ans}");
    assert_eq!(
        std::fs::read_to_string(b.ws("imp.k7/project.k7x")).unwrap(),
        "project(1)\n"
    );
    assert!(b.ws("imp.k7/clips/a.wav").exists());
    assert!(b.ws("_modules/audio/eq/module.k7x").exists());
    // The workspace alone has no step, and the answer's rev is the workspace's token.
    assert_eq!(b.steps().await.as_array().map(Vec::len), Some(0));
    let _ = std::fs::remove_dir_all(&b.dir);
}

// I1: a folder that is a project already is a 409 `taken`, and neither it nor what came into it is touched.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_name_taken_is_a_409_and_the_folder_stays() {
    let b = Bed::new("land-taken").await;
    std::fs::create_dir_all(b.ws("imp.k7")).unwrap();
    std::fs::write(b.ws("imp.k7/project.k7x"), "project(2)\n").unwrap();
    b.upload("imp.k7/clips/a.wav");
    let body = b.create("imp.k7", json!([text_op("imp.k7/other.k7x", "x(1)\n")]));
    let (status, ans) = b.changes(false, body).await;
    assert_eq!(status, 409, "{ans}");
    assert_eq!(ans["taken"], json!(true));
    assert_eq!(
        std::fs::read_to_string(b.ws("imp.k7/project.k7x")).unwrap(),
        "project(2)\n"
    );
    assert!(b.ws("imp.k7/clips/a.wav").exists() && !b.ws("imp.k7/other.k7x").exists());
    let _ = std::fs::remove_dir_all(&b.dir);
}

// I1, the brief's mutation "a failed commit leaves the uploaded samples": a commit failing after the uploads takes the folder away.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_commit_that_fails_removes_the_folder_and_its_samples() {
    let b = Bed::new("land-fail").await;
    b.upload("imp.k7/clips/a.wav");
    // A file stands where the set needs a folder: the landing fails on the disk, after the plan and the freshness passed.
    let body = b.create(
        "imp.k7",
        json!([
            text_op("imp.k7/project.k7x", "project(1)\n"),
            text_op("imp.k7/clips/a.wav/under.k7x", "x(1)\n"),
            text_op("_modules/audio/eq/module.k7x", "module(1)\n"),
        ]),
    );
    let (status, ans) = b.changes(false, body).await;
    assert_eq!(status, if cfg!(windows) { 404 } else { 400 }, "{ans}");
    assert!(
        !b.ws("imp.k7").exists(),
        "the folder and its samples are gone"
    );
    assert!(
        !b.ws("_modules/audio/eq").exists(),
        "the shared text did not land either"
    );
    let _ = std::fs::remove_dir_all(&b.dir);
}

// I1: every refusal that comes before the landing leaves no new folder, and the folder's own rules are 400s that touch nothing.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_refused_set_removes_the_new_folder_and_a_bad_name_touches_nothing() {
    let b = Bed::new("land-refused").await;
    for (ops, want) in [
        // Outside the folder and not a shared text.
        (json!([text_op("other/x.k7x", "x(1)\n")]), 403),
        // A written `.k7x` that does not parse.
        (
            json!([text_op("imp.k7/n.k7x", "extends(\"unterminated\n")]),
            422,
        ),
        // A stale `was`: the file is not there, the set says it was.
        (
            json!([{ "kind": "text", "file": "imp.k7/a.k7x", "text": "a(1)\n", "was": "00" }]),
            409,
        ),
    ] {
        b.upload("imp.k7/clips/a.wav");
        let (status, ans) = b.changes(false, b.create("imp.k7", ops.clone())).await;
        assert_eq!(status, want, "{ops}: {ans}");
        assert!(!b.ws("imp.k7").exists(), "{ops}");
    }
    b.upload("keep/a.wav");
    for name in ["../x", "a/b", "a\\b", "_modules", ".hidden", "", ".."] {
        let (status, ans) = b.changes(false, b.create(name, json!([]))).await;
        assert_eq!(status, 400, "{name}: {ans}");
    }
    // `create` is for the workspace alone: a project's `at` is refused, and the folder is not touched.
    let mut at_project = b.create("keep", json!([]));
    at_project["at"] = json!("p.k7");
    assert_eq!(b.changes(false, at_project).await.0, 400);
    assert!(b.ws("keep/a.wav").exists());
    // The shared texts of an ordinary workspace set are still only the shared texts.
    let plain =
        json!({ "at": "", "base": "x", "label": "x", "ops": [text_op("imp.k7/x.k7x", "x(1)\n")] });
    assert_eq!(b.changes(false, plain).await.0, 403);
    let _ = std::fs::remove_dir_all(&b.dir);
}

// I2: an import whose uploads failed asks for the folder to be taken, and a folder that holds a project is never taken.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_discard_takes_the_samples_and_spares_a_project() {
    let b = Bed::new("land-discard").await;
    b.upload("imp.k7/clips/a.wav");
    let mut body = b.create("imp.k7", json!([]));
    body["discard"] = json!(true);
    let (status, ans) = b.changes(false, body.clone()).await;
    assert_eq!((status, &ans["discarded"]), (200, &json!(true)), "{ans}");
    assert!(!b.ws("imp.k7").exists());
    // The same ask on a folder that holds a project changes nothing.
    std::fs::create_dir_all(b.ws("mine.k7")).unwrap();
    std::fs::write(b.ws("mine.k7/project.k7x"), "project(2)\n").unwrap();
    body["create"] = json!("mine.k7");
    assert_eq!(b.changes(false, body.clone()).await.0, 200);
    assert!(b.ws("mine.k7/project.k7x").exists());
    // A bad name is the same 400 as for a set, and `at` must be empty.
    body["create"] = json!("../x");
    assert_eq!(b.changes(false, body).await.0, 400);
    let _ = std::fs::remove_dir_all(&b.dir);
}
