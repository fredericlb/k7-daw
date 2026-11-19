//! The copy to the engine's machine (2223) against the TypeScript's traces: documents asked, jobs answered on a turned
//! clock, what is sent and what the poll shows. `settled` is only set by a job that answered.


use k7_file_service::engine::sync::{Copies, Ctx, FileKey, Reply, SyncEffect, within};
use serde_json::json;






fn one_file(_: &str) -> Option<FileKey> {
    Some(FileKey {
        size: 5,
        mtime_ms: 7,
    })
}

#[test]
fn a_copy_for_a_machine_the_binding_left_settles_nothing_of_the_new_one() {
    // F6: a copy to A, the binding moves to B, the session asks again mid-copy; A's answer must not settle B's files.
    let mut c = Copies::new();
    let doc = json!({ "f": "/ws/a.wav" });
    let on = |url| Ctx {
        windows: false,
        url: Some(url),
        stat: &one_file,
        roots: None,
    };
    let fx = c.ask("s", doc.clone(), vec![], &on("ws://a/jobs"));
    assert!(matches!(fx.as_slice(), [SyncEffect::Copy { url, .. }] if url == "ws://a/jobs"));
    assert!(
        c.ask("s", doc, vec![], &on("ws://b/jobs")).is_empty(),
        "the run in flight takes the document"
    );
    let ok = Reply::Synced(json!({ "type": "synced", "failed": 0 }));
    let fx = c.finished("s", ok, vec![], &on("ws://b/jobs"));
    assert!(
        matches!(fx.as_slice(), [SyncEffect::Copy { url, files }] if url == "ws://b/jobs" && files == &["/ws/a.wav"]),
        "the new machine is copied to, not sent a document without its files: {fx:?}"
    );
}

#[test]
fn the_roots_compare_the_way_a_windows_host_spells_a_path() {
    // F5: separators and a drive's case are one path, and nothing else is inside.
    let roots = vec!["C:/a".to_string(), "\\\\srv\\share".to_string()];
    for inside in [
        "C:\\a\\x",
        "C:/a/x",
        "c:/a/x",
        "C:/a",
        "\\\\srv\\share\\f",
        "//srv/share/f",
    ] {
        assert!(within(inside, &roots, true), "{inside}");
    }
    for outside in [
        "C:/ab",
        "C:",
        "/a",
        "C:/a/../b",
        "D:/a/x",
        "\\\\srv\\other\\f",
    ] {
        assert!(!within(outside, &roots, true), "{outside}");
    }
    // On another host a backslash and a lower-case drive are not the root's spelling.
    assert!(!within("c:/a/x", &roots, false));
    assert!(!within("C:\\a\\x", &roots, false));
    assert!(within("C:/a/x", &roots, false));
}
