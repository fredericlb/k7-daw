//! The engine link's pure rules against `vectors/engine.json`: the session names, `servedProject`, the backoff, and what
//! the send does to a document over the golden corpus (the paths it names, a take's hole, the groups' mix).

use k7_file_service::engine::{served_project, session_for, session_of};
use serde_json::{Value, json};

use crate::support::engine::vectors;

#[test]
fn the_served_project_is_named_by_address() {
    let cases = vectors()["served_project"].as_array().unwrap();
    assert!(cases.len() >= 1260);
    for c in cases {
        let (owner, files_root, root, asked) = (
            c["owner"].as_str().unwrap(),
            c["files_root"].as_str().unwrap(),
            c["root"].as_str().unwrap(),
            c["asked"].as_str().unwrap(),
        );
        // The session the workspace is served under, and the two keys `servedProject` looks it up by.
        let held = session_of(Some(root), files_root, owner);
        let found = held == session_for("k7", Some(asked)) || held == "k7";
        let got = found
            .then(|| served_project(files_root, root, c["project"].as_str().unwrap()))
            .flatten()
            .map_or(
                Value::Null,
                |(workspace, path)| json!({ "workspace": workspace, "path": path }),
            );
        assert_eq!(got, c["out"], "{c}");
    }
}
