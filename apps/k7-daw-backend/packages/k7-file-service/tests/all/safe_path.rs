//! `safePath`'s two gates (1021, 1022, 1024): the sentences, a link that leads out, a fake disk spelling Windows
//! roots and links on any host, and the hostile table, each answered fast.

use crate::support;

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use k7_file_service::confine::{Disk, confine_to};
use k7_file_service::libraries::Home;
use k7_file_service::space::{Declined, MAX_REL, PathError, Space};
use support::{Fake, Scratch, can_link, link};

const BOUND: Duration = Duration::from_secs(if cfg!(target_env = "musl") { 60 } else { 15 });

fn home() -> Home {
    Home {
        studio: "/studio".into(),
        libraries: "/libs".into(),
    }
}

#[test]
fn the_workspace_gate_words_its_refusals_as_the_typescript_does() {
    let s = Scratch::new("gate");
    let ws = s.dir("ws");
    s.file("ws/a/b.txt", "x");
    let (h, disk) = (home(), Disk);
    let space = Space::open(&h, &ws);
    assert_eq!(
        space.safe_path("a/b.txt", &disk),
        Ok(format!("{ws}/a/b.txt"))
    );
    assert_eq!(space.safe_path(".", &disk), Ok(ws.clone()));
    assert_eq!(space.safe_path("", &disk), Ok(ws.clone()));
    assert_eq!(
        space.safe_path("a\\b.txt", &disk),
        Ok(format!("{ws}/a/b.txt"))
    );
    let escapes = |rel: &str| PathError::Escapes {
        what: "workspace",
        rel: rel.into(),
    };
    for rel in [
        "../x",
        "a/../../x",
        "/etc/passwd",
        "C:/x",
        "\\\\server\\x",
        "a/b\0",
    ] {
        assert_eq!(space.safe_path(rel, &disk), Err(escapes(rel)), "{rel:?}");
    }
    // `C:x` has no root: it is a file of that name inside the workspace.
    assert_eq!(space.safe_path("C:x", &disk), Ok(format!("{ws}/C:x")));
    assert_eq!(
        escapes("../x").to_string(),
        "path escapes the workspace: ../x"
    );
}

#[test]
fn a_library_path_is_confined_to_that_library_and_nothing_else_below_the_folder_is_a_path() {
    let s = Scratch::new("libs");
    let ws = s.dir("studio/workspace/songs");
    let h = s.home();
    s.file("libraries/k7-core/modules/a.k7x", "x");
    s.dir("studio/library");
    let space = Space::open(&h, &ws);
    let disk = Disk;
    let lib = format!("{}/k7-core", h.libraries);
    assert_eq!(
        space.safe_path("_libraries/k7-core/modules/a.k7x", &disk),
        Ok(format!("{lib}/modules/a.k7x"))
    );
    assert_eq!(
        space.safe_path("/_libraries/k7-core", &disk),
        Ok(lib.clone())
    );
    assert_eq!(
        space.safe_path("_libraries\\k7-core\\modules", &disk),
        Ok(format!("{lib}/modules"))
    );
    assert_eq!(
        space.safe_path("_libraries", &disk),
        Ok(h.libraries.clone())
    );
    assert_eq!(
        space.safe_path("_libraries/account/x", &disk),
        Ok(format!("{}/library/x", h.studio))
    );
    let none = |rel: &str| PathError::NoLibrary { rel: rel.into() };
    for rel in [
        "_libraries/.sync-relay/x",
        "_libraries/../../x",
        "_libraries/engine/x",
        "_libraries/Constructor/x",
    ] {
        assert_eq!(space.safe_path(rel, &disk), Err(none(rel)), "{rel}");
    }
    let out = |rel: &str| PathError::Escapes {
        what: "libraries",
        rel: rel.into(),
    };
    assert_eq!(
        space.safe_path("_libraries/k7-core/../other/x", &disk),
        Err(out("_libraries/k7-core/../other/x"))
    );
    assert_eq!(
        space.safe_path("/_libraries/x/../..", &disk),
        Err(out("/_libraries/x/../.."))
    );
    assert_eq!(
        none("_libraries/x").to_string(),
        "names no library: _libraries/x"
    );
    assert_eq!(
        out("_libraries/k7-core/..").to_string(),
        "path escapes the libraries: _libraries/k7-core/.."
    );
}

#[test]
fn a_link_that_leads_out_is_refused_at_the_leaf_and_in_the_middle() {
    let s = Scratch::new("links");
    if !can_link(&s) {
        return;
    }
    let ws = s.dir("ws");
    let outside = s.dir("outside");
    s.file("outside/secret.txt", "x");
    s.dir("ws/inside");
    link(&outside, &s.path("ws/leaf")).unwrap();
    link(&outside, &s.path("ws/inside/mid")).unwrap();
    link(&s.path("nowhere"), &s.path("ws/dangling")).unwrap();
    link(&s.path("ws/inside"), &s.path("ws/fine")).unwrap();
    let (h, disk) = (home(), Disk);
    let space = Space::open(&h, &ws);
    for rel in [
        "leaf",
        "leaf/secret.txt",
        "inside/mid/secret.txt",
        "dangling",
        "dangling/x",
    ] {
        assert!(
            matches!(space.safe_path(rel, &disk), Err(PathError::Escapes { .. })),
            "{rel}"
        );
    }
    assert!(space.safe_path("fine", &disk).is_ok());
    assert!(space.safe_path("leaf/not-yet", &disk).is_err());
}

#[test]
fn a_fake_disk_spells_links_and_windows_roots_on_any_host() {
    let probe = Fake::with(
        &["/ws", "/ws/in", "/ws/out", "/out", "/out/f"],
        &[("/ws/out", "/out"), ("/ws", "/ws")],
    );
    let at = |p: &str| confine_to(&["/ws"], p, &probe);
    assert_eq!(
        at("/ws/in/x"),
        Some("/ws/in/x".into()),
        "a leaf that is not there yet, below a real folder"
    );
    assert_eq!(at("/ws/out"), None, "a link out at the leaf");
    assert_eq!(at("/ws/out/f"), None, "a link out in the middle");
    assert_eq!(
        at("/ws/dangling"),
        Some("/ws/dangling".into()),
        "nothing there: no link to follow"
    );
    // A dangling link is present, and its real path cannot be read.
    let dangling = Fake::with(&["/ws", "/ws/d"], &[("/ws/d", "")]);
    assert_eq!(confine_to(&["/ws"], "/ws/d", &dangling), None);
    // A root that is itself a link lands inside another allowed root.
    let linked_root = Fake::with(
        &["/r", "/r/x"],
        &[("/r", "/elsewhere"), ("/r/x", "/elsewhere/x")],
    );
    assert_eq!(
        confine_to(&["/r"], "/r/x", &linked_root),
        Some("/r/x".into())
    );
    // A root that does not exist holds nothing, so nothing leads out of it.
    assert_eq!(
        confine_to(&["/gone"], "/gone/a/b", &Fake::default()),
        Some("/gone/a/b".into())
    );
    // A link inside this root may land inside another allowed one.
    let two = Fake::with(&["/a", "/b", "/a/l"], &[("/a/l", "/b/z")]);
    assert_eq!(confine_to(&["/a"], "/a/l", &two), None);
    assert_eq!(confine_to(&["/a", "/b"], "/a/l", &two), Some("/a/l".into()));
    // A Windows root spelt `C:/` with a path in another case, nothing on the fake disk but the root.
    let win = Fake::with(&["C:/Work"], &[]);
    assert_eq!(
        confine_to(&["C:/Work"], "c:/work/songs/x", &win),
        Some("c:/work/songs/x".into())
    );
    assert_eq!(confine_to(&["C:/Work"], "C:\\Work\\..\\Other", &win), None);
    assert_eq!(confine_to(&["C:/Work"], "D:/Work/x", &win), None);
}

#[test]
fn the_hostile_table_is_answered_fast() {
    let s = Scratch::new("hostile");
    let ws = s.dir("ws");
    let (h, disk) = (home(), Disk);
    let space = Space::open(&h, &ws);
    let t0 = Instant::now();
    // A rel past the limit is declined before the disk is asked anything.
    let probe = Fake::default();
    let long = "a/".repeat(MAX_REL / 2 + 1);
    assert_eq!(
        space.safe_path(&long, &probe),
        Err(PathError::Declined(Declined::LongPath))
    );
    assert_eq!(
        probe.asked.load(Ordering::Relaxed),
        0,
        "no lstat for a long path"
    );
    // 8,000 segments of `..` climb out of the workspace.
    let up = "..".to_string() + &"/..".repeat(5_000);
    assert!(matches!(
        space.safe_path(&up, &disk),
        Err(PathError::Escapes { .. })
    ));
    assert_eq!(
        space.safe_path(&"../".repeat(8_000), &disk),
        Err(PathError::Declined(Declined::LongPath))
    );
    // 8,000 segments of `a`, none on the disk: one lstat a segment at most.
    let deep = "a/".repeat(8_000);
    let counting = Fake::with(&[&ws], &[]);
    assert!(space.safe_path(&deep, &counting).is_ok());
    assert!(
        counting.asked.load(Ordering::Relaxed) <= 8_002,
        "{}",
        counting.asked.load(Ordering::Relaxed)
    );
    assert!(space.safe_path(&deep, &disk).is_ok());
    assert!(t0.elapsed() < BOUND, "{:?}", t0.elapsed());
}

#[test]
fn a_nul_anywhere_is_refused_and_a_path_of_the_wrong_kind_never_panics() {
    let s = Scratch::new("nul");
    let ws = s.dir("ws");
    let (h, disk) = (home(), Disk);
    let space = Space::open(&h, &ws);
    for rel in ["\0", "a\0b", "_libraries/k7-core\0", "a/\0/b", "é\0"] {
        assert!(space.safe_path(rel, &disk).is_err(), "{rel:?}");
    }
}
