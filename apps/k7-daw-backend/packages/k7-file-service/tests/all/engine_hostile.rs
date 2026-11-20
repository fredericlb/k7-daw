//! Hostile input to the engine link's machines (the engine, the jobs path, a document): each answered, none hangs.
//! A bound is a count where one is observable; the wall time is a backstop.

use std::time::{Duration, Instant};

use k7_file_service::engine::docs::prepare;
use k7_file_service::engine::sync::{Copies, Ctx, FileKey, Reply, SyncEffect};
use k7_file_service::engine::wire::{In, decode_in};
use serde_json::{Value, json};

const BACKSTOP: u64 = if cfg!(target_env = "musl") { 60 } else { 15 };

fn within_backstop(started: Instant) {
    assert!(started.elapsed() < Duration::from_secs(BACKSTOP));
}

#[test]
fn frames_that_are_not_frames_are_ignored() {
    let started = Instant::now();
    let big = format!(r#"{{"type":"x","pad":"{}"}}"#, "a".repeat(16 << 20));
    assert!(matches!(decode_in(&big, "k7"), In::Other(..)));
    for junk in [
        "",
        "not json",
        "[1,2]",
        "null",
        "\u{0}\u{1}",
        "{\"type\":5}",
    ] {
        assert!(
            matches!(decode_in(junk, "k7"), In::Ignored | In::Other(..)),
            "{junk:?}"
        );
    }
    assert_eq!(
        decode_in(r#"{"type":"state","session":"someone-else"}"#, "k7"),
        In::Ignored
    );
    within_backstop(started);
}

#[test]
fn a_hundred_thousand_rows_and_plugins_are_read() {
    let started = Instant::now();
    let rows: Vec<Value> = (0..100_000)
        .map(|i| json!({ "id": format!("s{i}"), "loaded": i % 2 == 0 }))
        .collect();
    let In::Sessions(r) = decode_in(
        &json!({ "type": "sessions", "sessions": rows }).to_string(),
        "k7",
    ) else {
        panic!()
    };
    assert_eq!(r.len(), 100_000);
    let plugins: Vec<Value> = (0..100_000).map(|i| json!({ "name": i })).collect();
    let In::Plugins(p) = decode_in(
        &json!({ "type": "plugins", "plugins": plugins }).to_string(),
        "k7",
    ) else {
        panic!()
    };
    assert_eq!(p.as_array().unwrap().len(), 100_000);
    within_backstop(started);
}

fn stat(_: &str) -> Option<FileKey> {
    Some(FileKey {
        size: 1,
        mtime_ms: 1,
    })
}

#[test]
fn a_path_outside_the_roots_is_never_stated() {
    let roots = vec!["/srv/ws".to_string(), "/srv/libs".to_string()];
    let ctx = Ctx {
        windows: false,
        url: Some("http://m/jobs"),
        stat: &stat,
        roots: Some(&roots),
    };
    let mut c = Copies::new();
    let doc = json!({ "files": ["/etc/passwd", "/srv/ws/a.wav", "/srv/ws/../../etc/shadow", "/srv/wsx/b.wav", "/srv/libs"] });
    let fx = c.ask("s", doc, vec![], &ctx);
    let [SyncEffect::Copy { files, .. }] = fx.as_slice() else {
        panic!("{fx:?}")
    };
    assert_eq!(
        files,
        &["/srv/ws/a.wav".to_string(), "/srv/libs".to_string()]
    );
    // A document naming only what is outside goes at once, copying nothing.
    let fx = c.ask("t", json!({ "f": "/etc/passwd" }), vec![], &ctx);
    assert!(matches!(fx.as_slice(), [SyncEffect::Send(_)]));
}

#[test]
fn a_hundred_thousand_paths_and_a_huge_one_cost_a_walk() {
    let started = Instant::now();
    let roots = vec!["/srv/ws".to_string()];
    let ctx = Ctx {
        windows: false,
        url: Some("http://m/jobs"),
        stat: &|_| None,
        roots: Some(&roots),
    };
    let paths: Vec<String> = (0..100_000).map(|i| format!("/elsewhere/{i}")).collect();
    let long = format!("/srv/ws/{}", "x".repeat(64 << 10));
    let doc = json!({ "files": paths, "long": long });
    let mut c = Copies::new();
    let fx = c.ask("s", doc, vec![], &ctx);
    assert!(
        matches!(fx.as_slice(), [SyncEffect::Send(_)]),
        "no file on disk: sent at once"
    );
    within_backstop(started);
}

#[test]
fn a_job_that_lies_is_a_failure_or_nothing() {
    let ctx = Ctx {
        windows: false,
        url: Some("http://m/jobs"),
        stat: &stat,
        roots: None,
    };
    let mut c = Copies::new();
    let _ = c.ask("s", json!({ "f": "/a/b.wav" }), vec![], &ctx);
    // Progress with NaN counts reads as zero, never as a panic.
    c.progress(
        "s",
        &json!({ "files": "NaN", "filesDone": null, "bytes": -4, "bytesDone": [1] }),
    );
    let st = c.state("s").unwrap();
    assert_eq!(
        (st.files, st.files_done, st.bytes, st.bytes_done),
        (0, 0, 0, 0)
    );
    // `failures` that is no array, and a failed count that is no number.
    let fx = c.finished(
        "s",
        Reply::Synced(json!({ "failed": 1, "failures": "gone" })),
        vec![],
        &ctx,
    );
    assert!(matches!(fx.as_slice(), [SyncEffect::Log(_)]));
    assert_eq!(c.state("s").unwrap().failed, Some(1));
    assert!(c.is_held_back("s"));
    // A refusal naming another verb is a failure to retry, not a legacy machine.
    c.forget_failure("s");
    let _ = c.ask("s", json!({ "f": "/a/b.wav" }), vec![], &ctx);
    let _ = c.finished(
        "s",
        Reply::Failed("this machine does not do 'render'".into()),
        vec![],
        &ctx,
    );
    assert!(c.state("s").unwrap().error.is_some());
}

#[test]
fn a_document_the_typed_reader_refuses_still_folds_its_groups() {
    let doc = json!({ "unknownField": 1, "tracks": [{ "id": "a", "group": "g", "mute": false, "solo": false }], "groups": [{ "id": "g", "mute": true, "solo": false }] });
    let sent = prepare(&doc, None, None);
    assert!(!sent.typed);
    assert_eq!(sent.doc["tracks"][0]["mute"], json!(true));
}

#[test]
fn a_document_nested_deep_is_walked_without_a_stack() {
    let started = Instant::now();
    let mut v = json!("/a/b");
    for _ in 0..120 {
        v = json!([v]);
    }
    let mut out = vec![];
    k7_file_service::engine::sync::collect(&v, &mut out, false);
    assert_eq!(out, vec!["/a/b".to_string()]);
    within_backstop(started);
}
