//! vectors.rs — the history functions held to `fixtures/sync-vectors.json`, the frozen reference: the same pages and
//! the same files at every point as the TypeScript read them from the room folders it wrote.
//! What a reader gets wrong: the fixture is committed and never regenerated; its generator left with `live-sync`.

use crate::common;

use std::path::Path;

use common::{FakeJs, temp, write_files};
use serde_json::{Map, Value};

fn vectors() -> Value {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/sync-vectors.json");
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn check_history(dir: &Path, want: &Value) {
    let head = k7_sync::log::head_on_disk(dir).unwrap();
    assert_eq!(head, want["head"].as_f64().unwrap());
    let page = k7_sync::log::entries_before(dir, head + 1.0, 4).unwrap();
    assert_eq!(Value::Array(page), want["page"]);
    for s in 0..=head as u64 {
        let got = match k7_sync::log::files_at(&FakeJs, dir, s as f64) {
            Ok(files) => Value::Object(
                files
                    .into_iter()
                    .map(|(k, t)| (k, Value::String(t)))
                    .collect::<Map<_, _>>(),
            ),
            Err(e) => serde_json::json!({ "error": e.message() }),
        };
        assert_eq!(got, want["at"][s.to_string()], "files at {s}");
    }
}

#[test]
fn the_history_functions_read_as_the_typescript_reads() {
    let v = vectors();
    let root = temp("history");
    write_files(&root, v["relay"]["phase2"]["dir"].as_object().unwrap());
    check_history(&root.join("acct/.sync-relay/w"), &v["history"]);
    let _ = std::fs::remove_dir_all(&root);
}
