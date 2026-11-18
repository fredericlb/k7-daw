//! The tree (1050 to 1055): the literal's key order, folders first then bytes, what is hidden, a project's flags,
//! the limits (1052) and the one folder that cannot be listed (1054).

use crate::support;

use k7_file_service::space::{Declined, Space};
use k7_file_service::tree::{Limits, Tree, tree};
use serde_json::{Value, json};
use support::{Scratch, can_link, link};

fn walk(s: &Scratch, limits: &Limits) -> Result<Tree, Declined> {
    let h = s.home(false);
    tree(&Space::open(&h, &s.path("ws")), limits)
}

fn ok(s: &Scratch) -> Tree {
    walk(s, &Limits::default()).unwrap()
}

#[test]
fn writes_the_literal_in_its_key_order_folders_first_then_bytes() {
    let s = Scratch::new("shape");
    s.file("ws/b.txt", "");
    s.file("ws/a.WAV", "");
    s.file("ws/fake.k7", "not a folder");
    s.file("ws/Z/z.txt", "");
    s.file("ws/a-dir/x.png", "");
    s.file("ws/song.k7/main.k7x", "");
    s.dir("ws/empty");
    let t = ok(&s);
    let expected = json!([
        {"name":"Z","path":"Z","dir":true,"children":[{"name":"z.txt","path":"Z/z.txt","dir":false,"kind":"text"}]},
        {"name":"a-dir","path":"a-dir","dir":true,"children":[{"name":"x.png","path":"a-dir/x.png","dir":false,"kind":"binary"}]},
        {"name":"empty","path":"empty","dir":true,"children":[]},
        {"name":"song.k7","path":"song.k7","dir":true,"project":true,"app":"k7","children":[{"name":"main.k7x","path":"song.k7/main.k7x","dir":false,"kind":"text"}]},
        {"name":"a.WAV","path":"a.WAV","dir":false,"kind":"binary"},
        {"name":"b.txt","path":"b.txt","dir":false,"kind":"text"},
        {"name":"fake.k7","path":"fake.k7","dir":false,"kind":"text"}
    ]);
    // The text itself is compared, so the keys' order is held and not only their values.
    assert_eq!(t.json, serde_json::to_string(&expected).unwrap());
    assert_eq!((t.entries, t.unlistable, t.depth), (10, 0, 1));
}

#[test]
fn hides_what_skip_hides_at_any_depth() {
    let s = Scratch::new("hidden");
    s.file("ws/keep.txt", "");
    for hidden in [
        ".git/x",
        "node_modules/x",
        "_recycle_bin/x",
        "_history/x",
        "side.wav.cache",
        ".env",
        "sub/_history/j",
        "sub/.hid",
    ] {
        s.file(&format!("ws/{hidden}"), "");
    }
    s.file("ws/sub/ok.txt", "");
    let v: Value = serde_json::from_str(&ok(&s).json).unwrap();
    let names: Vec<&str> = v
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["sub", "keep.txt"]);
    let sub: Vec<&str> = v[0]["children"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["name"].as_str().unwrap())
        .collect();
    assert_eq!(sub, ["ok.txt"]);
}

#[test]
fn a_link_is_a_file_that_is_not_followed() {
    let s = Scratch::new("link");
    if !can_link(&s) {
        return;
    }
    s.file("ws/real/x.txt", "");
    s.dir("elsewhere");
    link(&s.path("elsewhere"), &s.path("ws/alias")).unwrap();
    let v: Value = serde_json::from_str(&ok(&s).json).unwrap();
    assert_eq!(
        v[1],
        json!({"name":"alias","path":"alias","dir":false,"kind":"text"})
    );
}

#[test]
fn counts_every_entry_a_listing_yields_hidden_ones_too() {
    let s = Scratch::new("seen");
    s.file("ws/visible.txt", "");
    for i in 0..50 {
        s.file(&format!("ws/.h{i}"), "");
    }
    let small = Limits {
        max_seen: 20,
        ..Limits::default()
    };
    assert_eq!(walk(&s, &small).unwrap_err(), Declined::TooMany);
    assert!(
        walk(
            &s,
            &Limits {
                max_seen: 51,
                ..Limits::default()
            }
        )
        .is_ok()
    );
    // A folder skipped by name is not entered, so what is inside it is not counted.
    let s = Scratch::new("seen-skipped");
    s.file("ws/a.txt", "");
    for i in 0..50 {
        s.file(&format!("ws/node_modules/f{i}"), "");
    }
    assert!(
        walk(
            &s,
            &Limits {
                max_seen: 5,
                ..Limits::default()
            }
        )
        .is_ok()
    );
}

#[test]
fn weighs_the_text_it_writes_and_the_folders_it_nests() {
    let s = Scratch::new("weigh");
    s.file("ws/a/b/c/d.txt", "");
    assert_eq!(
        walk(
            &s,
            &Limits {
                max_bytes: 40,
                ..Limits::default()
            }
        )
        .unwrap_err(),
        Declined::TooLarge
    );
    assert_eq!(
        walk(
            &s,
            &Limits {
                max_depth: 2,
                ..Limits::default()
            }
        )
        .unwrap_err(),
        Declined::TooDeep
    );
    let t = walk(
        &s,
        &Limits {
            max_depth: 3,
            ..Limits::default()
        },
    )
    .unwrap();
    assert_eq!(t.depth, 3);
}

#[cfg(unix)]
#[test]
fn a_name_with_a_backslash_is_spelt_with_a_slash_and_its_folder_is_not_listed() {
    let s = Scratch::new("backslash");
    s.dir("ws/sub");
    std::fs::write(s.raw("ws/a\\b.txt"), "").unwrap();
    std::fs::create_dir(s.raw("ws/d\\e")).unwrap();
    std::fs::write(s.raw("ws/sub/x\\y.txt"), "").unwrap();
    let t = ok(&s);
    let v: Value = serde_json::from_str(&t.json).unwrap();
    // The folder `d\e` is spelt `d/e`, which is not on the disk: it costs that folder (1053, 1054).
    assert_eq!(
        v[0],
        json!({"name":"d\\e","path":"d/e","dir":true,"children":[]})
    );
    assert_eq!(t.unlistable, 1);
    assert_eq!(
        v[2],
        json!({"name":"a\\b.txt","path":"a/b.txt","dir":false,"kind":"text"})
    );
    assert_eq!(v[1]["children"][0]["path"], json!("sub/x/y.txt"));
}

#[cfg(unix)]
#[test]
fn a_name_of_climbing_segments_never_leaves_the_workspace() {
    let s = Scratch::new("climb");
    s.file("outside/secret.txt", "");
    s.dir("ws");
    std::fs::create_dir(s.raw("ws/a\\..\\..\\outside")).unwrap();
    let t = ok(&s);
    assert_eq!(t.unlistable, 1);
    assert!(!t.json.contains("secret.txt"));
}

#[cfg(unix)]
#[test]
fn a_backslash_name_never_lists_what_a_link_leads_out_to() {
    let s = Scratch::new("backslash-link");
    s.file("other-account/x/secret.txt", "");
    s.dir("ws");
    link(&s.path("other-account"), &s.path("ws/lnk")).unwrap();
    // On POSIX this is one name; spelt `lnk/x` it reaches the far side of the link.
    std::fs::create_dir(s.raw("ws/lnk\\x")).unwrap();
    let t = ok(&s);
    assert!(!t.json.contains("secret.txt"), "{}", t.json);
    assert_eq!(t.unlistable, 1);
}

#[test]
fn a_folder_whose_names_alone_pass_the_byte_limit_is_declined() {
    let s = Scratch::new("names-bytes");
    for i in 0..50 {
        s.file(&format!("ws/{i:03}{}", "n".repeat(100)), "");
    }
    let r = walk(
        &s,
        &Limits {
            max_bytes: 2_000,
            ..Limits::default()
        },
    );
    assert_eq!(r.unwrap_err(), Declined::TooLarge);
}

#[cfg(unix)]
#[test]
fn a_name_that_is_not_utf8_is_lossy_and_never_a_project() {
    use std::os::unix::ffi::OsStrExt;
    let s = Scratch::new("lossy");
    s.dir("ws");
    let name = std::ffi::OsStr::from_bytes(b"bad\xff.k7");
    // APFS refuses a name that is not UTF-8; the Linux runs hold this case.
    if std::fs::create_dir(std::path::Path::new(&s.path("ws")).join(name)).is_err() {
        return;
    }
    let t = ok(&s);
    let v: Value = serde_json::from_str(&t.json).unwrap();
    assert_eq!(v[0]["name"], json!("bad\u{fffd}.k7"));
    assert!(v[0].get("project").is_none(), "{v}");
    assert_eq!(t.unlistable, 1);
}

#[cfg(unix)]
#[test]
fn a_folder_that_cannot_be_read_costs_that_folder_only() {
    use std::os::unix::fs::PermissionsExt;
    if support::is_root() {
        return;
    }
    let s = Scratch::new("chmod");
    s.file("ws/open/x.txt", "");
    s.file("ws/locked/y.txt", "");
    s.file("ws/after.txt", "");
    let locked = s.path("ws/locked");
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
    let t = ok(&s);
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
    let v: Value = serde_json::from_str(&t.json).unwrap();
    assert_eq!(t.unlistable, 1);
    assert_eq!(
        v[0],
        json!({"name":"locked","path":"locked","dir":true,"children":[]})
    );
    assert_eq!(v[1]["children"][0]["path"], json!("open/x.txt"));
    assert_eq!(v[2]["name"], json!("after.txt"));
}

#[test]
fn a_root_that_is_gone_is_one_unlistable_folder() {
    let s = Scratch::new("gone");
    let t = ok(&s);
    assert_eq!((t.json.as_str(), t.unlistable, t.entries), ("[]", 1, 0));
}

#[test]
fn reports_the_newest_folder_it_walked() {
    let s = Scratch::new("newest");
    s.file("ws/a/b.txt", "");
    let before = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
        - 5_000_000_000;
    let t = ok(&s);
    assert!(
        t.newest_folder_ns > before,
        "{} <= {before}",
        t.newest_folder_ns
    );
}
