//! The marker's write and reads (2020 to 2025): where it lives, its bytes, the repeat write that changes nothing, and
//! the hostile files a person or a crash leaves in its folder.

use crate::support;

use std::time::{Duration, UNIX_EPOCH};

use k7_file_service::paths::dirname;
use k7_file_service::projects::ProjError;
use k7_file_service::projects::marker::{
    clear, marker_of, read_raw, reads_as_none, write, write_if_changed,
};
use k7_file_service::space::{Declined, MAX_REL, Space};
use support::ws::{Acct, touch};

fn root_of(a: &Acct) -> String {
    a.ws("songs")
}

#[test]
fn the_marker_lives_beside_the_workspaces() {
    for (root, marker) in [
        (
            "/h/studio/workspace/songs",
            "/h/studio/workspace/.active-projects/songs",
        ),
        (
            "/h/studio/workspace/songs/",
            "/h/studio/workspace/.active-projects/songs",
        ),
        (
            "C:/Work/studio/workspace/songs",
            "C:/Work/studio/workspace/.active-projects/songs",
        ),
        ("c:\\Work\\ws", "c:/Work/.active-projects/ws"),
        ("/a/b/../c/ws", "/a/c/.active-projects/ws"),
    ] {
        assert_eq!(marker_of(root), marker, "{root}");
    }
}

#[test]
fn a_drive_home_writes_the_marker_as_the_shim_does() {
    let a = Acct::new("pm-drive");
    let root = root_of(&a);
    assert!(!root.contains('\\') && !root.starts_with(r"\\?\"), "{root}");
    write(&root, "deep/a.k7").unwrap();
    let at = marker_of(&root);
    assert_eq!(at, format!("{}/.active-projects/songs", dirname(&root)));
    assert_eq!(std::fs::read(&at).unwrap(), b"deep/a.k7\n");
}

#[test]
fn a_write_is_the_rel_and_a_newline_and_none_is_empty() {
    let a = Acct::new("pm-bytes");
    let root = root_of(&a);
    write(&root, "a.k7").unwrap();
    assert_eq!(std::fs::read(marker_of(&root)).unwrap(), b"a.k7\n");
    clear(&root).unwrap();
    assert_eq!(std::fs::read(marker_of(&root)).unwrap(), b"");
    assert_eq!(read_raw(&root), Ok(None));
    // A marker that was never there is still made, empty: the TypeScript writes it too.
    let b = Acct::new("pm-none");
    let fresh = root_of(&b);
    assert_eq!(write_if_changed(&fresh, ""), Ok(true));
    assert!(std::path::Path::new(&marker_of(&fresh)).exists());
}

#[test]
fn what_is_written_is_what_the_next_open_reads() {
    let a = Acct::new("pm-roundtrip");
    let root = root_of(&a);
    a.project("songs", "deep/b.k7", "");
    write(&root, "deep/b.k7").unwrap();
    assert_eq!(read_raw(&root), Ok(Some(format!("{root}/deep/b.k7"))));
    let h = a.home();
    assert_eq!(
        Space::open(&h, &root).active(),
        Ok(Some(format!("{root}/deep/b.k7")))
    );
}

#[test]
fn a_repeat_write_leaves_the_file_and_its_mtime_alone() {
    let a = Acct::new("pm-repeat");
    let root = root_of(&a);
    assert_eq!(write_if_changed(&root, "a.k7"), Ok(true));
    let at = marker_of(&root);
    touch(&at, 1_000_000, 0);
    assert_eq!(write_if_changed(&root, "a.k7"), Ok(false));
    let mtime = std::fs::metadata(&at).unwrap().modified().unwrap();
    assert_eq!(mtime, UNIX_EPOCH + Duration::from_secs(1_000_000));
    assert_eq!(write_if_changed(&root, "b.k7"), Ok(true));
    assert_eq!(std::fs::read(&at).unwrap(), b"b.k7\n");
}

#[test]
fn no_temp_is_left_beside_the_marker_and_a_crashed_one_is_never_read() {
    let a = Acct::new("pm-temp");
    let root = root_of(&a);
    let dir = a.s.path("studio/workspace/.active-projects");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(format!("{dir}/.songs.123.deadbeef.tmp"), "crash.k7\n").unwrap();
    assert_eq!(read_raw(&root), Ok(None));
    write(&root, "a.k7").unwrap();
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, [".songs.123.deadbeef.tmp", "songs"]);
}

#[test]
fn a_marker_text_is_trimmed_as_javascript_trims_it() {
    let a = Acct::new("pm-trim");
    let root = root_of(&a);
    for (text, rel) in [
        ("a.k7", Some("a.k7")),
        ("  a.k7  \n\n", Some("a.k7")),
        ("a.k7\r\n", Some("a.k7")),
        ("\u{feff}a.k7\n", Some("a.k7")),
        ("\u{a0}a.k7\u{3000}", Some("a.k7")),
        ("\n", None),
        ("", None),
        ("   ", None),
    ] {
        a.marker("songs", text);
        let want = rel.map(|r| format!("{root}/{r}"));
        assert_eq!(read_raw(&root), Ok(want), "{text:?}");
    }
}

#[test]
fn a_marker_naming_a_path_outside_the_root_is_resolved_not_confined() {
    let a = Acct::new("pm-outside");
    let root = root_of(&a);
    a.marker("songs", "../other.k7\n");
    assert_eq!(
        read_raw(&root),
        Ok(Some(a.s.path("studio/workspace/other.k7")))
    );
    a.marker("songs", "/abs/x.k7\n");
    assert_eq!(read_raw(&root), Ok(Some("/abs/x.k7".into())));
}

#[test]
fn a_marker_past_a_mebibyte_or_a_rel_past_the_limit_declines() {
    let a = Acct::new("pm-big");
    let root = root_of(&a);
    a.marker("songs", &"x".repeat(2 << 20));
    assert_eq!(read_raw(&root), Err(Declined::LongPath));
    a.marker("songs", &"y".repeat(MAX_REL + 1));
    assert_eq!(read_raw(&root), Err(Declined::LongPath));
    let before = std::fs::read(marker_of(&root)).unwrap();
    assert_eq!(
        write(&root, &"y".repeat(MAX_REL + 1)),
        Err(ProjError::Declined(Declined::LongPath))
    );
    assert_eq!(std::fs::read(marker_of(&root)).unwrap(), before);
}

#[test]
fn bytes_that_are_no_utf8_read_as_the_replacement_character() {
    let a = Acct::new("pm-lossy");
    let root = root_of(&a);
    std::fs::create_dir_all(a.s.path("studio/workspace/.active-projects")).unwrap();
    std::fs::write(marker_of(&root), b"\xff\xfe.k7").unwrap();
    assert_eq!(
        read_raw(&root),
        Ok(Some(format!("{root}/\u{fffd}\u{fffd}.k7")))
    );
}

#[test]
fn a_folder_or_a_file_where_the_marker_belongs_reads_as_none_and_a_write_fails_with_a_code() {
    let a = Acct::new("pm-hostile");
    let root = root_of(&a);
    a.s.dir("studio/workspace/.active-projects/songs");
    assert_eq!(read_raw(&root), Ok(None));
    assert!(matches!(write(&root, "a.k7"), Err(ProjError::Io { .. })));
    let b = Acct::new("pm-file");
    let broot = root_of(&b);
    b.s.file("studio/workspace/.active-projects", "not a folder");
    assert_eq!(read_raw(&broot), Ok(None));
    let err = write(&broot, "a.k7").unwrap_err();
    assert!(
        matches!(
            err,
            ProjError::Io {
                syscall: "mkdir",
                ..
            }
        ),
        "{err:?}"
    );
}

#[test]
fn a_link_as_the_marker_is_followed_as_readfilesync_follows_it() {
    let a = Acct::new("pm-link");
    if !support::can_link(&a.s) {
        return;
    }
    let root = root_of(&a);
    let target = a.s.file("elsewhere/passwd.txt", "a.k7\n");
    a.s.dir("studio/workspace/.active-projects");
    support::link_file(&target, &marker_of(&root)).unwrap();
    assert_eq!(read_raw(&root), Ok(Some(format!("{root}/a.k7"))));
}

#[cfg(unix)]
#[test]
fn a_marker_folder_that_cannot_be_written_answers_its_code() {
    use std::os::unix::fs::PermissionsExt;
    if support::is_root() {
        return;
    }
    let a = Acct::new("pm-readonly");
    let root = root_of(&a);
    let dir = a.s.dir("studio/workspace/.active-projects");
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    let err = write(&root, "a.k7").unwrap_err();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(
        matches!(err, ProjError::Io { code: "EACCES", .. }),
        "{err:?}"
    );
    assert!(
        std::fs::read_dir(&dir).unwrap().next().is_none(),
        "no temp left"
    );
}

#[test]
fn a_write_among_siblings_leaves_them_as_they_were() {
    // The write reads and writes its own file and its temp, never the other workspaces' markers.
    let a = Acct::new("pm-siblings");
    let root = root_of(&a);
    for n in 0..12 {
        a.s.file(&format!("studio/workspace/.active-projects/w{n}"), "x.k7\n");
    }
    write(&root, "a.k7").unwrap();
    assert_eq!(read_raw(&root), Ok(Some(format!("{root}/a.k7"))));
    let dir = dirname(&marker_of(&root));
    let names = std::fs::read_dir(&dir).unwrap().count();
    assert_eq!(names, 13, "the twelve siblings and the marker, no temp");
    assert_eq!(
        std::fs::read_to_string(format!("{dir}/w7")).unwrap(),
        "x.k7\n"
    );
}

// F4: where Windows refuses to open a folder as a file, the folder still reads as none.
#[test]
fn a_denied_open_reads_as_none_only_where_a_folder_is_the_cause() {
    use std::io::ErrorKind::*;
    for kind in [NotFound, NotADirectory, IsADirectory] {
        assert!(reads_as_none(kind, || false), "{kind:?}");
    }
    assert!(reads_as_none(PermissionDenied, || true));
    assert!(!reads_as_none(PermissionDenied, || false));
    assert!(!reads_as_none(Other, || true));
}
