//! The change token's inputs (1310 to 1314): the parity table of `walkProject(dir, true)`, the names' sum, the
//! limits, and the walk over hostile trees, each answered fast. The expected value is the TypeScript's rule.

use crate::support;

use std::fs::OpenOptions;
use std::time::{Duration, Instant, SystemTime};

use k7_file_service::revision::{Summary, outside_newest, spell, walk_sources};
use k7_file_service::tree::Limits;
use support::{Scratch, can_link, link, link_file};

const BOUND: Duration = Duration::from_secs(if cfg!(target_env = "musl") { 30 } else { 5 });

fn walk(s: &Scratch, rel: &str) -> (Summary, bool) {
    walk_sources(&s.path(rel), &Limits::default())
}

/// A file whose mtime is `secs` after the epoch, so a test names the `newest` it expects.
fn stamped(s: &Scratch, rel: &str, secs: u64) -> String {
    let p = s.file(rel, "");
    let f = OpenOptions::new().write(true).open(&p).unwrap();
    f.set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(secs))
        .unwrap();
    p
}

#[test]
fn a_k7x_and_a_vstpreset_are_counted_without_case() {
    // REVISION_FILE = /\.(k7x|vstpreset)$/i
    let s = Scratch::new("rev-case");
    stamped(&s, "p.k7/a.K7X", 10);
    stamped(&s, "p.k7/sub/b.VSTPRESET", 30);
    stamped(&s, "p.k7/c.vstpreset", 20);
    let (sum, unknowable) = walk(&s, "p.k7");
    assert_eq!((sum.count, sum.newest, unknowable), (3, 30_000.0, false));
}

#[test]
fn other_names_are_not_counted_and_a_wav_is_never_stated() {
    // a.k7x.bak, notes.md and take.wav do not match; only a source is stat'ed
    let s = Scratch::new("rev-other");
    s.file("p.k7/a.k7x.bak", "");
    s.file("p.k7/notes.md", "");
    s.file("p.k7/take.wav", "");
    s.file("p.k7/k7x", "");
    let (sum, _) = walk(&s, "p.k7");
    assert_eq!((sum.count, sum.newest, sum.names), (0, 0.0, 0));
    // A dangling link named like a wav is no source; named like one, its stat fails and it is skipped.
    if can_link(&s) {
        link_file(&s.path("nowhere"), &s.path("p.k7/gone.wav")).unwrap();
        assert_eq!(walk(&s, "p.k7").0.count, 0);
    }
}

#[test]
fn skipped_names_hide_their_sources() {
    // skip(): hidden names, node_modules, _recycle_bin, _history and *.cache
    let s = Scratch::new("rev-skip");
    for d in [
        ".sync",
        "node_modules",
        "_recycle_bin",
        "_history",
        "x.cache",
    ] {
        s.file(&format!("p.k7/{d}/y.k7x"), "");
    }
    s.file("p.k7/.hidden.k7x", "");
    s.file("p.k7/shown.k7x", "");
    s.file("p.k7/deep/.sync/z.k7x", "");
    assert_eq!(walk(&s, "p.k7").0.count, 1);
}

#[test]
fn a_directory_named_like_a_source_is_entered_not_counted() {
    // Dirent.isDirectory() wins over the name
    let s = Scratch::new("rev-dirnamed");
    s.file("p.k7/odd.k7x/inner.k7x", "");
    assert_eq!(walk(&s, "p.k7").0.count, 1);
}

#[test]
fn a_link_to_a_folder_is_not_entered_and_a_link_to_a_file_counts_its_target() {
    // isDirectory() is false for a link; statSync follows it
    let s = Scratch::new("rev-links");
    if !can_link(&s) {
        return;
    }
    s.file("elsewhere/inside.k7x", "");
    stamped(&s, "elsewhere/real.k7x", 50);
    s.dir("p.k7");
    link(&s.path("elsewhere"), &s.path("p.k7/folder")).unwrap();
    link_file(&s.path("elsewhere/real.k7x"), &s.path("p.k7/x.k7x")).unwrap();
    let (sum, _) = walk(&s, "p.k7");
    assert_eq!((sum.count, sum.newest), (1, 50_000.0));
    // A link to a file named like a source whose target is gone: the stat fails, the file is skipped.
    link_file(&s.path("nowhere"), &s.path("p.k7/dangling.k7x")).unwrap();
    assert_eq!(walk(&s, "p.k7").0.count, 1);
}

// 1797: a file is stat'ed from its listing on Unix; a link, which the walk follows, by its joined path.
#[test]
fn a_plain_file_is_stat_ed_from_the_listing_and_a_link_by_its_path() {
    use k7_file_service::revision::walk_project;
    let s = Scratch::new("rev-listing");
    stamped(&s, "p.k7/a.k7x", 10);
    stamped(&s, "p.k7/sub/b.k7x", 20);
    s.file("p.k7/notes.txt", "n");
    let w = walk_project(&s.path("p.k7"), &Limits::default(), false);
    assert_eq!(w.files.len(), 3);
    assert_eq!(w.by_path, if cfg!(unix) { 0 } else { 3 });
    if can_link(&s) {
        link_file(&s.path("p.k7/a.k7x"), &s.path("p.k7/ln.k7x")).unwrap();
        let w = walk_project(&s.path("p.k7"), &Limits::default(), false);
        let ln = w.files.iter().find(|f| f.rel == "ln.k7x").unwrap();
        assert_eq!(
            (ln.mtime_ms, w.by_path),
            (10_000.0, if cfg!(unix) { 1 } else { 4 })
        );
    }
}

#[cfg(unix)]
#[test]
fn an_unreadable_folder_is_empty_and_the_rest_is_counted() {
    use std::os::unix::fs::PermissionsExt;
    if support::is_root() {
        return;
    }
    let s = Scratch::new("rev-chmod");
    s.file("p.k7/shut/a.k7x", "");
    s.file("p.k7/open/b.k7x", "");
    let shut = s.path("p.k7/shut");
    std::fs::set_permissions(&shut, std::fs::Permissions::from_mode(0o000)).unwrap();
    let (sum, unknowable) = walk(&s, "p.k7");
    std::fs::set_permissions(&shut, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!((sum.count, unknowable), (1, false));
}

#[test]
fn a_swap_or_a_rename_moves_the_names_alone() {
    // 1311: count and newest are the TypeScript's, and a rename or a swap moves neither
    let s = Scratch::new("rev-names");
    stamped(&s, "p.k7/a.k7x", 10);
    stamped(&s, "p.k7/b.k7x", 20);
    let before = walk(&s, "p.k7").0;
    std::fs::rename(s.path("p.k7/a.k7x"), s.path("p.k7/c.k7x")).unwrap();
    let renamed = walk(&s, "p.k7").0;
    assert_eq!(
        (renamed.count, renamed.newest),
        (before.count, before.newest)
    );
    assert_ne!(renamed.names, before.names);
    // Swapped: a takes b's stamp and b takes a's, through a third name.
    std::fs::rename(s.path("p.k7/b.k7x"), s.path("p.k7/tmp.k7x")).unwrap();
    std::fs::rename(s.path("p.k7/c.k7x"), s.path("p.k7/b.k7x")).unwrap();
    std::fs::rename(s.path("p.k7/tmp.k7x"), s.path("p.k7/c.k7x")).unwrap();
    let swapped = walk(&s, "p.k7").0;
    // The same two names hold again, so the sum is the renamed one's: order-free, as the sum is.
    assert_eq!(swapped.names, renamed.names);
    // A folder renamed moves the names of every source below it.
    std::fs::create_dir(s.path("p.k7/d1")).unwrap();
    std::fs::rename(s.path("p.k7/b.k7x"), s.path("p.k7/d1/b.k7x")).unwrap();
    let before = walk(&s, "p.k7").0;
    std::fs::rename(s.path("p.k7/d1"), s.path("p.k7/d2")).unwrap();
    let after = walk(&s, "p.k7").0;
    assert_ne!(after.names, before.names);
    assert_eq!(after.count, before.count);
}

#[test]
fn the_newest_is_the_formula_of_the_hosts_fs_module() {
    // seconds * 1000 + nanoseconds / 1e6
    let s = Scratch::new("rev-ms");
    let p = s.file("p.k7/a.k7x", "");
    let f = OpenOptions::new().write(true).open(&p).unwrap();
    f.set_modified(SystemTime::UNIX_EPOCH + Duration::new(1_700_000_000, 250_000_000))
        .unwrap();
    let got = walk(&s, "p.k7").0.newest;
    assert!((got - 1_700_000_000_250.0).abs() < 1.0, "{got}");
}

#[test]
fn a_limit_makes_the_walk_unknowable() {
    // 1314: depth 300 under the default 256, and more entries than the limit allows
    let s = Scratch::new("rev-limits");
    s.file(&format!("deep.k7/{}x.k7x", "d/".repeat(300)), "");
    assert!(walk(&s, "deep.k7").1);
    for i in 0..300 {
        s.file(&format!("wide.k7/f{i}.k7x"), "");
    }
    let small = Limits {
        max_seen: 100,
        ..Limits::default()
    };
    assert!(walk_sources(&s.path("wide.k7"), &small).1);
    assert!(!walk(&s, "wide.k7").1);
}

#[test]
fn the_outside_files_are_the_newest_of_three_and_a_missing_one_is_zero() {
    let s = Scratch::new("rev-outside");
    let h = s.home();
    assert_eq!(outside_newest(&h), 0.0);
    stamped(&s, "studio/exts.json", 5);
    stamped(&s, "studio/preferences.yaml", 9);
    assert_eq!(outside_newest(&h), 9_000.0);
}

#[test]
fn the_spelling_is_boot_dot_epoch() {
    // 1310
    assert_eq!(spell("lz4k", 7, true), "lz4k.7");
    assert_eq!(spell("lz4k", 7, false), "none:lz4k.7");
}

// Hostile trees: each answered fast, the bound held on the best of three.

#[test]
fn a_folder_of_twenty_thousand_files_and_one_source() {
    wide_folder(20_000);
}

#[test]
#[ignore = "scale: a quadratic walk hides at 20,000 files and shows at 200,000"]
fn a_folder_of_two_hundred_thousand_files_and_one_source_at_scale() {
    wide_folder(200_000);
}

fn wide_folder(n: u32) {
    let s = Scratch::new("rev-wide");
    let dir = s.dir("p.k7/wide");
    std::thread::scope(|scope| {
        for part in 0..8u32 {
            let dir = &dir;
            scope.spawn(move || {
                for i in (part * n / 8)..((part + 1) * n / 8) {
                    std::fs::File::create(format!("{dir}/f{i:06}.txt")).unwrap();
                }
            });
        }
    });
    s.file("p.k7/wide/only.k7x", "");
    let mut best = Duration::MAX;
    for _ in 0..3 {
        let t0 = Instant::now();
        let (sum, unknowable) = walk(&s, "p.k7");
        best = best.min(t0.elapsed());
        assert_eq!((sum.count, unknowable), (1, false));
    }
    assert!(best < BOUND, "{best:?}");
}

#[test]
fn two_thousand_wavs_and_one_source_stat_the_source_alone() {
    let s = Scratch::new("rev-wavs");
    let clips = s.dir("p.k7/clips");
    for i in 0..2_000 {
        std::fs::File::create(format!("{clips}/t{i}.wav")).unwrap();
    }
    s.file("p.k7/main.k7x", "");
    let t0 = Instant::now();
    assert_eq!(walk(&s, "p.k7").0.count, 1);
    assert!(t0.elapsed() < BOUND);
}

#[test]
fn a_long_relative_path_is_walked_and_spelt_whole() {
    // Names stop at 255 bytes and PATH_MAX at 1,024: four folders of 200 bytes are the longest a Mac holds
    let s = Scratch::new("rev-longpath");
    let rel = format!("{}/x.k7x", vec!["n".repeat(200); 4].join("/"));
    s.file(&format!("p.k7/{rel}"), "");
    let t0 = Instant::now();
    let (sum, unknowable) = walk(&s, "p.k7");
    assert_eq!((sum.count, unknowable), (1, false));
    assert!(sum.names != 0 && t0.elapsed() < BOUND);
}

#[test]
fn a_nest_three_hundred_deep_is_walked_without_recursion() {
    let s = Scratch::new("rev-deep");
    s.file(&format!("p.k7/{}x.k7x", "d/".repeat(300)), "");
    let limits = Limits {
        max_depth: 400,
        ..Limits::default()
    };
    let dir = s.path("p.k7");
    let t0 = Instant::now();
    // A thread with a small stack: nothing recurses.
    let (sum, unknowable) = std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(move || walk_sources(&dir, &limits))
        .unwrap()
        .join()
        .unwrap();
    assert_eq!((sum.count, unknowable), (1, false));
    assert!(t0.elapsed() < BOUND);
}

#[cfg(unix)]
#[test]
fn a_name_holding_a_backslash_is_the_shim_s_spelling_and_so_not_found() {
    // join() reads `\` as `/`: node stats `a/b.k7x`, which is not there, and skips the file
    let s = Scratch::new("rev-backslash");
    s.dir("p.k7");
    if std::fs::File::create(s.raw("p.k7/a\\b.k7x")).is_err() {
        return;
    }
    s.file("p.k7/ok.k7x", "");
    assert_eq!(walk(&s, "p.k7").0.count, 1);
}

#[cfg(unix)]
#[test]
fn a_name_that_is_not_utf8_is_skipped_where_the_file_system_takes_it() {
    use std::os::unix::ffi::OsStrExt;
    let s = Scratch::new("rev-nonutf8");
    s.dir("p.k7");
    let dir = std::path::PathBuf::from(s.path("p.k7"));
    let odd = dir.join(std::ffi::OsStr::from_bytes(b"\xff\xfe.k7x"));
    if std::fs::File::create(odd).is_err() {
        return;
    }
    s.file("p.k7/ok.k7x", "");
    // The lossy name names no file: the stat fails, as node's would on its decoded string.
    assert_eq!(walk(&s, "p.k7").0.count, 1);
}
