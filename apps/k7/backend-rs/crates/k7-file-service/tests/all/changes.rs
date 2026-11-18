//! A change set (3-B2, 3-B7, 3-B8): the body's shape and limits, the gates, the parse, the stale rules, and a landing that is whole or nothing.

use crate::support;

use k7_file_service::changes::fresh::fresh;
use k7_file_service::changes::request::parse;
use k7_file_service::changes::{MAX_OPS, MAX_TEXT, Refusal};
use k7_file_service::journal::Mode;
use k7_file_service::text_hash::text_hash;
use serde_json::{Value, json};
use support::changes::{body, planned, refused, text};
use support::entries::Fx;

// 3-B8: more than 2,000 ops, or more than 8 MiB of text, is a 413 before any op is read.
#[test]
fn a_set_past_the_limits_is_refused_before_any_op_is_read() {
    let many: Vec<Value> = (0..=MAX_OPS)
        .map(|i| json!({ "kind": "remove", "file": format!("song.k7/{i}.txt"), "was": null }))
        .collect();
    let r = parse(&body(Value::Array(many))).unwrap_err();
    assert_eq!(r.status(), 413, "{r}");
    let fine: Vec<Value> = (0..MAX_OPS)
        .map(|i| json!({ "kind": "remove", "file": format!("song.k7/{i}.txt"), "was": null }))
        .collect();
    assert!(parse(&body(Value::Array(fine))).is_ok());
    let big = "x".repeat(MAX_TEXT + 1);
    let r = parse(&body(json!([text("song.k7/a.txt", &big, json!(null))]))).unwrap_err();
    assert_eq!(r.status(), 413, "{r}");
    let edge = "x".repeat(MAX_TEXT);
    assert!(parse(&body(json!([text("song.k7/a.txt", &edge, json!(null))]))).is_ok());
}

// The body's own shape: a missing `was` is not "absent", an unknown kind and a loose field are 400s with a sentence.
#[test]
fn a_malformed_body_is_a_400_with_the_field_named() {
    let said = |v: Value| parse(&v).unwrap_err().to_string();
    let op = |o: Value| body(json!([o]));
    assert_eq!(
        said(op(json!({ "kind": "text", "file": "a", "text": "x" }))),
        "ops[0].was: expected a string or null"
    );
    assert_eq!(
        said(op(json!({ "kind": "zap", "file": "a" }))),
        "ops[0].kind: unknown kind 'zap'"
    );
    assert_eq!(
        said(op(json!({ "kind": "copy", "file": "a" }))),
        "ops[0].from: expected a string"
    );
    assert_eq!(
        said(op(
            json!({ "kind": "move", "from": "a", "file": "b", "replace": 1 })
        )),
        "ops[0].replace: expected a boolean"
    );
    assert_eq!(
        said(json!({ "at": "p", "base": "t", "label": "l" })),
        "ops: expected an array"
    );
    assert_eq!(
        said(json!({ "at": "p", "base": "", "label": "l", "ops": [] })),
        "base: must not be empty"
    );
    assert_eq!(
        said(json!({ "at": "p", "base": "t", "label": "l", "ops": [], "journal": "other" })),
        "journal: expected 'join' or 'skip'"
    );
    assert_eq!(
        said(json!({ "at": "p", "base": "t", "label": "l", "ops": [], "want": ["views"] })),
        "want: expected a list of 'graph'"
    );
    let ok = parse(&json!({ "at": "p", "base": "t", "label": "l", "ops": [], "journal": "join", "want": ["graph"], "reload": true })).unwrap();
    assert!((ok.journal, ok.want_graph, ok.reload) == (Mode::Join, true, true));
    let skip =
        parse(&json!({ "at": "p", "base": "t", "label": "l", "ops": [], "journal": "skip" }))
            .unwrap();
    assert!(skip.journal == Mode::Skip);
}

// 3-B8, the gates: every path of the set passes the workspace's two gates and the hidden names; one bad path refuses the whole set.
#[test]
fn every_path_passes_the_gates_and_one_bad_path_refuses_the_set() {
    let fx = Fx::new("c-gates");
    let ok = text("song.k7/a.txt", "a", json!(null));
    for (bad, want) in [
        (
            "../outside.txt",
            "path escapes the workspace: ../outside.txt",
        ),
        ("/etc/passwd", "path escapes the workspace: /etc/passwd"),
        (
            "song.k7/.history/j.json",
            "'song.k7/.history/j.json' is hidden state, which a change set never touches",
        ),
        (
            ".sync-relay/x",
            "'.sync-relay/x' is hidden state, which a change set never touches",
        ),
        (
            "_recycle_bin/a.txt",
            "'_recycle_bin/a.txt' is hidden state, which a change set never touches",
        ),
        (
            "_libraries/UPPER/a.txt",
            "names no library: _libraries/UPPER/a.txt",
        ),
    ] {
        let r = refused(&fx, true, json!([ok.clone(), text(bad, "x", json!(null))]));
        assert_eq!((r.status(), r.to_string().as_str()), (403, want), "{bad}");
    }
    // The gates cover every kind's paths, a read source too.
    for o in [
        json!({ "kind": "remove", "file": "../x", "was": null }),
        json!({ "kind": "copy", "from": "../x", "file": "song.k7/b.txt" }),
        json!({ "kind": "copy", "from": "song.k7/clips/riff.k7x", "file": "../x" }),
        json!({ "kind": "move", "from": "song.k7/clips/riff.k7x", "file": "../x" }),
        json!({ "kind": "adopt", "job": "j1", "out": "a.wav", "file": "../x" }),
    ] {
        assert_eq!(refused(&fx, true, json!([o.clone()])).status(), 403, "{o}");
    }
    assert!(planned(&fx, true, json!([ok])).is_ok());
}

// A library is written by the origin's admins alone, and only below its draft root.
#[test]
fn a_library_is_written_by_an_admin_alone() {
    let fx = Fx::new("c-libs");
    let ops = json!([text(
        "_libraries/k7-core/modules/audio/verb/module.k7x",
        "extends(\"@engine/reverb\")\n// admin\n",
        json!(null)
    )]);
    let r = refused(&fx, false, ops.clone());
    assert_eq!(r.status(), 403);
    assert_eq!(
        r.to_string(),
        "can't write '_libraries/k7-core/modules/audio/verb/module.k7x': only the origin's admins edit a library"
    );
    assert!(planned(&fx, true, ops).is_ok());
    // A copy out of a library is a read; a move out of one is a write there.
    let copy = json!([{ "kind": "copy", "from": "_libraries/k7-core/modules/audio/verb/module.k7x", "file": "song.k7/v.k7x" }]);
    assert!(planned(&fx, false, copy).is_ok());
    let take = json!([{ "kind": "move", "from": "_libraries/k7-core/modules/audio/verb/module.k7x", "file": "song.k7/v.k7x" }]);
    assert_eq!(refused(&fx, false, take).status(), 403);
}

// What a text op may not be: a binary, YAML; and what two ops may not do: name one file.
#[test]
fn a_text_op_names_no_binary_no_yaml_and_no_file_twice() {
    let fx = Fx::new("c-names");
    assert_eq!(
        refused(&fx, true, json!([text("song.k7/a.wav", "x", json!(null))])).to_string(),
        "cannot edit a binary file"
    );
    assert_eq!(
        refused(&fx, true, json!([text("song.k7/a.yml", "x", json!(null))])).to_string(),
        "song.k7/a.yml: a workspace file is .k7x, not YAML"
    );
    let twice = json!([
        text("song.k7/a.txt", "1", json!(null)),
        text("song.k7/./a.txt", "2", json!(null))
    ]);
    assert_eq!(
        refused(&fx, true, twice).to_string(),
        "two ops name 'song.k7/./a.txt'"
    );
    let moved = json!([{ "kind": "move", "from": "song.k7/clips/riff.k7x", "file": "song.k7/r.k7x" }, text("song.k7/clips/riff.k7x", "x", json!("h"))]);
    assert_eq!(refused(&fx, true, moved).status(), 400);
    let read_written = json!([text("song.k7/a.txt", "1", json!(null)), { "kind": "copy", "from": "song.k7/a.txt", "file": "song.k7/b.txt" }]);
    assert_eq!(refused(&fx, true, read_written).status(), 400);
    for (o, why) in [
        (
            json!({ "kind": "copy", "from": "song.k7/clips", "file": "song.k7/c" }),
            "'song.k7/clips' is a folder, not a file",
        ),
        (
            json!({ "kind": "copy", "from": "song.k7/none.txt", "file": "song.k7/c" }),
            "'song.k7/none.txt': no such file",
        ),
        (
            json!({ "kind": "adopt", "job": "../j", "out": "a.wav", "file": "song.k7/a.wav" }),
            "'../j/a.wav' names no staged output",
        ),
        (
            json!({ "kind": "adopt", "job": "j1", "out": "../a.wav", "file": "song.k7/a.wav" }),
            "'j1/../a.wav' names no staged output",
        ),
        (
            json!({ "kind": "adopt", "job": "j1", "out": "a.wav", "file": "song.k7/a.wav" }),
            "'j1/a.wav': no such file",
        ),
    ] {
        let r = refused(&fx, true, json!([o]));
        assert_eq!((r.status(), r.to_string().as_str()), (400, why));
    }
}

// 3-B7: a written .k7x that does not parse is a 422 naming the file; the check comes after the gates and before freshness.
#[test]
fn a_written_k7x_that_does_not_parse_is_a_422() {
    let fx = Fx::new("c-parse");
    let garbage = "extends(\"unterminated\n";
    let r = refused(
        &fx,
        true,
        json!([text("song.k7/n.k7x", garbage, json!(null))]),
    );
    assert_eq!(r.status(), 422);
    match &r {
        Refusal::Unparsable { file, error } => {
            assert_eq!(file, "song.k7/n.k7x");
            assert!(error.starts_with("song.k7/n.k7x: "), "{error}");
        }
        other => panic!("{other:?}"),
    }
    // The same garbage in a .txt, and a good .k7x, are written.
    assert!(
        planned(
            &fx,
            true,
            json!([
                text("song.k7/n.txt", garbage, json!(null)),
                text("song.k7/n.k7x", "x(1)\n", json!(null))
            ])
        )
        .is_ok()
    );
    // Forbidden outranks unparsable.
    let both = json!([
        text("song.k7/n.k7x", garbage, json!(null)),
        text("../x.txt", "x", json!(null))
    ]);
    assert_eq!(refused(&fx, true, both).status(), 403);
    // Unparsable outranks stale: a `was` that moved is never reached.
    fx.s.file("studio/workspace/songs/song.k7/n.k7x", "x(2)\n");
    let p = refused(
        &fx,
        true,
        json!([text("song.k7/n.k7x", garbage, json!("old"))]),
    );
    assert_eq!(p.status(), 422);
}

// 3-B2: the project's token, and each written file's `was`; both name what moved and a 409 writes nothing.
#[test]
fn a_stale_base_is_judged_by_the_token_and_by_each_was() {
    let fx = Fx::new("c-fresh");
    fx.s.file("studio/workspace/songs/song.k7/a.txt", "disk");
    let set = |was: Value| planned(&fx, true, json!([text("song.k7/a.txt", "mine", was)])).unwrap();
    // The token alone.
    assert_eq!(
        fresh(&set(json!(text_hash("disk"))), "t1", "t2"),
        Err(Refusal::Stale {
            files: vec![],
            project: true
        })
    );
    // A `was` alone: the file moved under a token that did not.
    assert_eq!(
        fresh(&set(json!(text_hash("opened"))), "t1", "t1"),
        Err(Refusal::Stale {
            files: vec!["song.k7/a.txt".into()],
            project: false
        })
    );
    // Both, named together.
    assert_eq!(
        fresh(&set(json!(text_hash("opened"))), "t1", "t9"),
        Err(Refusal::Stale {
            files: vec!["song.k7/a.txt".into()],
            project: true
        })
    );
    assert_eq!(fresh(&set(json!(text_hash("disk"))), "t1", "t1"), Ok(()));
    // `null` says the file is absent: a file that appeared is stale, an absent one is not.
    assert_eq!(
        fresh(&set(json!(null)), "t1", "t1"),
        Err(Refusal::Stale {
            files: vec!["song.k7/a.txt".into()],
            project: false
        })
    );
    let fresh_file = planned(
        &fx,
        true,
        json!([text("song.k7/new.txt", "n", json!(null))]),
    )
    .unwrap();
    assert_eq!(fresh(&fresh_file, "t1", "t1"), Ok(()));
    let gone = planned(
        &fx,
        true,
        json!([text("song.k7/new.txt", "n", json!(text_hash("was there")))]),
    )
    .unwrap();
    assert_eq!(
        fresh(&gone, "t1", "t1"),
        Err(Refusal::Stale {
            files: vec!["song.k7/new.txt".into()],
            project: false
        })
    );
    // A save of what the file already holds is no conflict, as `PUT /file` lets it through.
    let same = planned(
        &fx,
        true,
        json!([text("song.k7/a.txt", "disk", json!(text_hash("opened")))]),
    )
    .unwrap();
    assert_eq!(fresh(&same, "t1", "t1"), Ok(()));
}

// 3-B2 for the other kinds: a remove names what it removes, a destination that exists is replaced only on request.
#[test]
fn a_remove_and_a_destination_are_judged_too() {
    let fx = Fx::new("c-fresh-kinds");
    fx.s.file("studio/workspace/songs/song.k7/a.txt", "disk");
    fx.s.file("studio/workspace/songs/song.k7/b.bin.txt", "b");
    let p = planned(
        &fx,
        true,
        json!([{ "kind": "remove", "file": "song.k7/a.txt", "was": text_hash("opened") }]),
    )
    .unwrap();
    assert_eq!(
        fresh(&p, "t1", "t1"),
        Err(Refusal::Stale {
            files: vec!["song.k7/a.txt".into()],
            project: false
        })
    );
    let p = planned(
        &fx,
        true,
        json!([{ "kind": "remove", "file": "song.k7/a.txt", "was": text_hash("disk") }]),
    )
    .unwrap();
    assert_eq!(fresh(&p, "t1", "t1"), Ok(()));
    let p = planned(
        &fx,
        true,
        json!([{ "kind": "remove", "file": "song.k7/a.txt", "was": null }]),
    )
    .unwrap();
    assert!(fresh(&p, "t1", "t1").is_err());
    let copy = |replace: bool| {
        planned(&fx, true, json!([{ "kind": "copy", "from": "song.k7/b.bin.txt", "file": "song.k7/a.txt", "replace": replace }])).unwrap()
    };
    assert_eq!(
        fresh(&copy(false), "t1", "t1"),
        Err(Refusal::Stale {
            files: vec!["song.k7/a.txt".into()],
            project: false
        })
    );
    assert_eq!(fresh(&copy(true), "t1", "t1"), Ok(()));
}
