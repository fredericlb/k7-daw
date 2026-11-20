//! The workspace's listings (1410 to 1413): a playlist read through the k7x reader, a reference track's extension and
//! size, the walk below the folder (a link is neither folder nor file), and the limits every entry counts against.

use crate::support;

use k7_file_service::listings::{Listed, Listing, Remote, list, list_gated};
use k7_file_service::space::{Declined, Space};
use k7_file_service::tree::Limits;
use serde_json::{Value, json};
use support::{Scratch, can_link, link, link_file};

fn run(s: &Scratch, which: Listing, limits: &Limits) -> Result<Listed, Declined> {
    let h = s.home();
    list(
        &Space::open(&h, &s.path("ws")),
        which,
        &Remote::default(),
        limits,
    )
}

fn rows(s: &Scratch, which: Listing, key: &str) -> Vec<Value> {
    let l = run(s, which, &Limits::default()).unwrap();
    serde_json::from_str::<Value>(&l.json).unwrap()[key]
        .as_array()
        .unwrap()
        .clone()
}

fn playlists(s: &Scratch) -> Vec<Value> {
    rows(s, Listing::Playlists, "playlists")
}

#[test]
fn a_playlist_is_its_title_trimmed_and_the_lines_that_name_a_song() {
    let s = Scratch::new("pl-shape");
    s.file(
        "ws/_playlists/a.k7x",
        "title(\"  Set \u{a0}\")\nitems([\"one\", song(\"two\").variant(\"v\"), variant(\"x\"), \"  \", 12, song(\"  \")])\n",
    );
    s.file("ws/_playlists/untitled.k7x", "items([\"x\"])\n");
    s.file("ws/_playlists/blank.k7x", "title(\"  \")\n");
    s.file("ws/_playlists/broken.k7x", "title(");
    s.file("ws/_playlists/n.k7x", "title(5)\nitems(\"no\")\n");
    // 1410: the key order, the absent title, the failed parse as `ok: false` with no lines.
    assert_eq!(
        run(&s, Listing::Playlists, &Limits::default())
            .unwrap()
            .json,
        concat!(
            r#"{"playlists":["#,
            r#"{"path":"_playlists/a.k7x","name":"a","title":"Set","count":2,"ok":true},"#,
            r#"{"path":"_playlists/blank.k7x","name":"blank","count":0,"ok":true},"#,
            r#"{"path":"_playlists/broken.k7x","name":"broken","count":0,"ok":false},"#,
            r#"{"path":"_playlists/n.k7x","name":"n","count":0,"ok":true},"#,
            r#"{"path":"_playlists/untitled.k7x","name":"untitled","count":1,"ok":true}]}"#
        )
    );
}

#[test]
fn a_bom_is_kept_and_an_invalid_utf8_byte_is_replaced() {
    let s = Scratch::new("pl-bytes");
    s.file(
        "ws/_playlists/bom.k7x",
        "\u{feff}title(\"Bom\")\nitems([\"a\"])\n",
    );
    std::fs::write(
        s.path("ws/_playlists/bad.k7x"),
        b"title(\"Caf\xe9\")\nitems([\"a\"])\n",
    )
    .unwrap();
    let got = playlists(&s);
    assert_eq!(got[0]["title"], "Caf\u{fffd}");
    assert_eq!(
        got[1],
        json!({"path": "_playlists/bom.k7x", "name": "bom", "title": "Bom", "count": 1, "ok": true})
    );
}

#[test]
fn a_lone_surrogate_escape_is_the_typescripts_and_a_large_text_is_declined() {
    let s = Scratch::new("pl-declines");
    s.file("ws/_playlists/lone.k7x", "title(\"\\uD800\")\n");
    assert_eq!(
        run(&s, Listing::Playlists, &Limits::default()).unwrap_err(),
        Declined::LossyText
    );
    std::fs::remove_file(s.path("ws/_playlists/lone.k7x")).unwrap();
    s.file("ws/_playlists/big.k7x", &"x".repeat(101));
    let small = Limits {
        max_text: 100,
        ..Limits::default()
    };
    assert_eq!(
        run(&s, Listing::Playlists, &small).unwrap_err(),
        Declined::TooLarge
    );
    // At the limit it reads.
    let exact = Limits {
        max_text: 101,
        ..Limits::default()
    };
    assert!(run(&s, Listing::Playlists, &exact).is_ok());
}

#[test]
fn the_text_of_a_playlist_is_asked_for_before_it_is_read() {
    let s = Scratch::new("pl-admit");
    s.file("ws/_playlists/a.k7x", "title(\"A\")\n");
    let h = s.home();
    let space = Space::open(&h, &s.path("ws"));
    let asked = std::sync::Mutex::new(Vec::new());
    let r = list_gated(
        &space,
        Listing::Playlists,
        &Remote::default(),
        &Limits::default(),
        &|n| {
            asked.lock().unwrap().push(n);
            false
        },
    );
    assert_eq!(
        (r.unwrap_err(), asked.into_inner().unwrap()),
        (Declined::Busy, vec![11])
    );
}

#[test]
fn names_sort_by_code_point_and_a_dot_file_is_listed() {
    let s = Scratch::new("pl-order");
    for name in ["\u{1F600}", "\u{ff5e}", "B", "a", ".dot"] {
        s.file(&format!("ws/_playlists/{name}.k7x"), "items([])\n");
    }
    s.file("ws/_playlists/.hid/in.k7x", "items([])\n");
    let paths: Vec<String> = playlists(&s)
        .iter()
        .map(|r| r["path"].as_str().unwrap().to_string())
        .collect();
    // U+FF5E sorts before U+1F600 by code point, after it by UTF-16 unit.
    assert_eq!(
        paths,
        [
            "_playlists/.dot.k7x",
            "_playlists/.hid/in.k7x",
            "_playlists/B.k7x",
            "_playlists/a.k7x",
            "_playlists/\u{ff5e}.k7x",
            "_playlists/\u{1F600}.k7x"
        ]
    );
}

#[test]
fn a_link_below_the_folder_is_neither() {
    let s = Scratch::new("pl-link");
    if !can_link(&s) {
        return;
    }
    s.file("ws/_playlists/real/a.k7x", "items([])\n");
    s.file("outside/x.k7x", "items([])\n");
    link(&s.path("outside"), &s.path("ws/_playlists/alias")).unwrap();
    link_file(
        &s.path("ws/_playlists/real/a.k7x"),
        &s.path("ws/_playlists/l.k7x"),
    )
    .unwrap();
    let paths: Vec<Value> = playlists(&s).iter().map(|r| r["path"].clone()).collect();
    assert_eq!(paths, [json!("_playlists/real/a.k7x")]);
}

#[test]
fn the_folder_asked_is_followed_when_it_is_a_link_and_named_inside() {
    let s = Scratch::new("pl-root-link");
    if !can_link(&s) {
        return;
    }
    s.file("outside/x.k7x", "items([\"a\"])\n");
    s.dir("ws");
    link(&s.path("outside"), &s.path("ws/_playlists")).unwrap();
    let got = playlists(&s);
    assert_eq!(got.len(), 1);
    assert_eq!(got[0]["path"], "_playlists/x.k7x");
}

#[test]
fn a_folder_spelt_otherwise_is_listed_as_joined() {
    let s = Scratch::new("pl-case");
    s.file("ws/_Playlists/a.k7x", "items([])\n");
    // 1413: where the disk folds case the folder is found by the TypeScript's join, and named so.
    let folds = std::fs::metadata(s.path("ws/_playlists")).is_ok();
    let got = playlists(&s);
    if folds {
        assert_eq!(got[0]["path"], "_playlists/a.k7x");
    } else {
        assert!(got.is_empty());
    }
}

#[test]
fn a_backslash_in_a_name_is_spelt_with_a_slash_and_reads_nothing() {
    if cfg!(windows) {
        return;
    }
    let s = Scratch::new("pl-backslash");
    s.file("ws/_playlists/ok.k7x", "items([])\n");
    std::fs::write(s.raw("ws/_playlists/s\\t.k7x"), "items([\"a\"])\n").unwrap();
    let got = playlists(&s);
    assert_eq!(
        got[1],
        json!({"path": "_playlists/s/t.k7x", "name": "t", "count": 0, "ok": false})
    );
}

#[test]
fn a_reference_is_its_extension_and_its_size() {
    let s = Scratch::new("refs");
    s.file("ws/_references/a.WAV", "RIFF");
    s.file("ws/_references/x/y.mp3", "12345678");
    s.file("ws/_references/t.txt", "skipped");
    s.file("ws/_references/.wav", "dot only");
    s.file("ws/_references/a.tar.flac", "t");
    s.file("ws/_references/noext", "n");
    s.file("ws/_references/.hidden.opus", "h");
    let got = rows(&s, Listing::References, "references");
    // 1412: the name is the base name before its last dot; the size is exact.
    assert_eq!(
        serde_json::to_string(&got).unwrap(),
        concat!(
            r#"[{"path":"_references/.hidden.opus","name":".hidden","bytes":1},"#,
            r#"{"path":"_references/a.WAV","name":"a","bytes":4},"#,
            r#"{"path":"_references/a.tar.flac","name":"a.tar","bytes":1},"#,
            r#"{"path":"_references/x/y.mp3","name":"y","bytes":8}]"#
        )
    );
}

#[test]
fn a_link_among_the_references_is_no_track() {
    let s = Scratch::new("refs-link");
    if !can_link(&s) {
        return;
    }
    s.file("ws/_references/real.wav", "r");
    link_file(
        &s.path("ws/_references/real.wav"),
        &s.path("ws/_references/l.wav"),
    )
    .unwrap();
    assert_eq!(rows(&s, Listing::References, "references").len(), 1);
}

#[test]
fn every_entry_counts_against_the_walk_the_hidden_ones_too() {
    let s = Scratch::new("below-limits");
    s.file("ws/_references/a.wav", "");
    for i in 0..300 {
        s.file(&format!("ws/_references/.h{i}"), "");
    }
    let few = Limits {
        max_seen: 100,
        ..Limits::default()
    };
    assert_eq!(
        run(&s, Listing::References, &few).unwrap_err(),
        Declined::TooMany
    );
    s.dir(&format!("ws/_playlists/{}", "d/".repeat(5)));
    let shallow = Limits {
        max_depth: 3,
        ..Limits::default()
    };
    assert_eq!(
        run(&s, Listing::Playlists, &shallow).unwrap_err(),
        Declined::TooDeep
    );
}

#[test]
fn the_rows_are_held_to_the_byte_limit() {
    let s = Scratch::new("below-bytes");
    for i in 0..50 {
        s.file(&format!("ws/_references/track-{i}.wav"), "");
    }
    let small = Limits {
        max_bytes: 500,
        ..Limits::default()
    };
    assert_eq!(
        run(&s, Listing::References, &small).unwrap_err(),
        Declined::TooLarge
    );
}

#[test]
fn a_row_has_a_stamp_of_its_file_and_its_folder() {
    let s = Scratch::new("stamps");
    let a = s.file("ws/_playlists/n/a.k7x", "items([])\n");
    let l = run(&s, Listing::Playlists, &Limits::default()).unwrap();
    assert_eq!(l.rows.len(), 1);
    assert_eq!(l.rows[0].file.as_deref(), Some(a.as_str()));
    assert_eq!(l.rows[0].folder, s.path("ws/_playlists/n"));
    assert!(l.newest_folder_ns > 0);
}
