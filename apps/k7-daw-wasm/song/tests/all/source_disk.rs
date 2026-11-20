// source_disk.rs — DiskSource over temporary folders: listing, stamps, ranges, and every escape confine.test.ts holds.
// A reader would get wrong that the link cases skip where links need rights; the lexical ones always run.

use crate::support;

use song::{DiskSource, Kind, Source, SourceError, Span};
use std::fs;
use std::path::Path;
use support::Tmp;

/// Makes a directory link; false when this machine's rights do not allow it.
fn link_dir(target: &Path, link: &Path) -> bool {
    #[cfg(unix)]
    return std::os::unix::fs::symlink(target, link).is_ok();
    #[cfg(windows)]
    return std::os::windows::fs::symlink_dir(target, link).is_ok();
    #[cfg(not(any(unix, windows)))]
    return false;
}

fn link_file(target: &Path, link: &Path) -> bool {
    #[cfg(unix)]
    return std::os::unix::fs::symlink(target, link).is_ok();
    #[cfg(windows)]
    return std::os::windows::fs::symlink_file(target, link).is_ok();
    #[cfg(not(any(unix, windows)))]
    return false;
}

fn project() -> (Tmp, DiskSource) {
    let t = Tmp::new("disk");
    t.write("alice/song/clips/kick.k7x", "source(\"kick.wav\")\n");
    t.write("bob/secret.wav", "RIFF");
    let src = DiskSource::new(&[t.at("alice/song")]).expect("a source");
    (t, src)
}

#[test]
fn answers_a_path_inside_a_mount_normalised() {
    // confine.test.ts: answers a path inside a root, absolute and normalised.
    let (t, src) = project();
    let st = src.stat(&t.at("alice/song/clips/../clips/./kick.k7x")).expect("stat");
    assert_eq!(st.kind, Kind::File);
    assert_eq!(
        src.read_text(&t.at("alice/song/clips/kick.k7x")).expect("read"),
        "source(\"kick.wav\")\n"
    );
}

#[test]
fn refuses_a_dot_dot_out_an_absolute_elsewhere_a_sibling_prefix_and_a_nul() {
    // confine.test.ts: refuses a `..` out, an absolute path elsewhere, a sibling prefix and a NUL.
    let (t, src) = project();
    assert_eq!(src.stat(&t.at("alice/song/../../x")), Err(SourceError::Outside));
    assert_eq!(src.stat(&t.at("bob/secret.wav")), Err(SourceError::Outside));
    assert_eq!(src.stat(&t.at("alice/song-evil/a.wav")), Err(SourceError::Outside));
    assert!(matches!(src.stat(&t.at("alice/song/a\0.wav")), Err(SourceError::Io(_))));
    assert!(matches!(src.stat(""), Err(SourceError::Io(_))));
    assert!(matches!(src.stat("alice/song"), Err(SourceError::Io(_))));
    assert_eq!(src.read("/no/such/place", Span::All), Err(SourceError::Outside));
}

#[test]
fn refuses_a_link_inside_the_mount_that_leads_out() {
    // confine.test.ts: refuses a symlink inside the root that leads out of every root.
    let (t, src) = project();
    if !link_dir(&t.0.join("bob"), &t.0.join("alice/song/out")) {
        return;
    }
    assert_eq!(
        src.read(&t.at("alice/song/out/secret.wav"), Span::All),
        Err(SourceError::Outside)
    );
    assert_eq!(src.stat(&t.at("alice/song/out/new.wav")), Err(SourceError::Outside));
    assert_eq!(src.list(&t.at("alice/song/out")), Err(SourceError::Outside));
}

#[test]
fn refuses_a_file_link_out_and_a_dangling_one() {
    // confine.test.ts: refuses a dangling link, which a write would follow out.
    let (t, src) = project();
    if !link_file(&t.0.join("bob/secret.wav"), &t.0.join("alice/song/leak.wav")) {
        return;
    }
    assert_eq!(
        src.read(&t.at("alice/song/leak.wav"), Span::All),
        Err(SourceError::Outside)
    );
    assert!(link_file(
        &t.0.join("bob/planted.wav"),
        &t.0.join("alice/song/trap.wav")
    ));
    assert_eq!(src.stat(&t.at("alice/song/trap.wav")), Err(SourceError::Outside));
}

#[test]
fn accepts_a_link_from_one_mount_into_another() {
    // confine.test.ts: accepts a link from one allowed root into another.
    let t = Tmp::new("disk-two");
    t.write("alice/song/a.k7x", "x(1)\n");
    t.write("libs/drums/kick.wav", "RIFF");
    let src = DiskSource::new(&[t.at("alice/song"), t.at("libs")]).expect("a source");
    if !link_dir(&t.0.join("libs/drums"), &t.0.join("alice/song/lib")) {
        return;
    }
    assert_eq!(
        src.read(&t.at("alice/song/lib/kick.wav"), Span::All),
        Ok(b"RIFF".to_vec())
    );
}

#[test]
fn allows_a_missing_mount_and_confines_it_like_an_existing_one() {
    let t = Tmp::new("disk-missing");
    let src = DiskSource::new(&[t.at("not/yet")]).expect("a source");
    assert_eq!(src.stat(&t.at("not/yet/clips")), Err(SourceError::NotFound));
    assert_eq!(src.stat(&t.at("not/elsewhere")), Err(SourceError::Outside));
}

#[test]
fn lists_bytewise_and_counts_a_link_as_what_it_points_at() {
    let (t, src) = project();
    t.write("alice/song/clips/B.k7x", "x(1)\n");
    t.write("alice/song/clips/a-b.k7x", "x(1)\n");
    t.write("alice/song/clips/a.k7x", "x(1)\n");
    t.mkdir("alice/song/clips/sub");
    let names: Vec<String> = src
        .list(&t.at("alice/song/clips"))
        .expect("list")
        .into_iter()
        .map(|e| e.name)
        .collect();
    assert_eq!(names, ["B.k7x", "a-b.k7x", "a.k7x", "kick.k7x", "sub"]);
    if link_file(
        &t.0.join("alice/song/clips/a.k7x"),
        &t.0.join("alice/song/clips/linked.k7x"),
    ) {
        let list = src.list(&t.at("alice/song/clips")).expect("list");
        let linked = list.iter().find(|e| e.name == "linked.k7x").expect("listed");
        assert_eq!(linked.kind, Kind::File);
    }
}

#[test]
fn a_fresh_file_is_unsettled_and_an_old_one_settled() {
    let (t, src) = project();
    let fresh = t.at("alice/song/clips/kick.k7x");
    assert!(!src.stat(&fresh).expect("stat").stamp.settled);
    let old = std::time::SystemTime::now() - std::time::Duration::from_secs(60);
    let file = fs::OpenOptions::new()
        .write(true)
        .open(t.0.join("alice/song/clips/kick.k7x"))
        .expect("open");
    file.set_modified(old).expect("backdate");
    let st = src.stat(&fresh).expect("stat");
    assert!(st.stamp.settled);
    assert_eq!(st.stamp.len, st.len);
}

#[test]
fn reads_a_range_of_a_file() {
    let (t, src) = project();
    t.write("alice/song/head.bin", "0123456789");
    let at = t.at("alice/song/head.bin");
    assert_eq!(src.read(&at, Span::Range { offset: 2, len: 3 }), Ok(b"234".to_vec()));
    assert_eq!(src.read(&at, Span::Range { offset: 8, len: 99 }), Ok(b"89".to_vec()));
    assert_eq!(
        src.read(&t.at("alice/song/absent"), Span::All),
        Err(SourceError::NotFound)
    );
}

#[test]
fn refuses_text_that_is_not_utf8() {
    let (t, src) = project();
    fs::write(t.0.join("alice/song/bad.k7x"), [0xff, 0xfe, 0x00]).expect("write");
    assert!(matches!(
        src.read_text(&t.at("alice/song/bad.k7x")),
        Err(SourceError::Io(_))
    ));
}

#[test]
fn a_backslash_or_a_colon_in_a_name_is_what_windows_would_read_as_more() {
    use song::source::path::windows_hazard;
    assert!(windows_hazard("C:/p/..\\..\\x"));
    assert!(windows_hazard("C:/p/a:stream"));
    assert!(windows_hazard("/p/a\\b"));
    assert!(!windows_hazard("C:/p/a/b"));
    assert!(!windows_hazard("/p/a/b"));
}

#[cfg(windows)]
#[test]
fn the_disk_source_refuses_a_backslash_climb_and_a_stream_name_on_windows() {
    let (t, src) = project();
    let climb = format!("{}/alice/song/..\\..\\bob/secret.wav", support::spell(&t.0));
    assert_eq!(src.read(&climb, Span::All), Err(SourceError::Outside));
    assert_eq!(
        src.stat(&t.at("alice/song/clips/kick.k7x:stream")),
        Err(SourceError::Outside)
    );
}

#[test]
fn only_files_and_folders_are_listed() {
    let (t, src) = project();
    t.write("alice/song/clips/a.k7x", "x(1)\n");
    #[cfg(unix)]
    {
        let sock = t.0.join("alice/song/clips/live.k7x");
        if std::os::unix::net::UnixListener::bind(&sock).is_ok() {
            let names: Vec<String> = src
                .list(&t.at("alice/song/clips"))
                .expect("list")
                .into_iter()
                .map(|e| e.name)
                .collect();
            assert!(!names.contains(&"live.k7x".to_owned()));
        }
    }
    assert!(
        src.list(&t.at("alice/song/clips"))
            .expect("list")
            .iter()
            .all(|e| matches!(e.kind, Kind::File | Kind::Dir))
    );
}
