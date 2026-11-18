//! `read(rel)` (1042 to 1045): the order of its checks, the failures it names, and what it declines.

use crate::support;

use std::time::{Duration, Instant};

#[cfg(unix)]
use k7_file_service::read::open_regular;
use k7_file_service::read::{FileText, ReadError, read_text, read_text_gated};
use k7_file_service::space::{Declined, PathError, Space};
use k7_file_service::tree::Limits;
use support::{Scratch, can_link, link, link_file};

const BOUND: Duration = Duration::from_secs(if cfg!(target_env = "musl") { 60 } else { 15 });

fn read(s: &Scratch, rel: &str, limits: &Limits) -> Result<FileText, ReadError> {
    let h = s.home(false);
    read_text(&Space::open(&h, &s.path("ws")), rel, limits)
}

fn text(content: &str) -> Result<FileText, ReadError> {
    Ok(FileText {
        binary: false,
        content: content.into(),
    })
}

#[test]
fn reads_a_file_whole_and_keeps_a_bom_and_a_bad_byte_as_the_host_does() {
    let s = Scratch::new("read");
    s.dir("ws");
    s.file("ws/a.txt", "one\ntwo");
    s.file("ws/bom.txt", "\u{feff}x");
    std::fs::write(s.path("ws/bad.txt"), b"a\xffb").unwrap();
    let l = Limits::default();
    assert_eq!(read(&s, "a.txt", &l), text("one\ntwo"));
    assert_eq!(read(&s, "bom.txt", &l), text("\u{feff}x"));
    assert_eq!(read(&s, "bad.txt", &l), text("a\u{fffd}b"));
    assert_eq!(read(&s, "./a.txt", &l), text("one\ntwo"));
}

#[test]
fn a_binary_name_answers_before_any_stat() {
    let s = Scratch::new("binary");
    s.dir("ws/Folder.WAV");
    let l = Limits::default();
    let binary = Ok(FileText {
        binary: true,
        content: String::new(),
    });
    // A missing `.wav`, and a `.WAV` that is a folder: neither is looked at.
    assert_eq!(read(&s, "gone.wav", &l), binary);
    assert_eq!(read(&s, "Folder.WAV", &l), binary);
    // `.wav` alone is no extension.
    assert_eq!(
        read(&s, ".wav", &l),
        Err(ReadError::NotFound { rel: ".wav".into() })
    );
}

#[test]
fn names_what_is_wrong_with_the_entry() {
    let s = Scratch::new("names");
    s.dir("ws/dir");
    s.file("ws/file.txt", "x");
    let l = Limits::default();
    assert_eq!(
        read(&s, "missing.txt", &l),
        Err(ReadError::NotFound {
            rel: "missing.txt".into()
        })
    );
    assert_eq!(
        read(&s, "dir", &l),
        Err(ReadError::IsFolder { rel: "dir".into() })
    );
    assert_eq!(
        read(&s, ".", &l),
        Err(ReadError::IsFolder { rel: ".".into() })
    );
    // Below a file, POSIX's stat says ENOTDIR (a 400) and Windows' ERROR_PATH_NOT_FOUND, the shim's ENOENT (a 404).
    let below = if cfg!(windows) {
        ReadError::NotFound {
            rel: "file.txt/below".into(),
        }
    } else {
        ReadError::Io { code: "ENOTDIR" }
    };
    assert_eq!(read(&s, "file.txt/below", &l), Err(below));
    assert!(matches!(
        read(&s, "../x", &l),
        Err(ReadError::Path(PathError::Escapes { .. }))
    ));
    assert!(matches!(
        read(&s, "_libraries/.hidden/x", &l),
        Err(ReadError::Path(_))
    ));
}

#[test]
fn a_file_past_the_limit_or_not_a_file_is_declined() {
    let s = Scratch::new("limits");
    s.file("ws/at.txt", &"x".repeat(1000));
    s.file("ws/over.txt", &"x".repeat(1001));
    let small = Limits {
        max_text: 1000,
        ..Limits::default()
    };
    assert_eq!(
        read(&s, "at.txt", &small).map(|f| f.content.len()),
        Ok(1000)
    );
    assert_eq!(
        read(&s, "over.txt", &small),
        Err(ReadError::Declined(Declined::TooLarge))
    );
    let long = Limits {
        max_rel: 8,
        ..Limits::default()
    };
    assert_eq!(
        read(&s, "a/b/c/d/e/f.txt", &long),
        Err(ReadError::Declined(Declined::LongPath))
    );
}

#[cfg(unix)]
#[test]
fn a_fifo_is_declined_and_never_opened() {
    let s = Scratch::new("fifo");
    s.dir("ws");
    let fifo = s.path("ws/pipe");
    if !std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .is_ok_and(|s| s.success())
    {
        return;
    }
    let t0 = Instant::now();
    assert_eq!(
        read(&s, "pipe", &Limits::default()),
        Err(ReadError::Declined(Declined::NotRegular))
    );
    assert!(t0.elapsed() < BOUND);
}

#[test]
fn a_link_out_is_refused_by_the_gate_and_a_link_in_is_read() {
    let s = Scratch::new("read-link");
    if !can_link(&s) {
        return;
    }
    s.dir("ws");
    s.file("outside/secret.txt", "x");
    s.file("ws/real.txt", "in");
    link(&s.path("outside"), &s.path("ws/out")).unwrap();
    link_file(&s.path("ws/real.txt"), &s.path("ws/alias.txt")).unwrap();
    let l = Limits::default();
    assert!(matches!(
        read(&s, "out/secret.txt", &l),
        Err(ReadError::Path(PathError::Escapes { .. }))
    ));
    assert_eq!(read(&s, "alias.txt", &l), text("in"));
    // A folder link to a file: Windows' `canonicalize` cannot open it, so the gate refuses it as the shim's `realpath` does.
    link(&s.path("ws/real.txt"), &s.path("ws/folder-alias.txt")).unwrap();
    if cfg!(windows) {
        assert!(matches!(
            read(&s, "folder-alias.txt", &l),
            Err(ReadError::Path(PathError::Escapes { .. }))
        ));
    } else {
        assert_eq!(read(&s, "folder-alias.txt", &l), text("in"));
    }
}

#[test]
fn a_long_path_is_answered_fast() {
    let s = Scratch::new("long");
    s.dir("ws");
    let t0 = Instant::now();
    let rel = "a/".repeat(200);
    assert_eq!(
        read(&s, &rel, &Limits::default()),
        Err(ReadError::NotFound { rel: rel.clone() })
    );
    assert!(t0.elapsed() < BOUND);
}

#[cfg(unix)]
#[test]
fn a_fifo_swapped_in_after_the_stat_is_refused_and_does_not_block_the_open() {
    let s = Scratch::new("open-fifo");
    s.dir("ws");
    let fifo = s.path("ws/pipe");
    if !std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .is_ok_and(|s| s.success())
    {
        return;
    }
    let (tx, rx) = std::sync::mpsc::channel();
    // An open that blocks leaves its thread behind; the test fails on the timeout.
    std::thread::spawn(move || {
        let r = open_regular(&fifo).map(|_| ());
        let _ = tx.send(r);
    });
    assert_eq!(
        rx.recv_timeout(BOUND).expect("the open blocked"),
        Err(ReadError::Declined(Declined::NotRegular))
    );
}

#[cfg(unix)]
#[test]
fn a_link_at_the_leaf_is_followed_by_the_open_as_a_stat_follows_it() {
    let s = Scratch::new("open-link");
    s.file("ws/real.txt", "x");
    std::os::unix::fs::symlink(s.path("ws/real.txt"), s.path("ws/via.txt")).unwrap();
    assert_eq!(read(&s, "via.txt", &Limits::default()), text("x"));
}

#[test]
fn a_file_the_gate_does_not_admit_is_declined_busy_once_its_size_is_known() {
    let s = Scratch::new("admit");
    s.file("ws/big.txt", &"b".repeat(5_000));
    s.file("ws/small.txt", "s");
    let h = s.home(false);
    let space = Space::open(&h, &s.path("ws"));
    let admit = |n: u64| n < 1_000;
    assert_eq!(
        read_text_gated(&space, "big.txt", &Limits::default(), &admit),
        Err(ReadError::Declined(Declined::Busy))
    );
    assert_eq!(
        read_text_gated(&space, "small.txt", &Limits::default(), &admit),
        text("s")
    );
}
