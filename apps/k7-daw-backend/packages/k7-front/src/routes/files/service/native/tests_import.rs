//! `POST /import` through a real account and `dispatch`: a dropped `.dawproject` and a `.als` of the workspace land as a project folder,
//! the refusals leave nothing, a typed name that is taken is never renamed. The fixtures are k7-import's own.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{Value, json};

use super::Tag;
use super::tests_account::text;
use super::tests_journal::Bed;
use crate::routes::files::service::routes::{self, H};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../k7-import/tests/fixtures")
}

impl Bed {
    fn songs(&self) -> PathBuf {
        PathBuf::from(&self.root)
    }

    /// `POST /import` with the dropped bytes, the name and the folder asked for riding where the headers put them.
    async fn drop_file(&self, file: &str, bytes: Vec<u8>, folder: Option<&str>) -> (u16, Value) {
        let mut body = json!({ "file": file });
        if let Some(f) = folder {
            body["as"] = json!(f);
        }
        let mut c = self.call(Tag::Import, body);
        c.raw = Some(Arc::new(bytes));
        self.answer(c).await
    }

    async fn import_path(&self, rel: &str, folder: Option<&str>) -> (u16, Value) {
        let mut body = json!({ "rel": rel });
        if let Some(f) = folder {
            body["as"] = json!(f);
        }
        self.answer(self.call(Tag::Import, body)).await
    }

    async fn answer(&self, c: routes::Call) -> (u16, Value) {
        let (s, t) = text(routes::dispatch(H::Import, &c).await).await;
        (s, serde_json::from_str(&t).unwrap_or(Value::Null))
    }

    /// Every file under the workspace, by path from its root.
    fn tree(&self) -> Vec<String> {
        fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
            for e in std::fs::read_dir(dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(root, &p, out);
                } else {
                    out.push(
                        p.strip_prefix(root)
                            .unwrap()
                            .to_string_lossy()
                            .replace('\\', "/"),
                    );
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.songs(), &self.songs(), &mut out);
        out.sort();
        out
    }

    fn bytes(&self, rel: &str) -> Vec<u8> {
        std::fs::read(self.songs().join(rel)).unwrap_or_else(|_| panic!("no file {rel}"))
    }
}

fn fixture(rel: &str) -> Vec<u8> {
    std::fs::read(fixtures().join(rel)).unwrap()
}

// A drop lands the project's texts and its audio in a new folder, the name asked for is the folder's, and a second drop of it is refused whole.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_dropped_dawproject_lands_and_a_taken_name_is_refused() {
    let b = Bed::new("imp-drop").await;
    let (status, ans) = b
        .drop_file(
            "hiver.dawproject",
            fixture("hiver.dawproject"),
            Some("hiver"),
        )
        .await;
    assert_eq!(status, 200, "{ans}");
    assert_eq!(ans["project"], json!("hiver"));
    let log = ans["log"].as_array().unwrap();
    assert!(
        log.iter().any(|l| l
            .as_str()
            .unwrap()
            .contains("3 tracks · 1 audio clips · 2 MIDI clips")),
        "{log:?}"
    );
    assert_eq!(log.last().unwrap(), "wrote hiver");

    assert!(
        b.tree().contains(&"hiver/graph.k7x".to_string()),
        "{:?}",
        b.tree()
    );
    assert_eq!(
        b.bytes("hiver/clips/vox.wav"),
        fixture("workspace/Samples/vox.wav")
    );
    let lane = String::from_utf8(b.bytes("hiver/nodes/drums.k7x")).unwrap();
    assert!(
        lane.contains("ref(\"drums_0\")") && lane.contains("ref(\"drums_1\")"),
        "{lane}"
    );

    let before = b.tree();
    let (status, ans) = b
        .drop_file(
            "hiver.dawproject",
            fixture("hiver.dawproject"),
            Some("hiver"),
        )
        .await;
    assert_eq!((status, &ans["taken"]), (409, &json!(true)), "{ans}");
    assert_eq!(ans["error"], json!("\"hiver\" already exists"));
    assert_eq!(b.tree(), before, "a refused name writes nothing");

    // A name nobody typed is made free with a suffix.
    let (status, ans) = b
        .drop_file("hiver.dawproject", fixture("hiver.dawproject"), None)
        .await;
    assert_eq!((status, &ans["project"]), (200, &json!("hiver-2")), "{ans}");
}

// A `.als` of the tree reads the samples beside it, counts the one outside and lands its plugin state in the shared modules.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_als_of_the_workspace_reads_the_samples_beside_it() {
    let b = Bed::new("imp-als").await;
    std::fs::create_dir_all(b.songs().join("Samples")).unwrap();
    std::fs::write(b.songs().join("hiver.als"), fixture("workspace/hiver.als")).unwrap();
    std::fs::write(
        b.songs().join("Samples/vox.wav"),
        fixture("workspace/Samples/vox.wav"),
    )
    .unwrap();

    let (status, ans) = b.import_path("hiver.als", None).await;
    assert_eq!(status, 200, "{ans}");
    assert_eq!(ans["project"], json!("hiver"));
    let log: Vec<&str> = ans["log"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l.as_str().unwrap())
        .collect();
    assert!(
        log.iter()
            .any(|l| l.starts_with("warning: 1 sample(s) live outside the workspace")),
        "{log:?}"
    );
    assert_eq!(
        b.bytes("hiver/clips/vox.wav"),
        fixture("workspace/Samples/vox.wav")
    );
    assert_eq!(
        b.bytes("_modules/instruments/diva/diva.vstpreset"),
        [0x0A, 0x0B, 0x0C, 0x0D]
    );
    assert!(
        b.tree()
            .contains(&"_modules/instruments/diva/module.k7x".to_string())
    );
    // The source file stays where it was.
    assert!(b.songs().join("hiver.als").is_file());
}

// A file that does not translate, a lone `.als` dropped and a file past the ceiling are each refused, and nothing is left behind.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_refused_import_leaves_the_workspace_as_it_was() {
    let b = Bed::new("imp-refuse").await;
    let before = b.tree();
    let (status, ans) = b
        .drop_file("x.dawproject", b"PK\x03\x04 not a zip".to_vec(), Some("x"))
        .await;
    assert_eq!(status, 422, "{ans}");
    let (status, ans) = b
        .drop_file("lone.als", fixture("workspace/hiver.als"), Some("lone"))
        .await;
    assert_eq!(status, 400, "{ans}");
    assert!(
        ans["error"]
            .as_str()
            .unwrap()
            .contains("import it from the tree"),
        "{ans}"
    );
    let (status, ans) = b
        .drop_file(
            "big.dawproject",
            vec![0u8; k7_import::MAX_IMPORT_BYTES + 1],
            Some("big"),
        )
        .await;
    assert_eq!(status, 413, "{ans}");
    let (status, _) = b.import_path("nothing.als", None).await;
    assert_eq!(status, 404);
    let (status, ans) = b.drop_file("e.dawproject", Vec::new(), Some("e")).await;
    assert_eq!(status, 400, "{ans}");
    assert_eq!(b.tree(), before);
    assert!(!b.songs().join("x").exists() && !b.songs().join("big").exists());
}
