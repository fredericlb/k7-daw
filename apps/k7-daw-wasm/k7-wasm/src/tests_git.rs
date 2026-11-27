// tests_git.rs — the git.* and media.* operations through the bytes the ABI carries: the shape of each request and its answer.

use crate::ops::{ERROR, OK};
use crate::*;
use serde_json::{Value, json};

fn run(request: &Value) -> (u32, Value) {
    let mut out = Vec::new();
    let status = Session::new().handle(request.to_string().as_bytes(), &mut out);
    (status, serde_json::from_slice(&out).unwrap())
}

fn ok(request: Value) -> Value {
    let (status, v) = run(&request);
    assert_eq!(status, OK, "{v}");
    v
}

#[test]
fn the_bar_offers_what_the_state_allows() {
    let state = json!({"mode": "connected", "branch": "main", "remote": "git@h:o/r.git", "ahead": 1, "behind": 2});
    let v = ok(json!({"op": "git.bar", "args": {"state": state}}));
    assert_eq!(v["push"], false);
    assert_eq!(v["pushBlocked"], "pullFirst");
    assert_eq!(v["toPull"], 2);
    assert_eq!(ok(json!({"op": "git.bar", "args": {"state": null}}))["show"], false);
}

#[test]
fn the_menu_names_the_remote() {
    let state = json!({"mode": "connected", "remote": "git@github.com:o/r.git"});
    let v = ok(json!({"op": "git.menu", "args": {"state": state}}));
    assert_eq!(v, json!({"connected": true, "remote": "github.com/o/r"}));
}

#[test]
fn a_save_leaves_the_too_large_files_out() {
    let entries = json!([
        {"path": "a.k7x", "change": "modified", "size": 1},
        {"path": "big.wav", "change": "added", "size": 3000000000u64, "tooLarge": true},
    ]);
    let v = ok(json!({"op": "git.commitView", "args": {"entries": entries}}));
    assert_eq!(v["sent"][0]["path"], "a.k7x");
    assert_eq!(v["leftOut"][0]["path"], "big.wav");
    assert_eq!(v["lfs"], 0);
}

#[test]
fn an_address_says_its_host_and_its_name() {
    let host = ok(json!({"op": "git.sshHost", "args": {"url": "ssh://git@h.org:2222/me/r.git"}}));
    assert_eq!(host, json!({"host": "h.org", "port": 2222}));
    assert_eq!(
        ok(json!({"op": "git.sshHost", "args": {"url": "/srv/r.git"}})),
        Value::Null
    );
    assert_eq!(
        ok(json!({"op": "git.isWebAddress", "args": {"url": "HTTPS://h/r"}})),
        true
    );
    assert_eq!(
        ok(json!({"op": "git.nameFromUrl", "args": {"url": "git@h:me/songs.git"}})),
        "songs"
    );
    assert_eq!(
        ok(json!({"op": "git.remoteName", "args": {"url": "git@h:me/songs.git"}})),
        "h/me/songs"
    );
    let taken = json!({"name": "Band", "taken": ["band"]});
    assert_eq!(ok(json!({"op": "git.cloneName", "args": taken})), "Band 2");
    assert_eq!(
        ok(json!({"op": "git.shortVersion", "args": {"version": "17.0.2+gitea"}})),
        "17.0.2"
    );
    assert_eq!(
        ok(json!({"op": "git.sshPort", "args": {"typed": "2222"}})),
        json!({"kind": "port", "port": 2222})
    );
    assert_eq!(
        ok(json!({"op": "git.sshPort", "args": {"typed": ""}})),
        json!({"kind": "default"})
    );
}

#[test]
fn a_merge_offers_and_counts() {
    let c =
        |id: &str, kind: &str, both: bool| json!({"id": id, "path": "p", "kind": kind, "both": both, "choice": null});
    let v = ok(json!({"op": "git.offered", "args": {"conflict": c("a", "text", true)}}));
    assert_eq!(v, json!(["mine", "theirs", "both"]));
    let args = json!({"conflicts": [c("a", "text", false), c("b", "binary", false)], "chosen": ["a"]});
    assert_eq!(ok(json!({"op": "git.remaining", "args": args})), 1);
    assert_eq!(ok(json!({"op": "git.canFinish", "args": args})), false);
}

#[test]
fn the_picker_arranges_and_dates() {
    let repo =
        |n: &str, at: &str, k7: bool| json!({"fullName": n, "sshUrl": "u", "private": true, "pushedAt": at, "k7": k7});
    let repos = json!([
        repo("o/a", "2026-01-01T00:00:00Z", false),
        repo("o/b", "2026-02-01T00:00:00Z", true)
    ]);
    let v = ok(json!({"op": "git.arrange", "args": {"repos": repos, "filter": ""}}));
    assert_eq!(v["k7"][0]["fullName"], "o/b");
    assert_eq!(v["others"][0]["fullName"], "o/a");
    let now = 1_780_315_200_000u64;
    let v = ok(json!({"op": "git.pushedAgo", "args": {"iso": "2026-06-01T11:15:00Z", "now": now}}));
    assert_eq!(v, json!({"unit": "minutes", "n": 45}));
}

#[test]
fn the_roam_section_lists_what_this_device_lacks() {
    let entry = |id: &str, remote: Value| json!({"id": id, "name": id, "remote": remote, "addedAt": 1});
    let state = json!({
        "answer": {"available": true, "entries": [entry("a", json!("git@h:o/a.git")), entry("b", Value::Null)], "here": []},
        "cloned": [],
    });
    let v = ok(json!({"op": "git.roamOffered", "args": {"state": state}}));
    assert_eq!(v.as_array().map(Vec::len), Some(1));
}

#[test]
fn a_save_is_dated_on_the_callers_clock() {
    let v = ok(json!({"op": "git.saveWhen", "args": {"epochMs": 1_795_392_840_000i64, "offsetMin": 60}}));
    assert_eq!(v, "23/11/2026 01:14");
}

#[test]
fn a_bad_request_is_an_error_not_a_trap() {
    let (status, v) = run(&json!({"op": "git.bar", "args": {"state": {"mode": "nowhere"}}}));
    assert_eq!(status, ERROR);
    assert_eq!(v["code"], "bad_request");
}

#[test]
fn the_trim_window_finds_the_sound() {
    let loud = 10f64.powf(-6.0 / 20.0);
    let env = [0.0, 0.0, 0.0, loud, loud, loud, loud, 0.0, 0.0, 0.0];
    let args = json!({"env": env, "seconds": 10.0, "gateDb": -40.0});
    assert_eq!(
        ok(json!({"op": "media.trimWindow", "args": args})),
        json!({"from": 2.0, "to": 8.0})
    );
    let silent = json!({"env": [0.0, 0.0], "seconds": 2.0, "gateDb": -40.0, "within": null});
    assert_eq!(ok(json!({"op": "media.trimWindow", "args": silent})), Value::Null);
}

#[test]
fn the_spectrum_strip_is_four_bytes_a_column() {
    let args = json!({"columns": [[0.5, 0.0, 0.0], null], "plain": [148, 163, 184]});
    let v = ok(json!({"op": "media.spectrumStrip", "args": args}));
    // 255,45,10,255 then 148,163,184,255.
    assert_eq!(v, "/y0K/5SjuP8=");
}

#[test]
fn a_webm_head_without_a_duration_gets_one() {
    // An EBML header, then a Segment of unknown size holding an Info with only its timecode scale.
    let head: Vec<u8> = [
        &[0x1a, 0x45, 0xdf, 0xa3, 0x84, 0x42, 0x82, 0x81, 0x77][..],
        &[0x18, 0x53, 0x80, 0x67, 0x01, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
        &[0x15, 0x49, 0xa9, 0x66, 0x85, 0x2a, 0xd7, 0xb1, 0x81, 0x01],
    ]
    .concat();
    use base64::{Engine, engine::general_purpose::STANDARD};
    let args = json!({"head": STANDARD.encode(&head), "durationMs": 1000.0});
    let v = ok(json!({"op": "media.webmPatch", "args": args}));
    assert_eq!(v["consumed"], head.len());
    let bytes = STANDARD.decode(v["bytes"].as_str().unwrap()).unwrap();
    assert_eq!(bytes.len(), head.len() + 11);
    assert_eq!(
        ok(json!({"op": "media.webmPatch", "args": {"head": "AQID", "durationMs": 1.0}})),
        Value::Null
    );
}
