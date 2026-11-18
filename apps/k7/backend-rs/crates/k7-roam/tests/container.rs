//! The move out of the iCloud container, over temp folders: nothing is overwritten, nothing is lost.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use k7_roam::container::{Ops, Real, Why, is_dataless, migrate, migrate_with};

static N: AtomicU32 = AtomicU32::new(0);

struct Rig {
    source: PathBuf,
    target: PathBuf,
}

impl Rig {
    fn new() -> Rig {
        let n = N.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!("k7-roam-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (source, target) = (root.join("container"), root.join("local"));
        std::fs::create_dir_all(&source).unwrap();
        std::fs::create_dir_all(&target).unwrap();
        Rig { source, target }
    }

    fn put(&self, rel: &str, bytes: &[u8]) {
        let f = self.source.join(rel);
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(f, bytes).unwrap();
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(self.source.parent().unwrap());
    }
}

/// A seam that fails on demand: the rename (a cross-volume move), one file's copy, or the removal of the original.
#[derive(Default)]
struct Fault {
    rename: bool,
    copy: Option<&'static str>,
    remove_original: bool,
}

impl Ops for Fault {
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        // The copy's own final rename (from the scratch folder) goes through; only a rename out of the container fails.
        if self.rename && !from.to_string_lossy().contains(".migrating") {
            return Err(io::Error::other("cross-device"));
        }
        Real.rename(from, to)
    }
    fn copy_file(&self, from: &Path, to: &Path) -> io::Result<u64> {
        if self.copy.is_some_and(|c| from.ends_with(c)) {
            return Err(io::Error::other("disk full"));
        }
        Real.copy_file(from, to)
    }
    fn remove_tree(&self, at: &Path) -> io::Result<()> {
        if self.remove_original && at.to_string_lossy().contains("container") {
            return Err(io::Error::other("busy"));
        }
        Real.remove_tree(at)
    }
}

fn repo(at: &Path) {
    let r = git2::Repository::init(at).unwrap();
    std::fs::write(at.join("song.k7x"), b"a song").unwrap();
    let mut idx = r.index().unwrap();
    idx.add_path(Path::new("song.k7x")).unwrap();
    idx.write().unwrap();
    let tree = r.find_tree(idx.write_tree().unwrap()).unwrap();
    let sig = git2::Signature::now("t", "t@t").unwrap();
    r.commit(Some("HEAD"), &sig, &sig, "one", &tree, &[])
        .unwrap();
}

fn read(p: &Path) -> Vec<u8> {
    std::fs::read(p).unwrap()
}

#[test]
fn a_workspace_moves_whole_and_leaves_the_container() {
    let r = Rig::new();
    r.put("Banc/a.k7x", b"1");
    r.put("Banc/deep/er/b.wav", b"2222");
    r.put("abcd/c.k7x", b"3");
    let report = migrate(&r.source, &r.target);
    assert_eq!(report.moved, ["Banc", "abcd"]);
    assert_eq!(read(&r.target.join("Banc/deep/er/b.wav")), b"2222");
    assert!(!r.source.join("Banc").exists() && !r.source.join("abcd").exists());
}

#[test]
fn a_name_already_here_is_never_overwritten_and_both_sides_stay() {
    let r = Rig::new();
    r.put("Examples/theirs.k7x", b"container");
    std::fs::create_dir_all(r.target.join("Examples")).unwrap();
    std::fs::write(r.target.join("Examples/mine.k7x"), b"local").unwrap();
    let report = migrate(&r.source, &r.target);
    assert!(report.moved.is_empty());
    assert_eq!(report.kept, [("Examples".to_string(), Why::Exists)]);
    assert_eq!(read(&r.source.join("Examples/theirs.k7x")), b"container");
    assert_eq!(read(&r.target.join("Examples/mine.k7x")), b"local");
    assert!(!r.target.join("Examples/theirs.k7x").exists());
}

#[test]
fn a_workspace_with_a_placeholder_stays_and_the_others_still_move() {
    let r = Rig::new();
    r.put("Whole/a.k7x", b"1");
    r.put("Cloudy/a.k7x", b"1");
    r.put("Cloudy/take/.take.wav.icloud", b"stub");
    r.put(".Evicted.icloud", b"stub");
    let report = migrate(&r.source, &r.target);
    assert_eq!(report.moved, ["Whole"]);
    assert_eq!(report.kept.len(), 2);
    assert!(
        report
            .kept
            .iter()
            .all(|(_, w)| matches!(w, Why::NotDownloaded(_)))
    );
    assert!(r.source.join("Cloudy/take/.take.wav.icloud").exists());
    assert!(!r.target.join("Cloudy").exists() && !r.target.join("Evicted").exists());
}

#[test]
fn the_flag_of_a_file_icloud_has_not_downloaded_is_read() {
    assert!(is_dataless(0x4000_0000 | 0x20));
    assert!(!is_dataless(0x20));
}

#[test]
fn across_volumes_the_copy_is_verified_then_the_original_is_deleted() {
    let r = Rig::new();
    r.put("Banc/a.k7x", b"1");
    r.put("Banc/audio/b.wav", b"2222");
    repo(&r.source.join("Banc"));
    let report = migrate_with(
        &Fault {
            rename: true,
            ..Fault::default()
        },
        &r.source,
        &r.target,
    );
    assert_eq!(report.moved, ["Banc"]);
    assert_eq!(read(&r.target.join("Banc/audio/b.wav")), b"2222");
    assert!(git2::Repository::open(r.target.join("Banc")).is_ok());
    assert!(!r.source.join("Banc").exists());
    assert!(std::fs::read_dir(&r.target).unwrap().all(|e| {
        !e.unwrap()
            .file_name()
            .to_string_lossy()
            .contains("migrating")
    }));
}

#[test]
fn a_copy_that_fails_leaves_the_original_whole_and_no_half_copy() {
    let r = Rig::new();
    r.put("Banc/a.k7x", b"1");
    r.put("Banc/bad.wav", b"2222");
    let report = migrate_with(
        &Fault {
            rename: true,
            copy: Some("bad.wav"),
            ..Fault::default()
        },
        &r.source,
        &r.target,
    );
    assert!(report.moved.is_empty());
    assert!(matches!(report.kept[0].1, Why::Failed(_)));
    assert_eq!(read(&r.source.join("Banc/bad.wav")), b"2222");
    assert_eq!(read(&r.source.join("Banc/a.k7x")), b"1");
    assert!(std::fs::read_dir(&r.target).unwrap().next().is_none());
}

#[test]
fn an_original_that_cannot_be_removed_leaves_both_copies_and_says_so() {
    let r = Rig::new();
    r.put("Banc/a.k7x", b"1");
    let report = migrate_with(
        &Fault {
            rename: true,
            remove_original: true,
            ..Fault::default()
        },
        &r.source,
        &r.target,
    );
    assert_eq!(report.leftover, ["Banc"]);
    assert_eq!(read(&r.target.join("Banc/a.k7x")), b"1");
    assert_eq!(read(&r.source.join("Banc/a.k7x")), b"1");
    // The next run finds the name taken and touches neither.
    let again = migrate(&r.source, &r.target);
    assert_eq!(again.kept, [("Banc".to_string(), Why::Exists)]);
    assert_eq!(read(&r.target.join("Banc/a.k7x")), b"1");
}

#[cfg(unix)]
#[test]
fn a_repository_with_a_damaged_object_is_not_moved() {
    let r = Rig::new();
    repo(&r.source.join("Banc"));
    let git = r.source.join("Banc/.git/objects");
    let loose = walk_files(&git)
        .into_iter()
        .find(|p| !p.to_string_lossy().contains("pack") && !p.to_string_lossy().contains("info"))
        .unwrap();
    std::fs::set_permissions(&loose, std::os::unix::fs::PermissionsExt::from_mode(0o644)).unwrap();
    let len = std::fs::metadata(&loose).unwrap().len();
    std::fs::write(&loose, vec![7u8; len as usize]).unwrap();
    let report = migrate_with(
        &Fault {
            rename: true,
            ..Fault::default()
        },
        &r.source,
        &r.target,
    );
    assert!(report.moved.is_empty());
    assert!(
        matches!(&report.kept[0].1, Why::Failed(e) if e.contains("repository")),
        "{:?}",
        report.kept
    );
    assert!(r.source.join("Banc/song.k7x").exists());
    assert!(std::fs::read_dir(&r.target).unwrap().next().is_none());
}

#[cfg(unix)]
fn walk_files(d: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(d).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(walk_files(&p));
        } else {
            out.push(p);
        }
    }
    out
}

#[cfg(unix)]
#[test]
fn a_git_link_to_nosync_comes_out_as_a_real_git_folder_by_either_way_of_moving() {
    for rename in [false, true] {
        let r = Rig::new();
        repo(&r.source.join("Banc"));
        std::fs::rename(
            r.source.join("Banc/.git"),
            r.source.join("Banc/.git.nosync"),
        )
        .unwrap();
        std::os::unix::fs::symlink(".git.nosync", r.source.join("Banc/.git")).unwrap();
        let report = migrate_with(
            &Fault {
                rename: !rename,
                ..Fault::default()
            },
            &r.source,
            &r.target,
        );
        assert_eq!(report.moved, ["Banc"], "{:?}", report.kept);
        let dot = std::fs::symlink_metadata(r.target.join("Banc/.git")).unwrap();
        assert!(dot.is_dir() && !dot.file_type().is_symlink());
        assert!(!r.target.join("Banc/.git.nosync").exists());
        assert!(git2::Repository::open(r.target.join("Banc")).is_ok());
    }
}

#[test]
fn a_second_run_changes_nothing() {
    let r = Rig::new();
    r.put("Banc/a.k7x", b"1");
    r.put(".active-projects/Banc", b"song.k7x");
    assert_eq!(migrate(&r.source, &r.target).moved, ["Banc"]);
    let second = migrate(&r.source, &r.target);
    assert!(second.moved.is_empty() && second.kept.is_empty() && second.leftover.is_empty());
    assert_eq!(read(&r.target.join("Banc/a.k7x")), b"1");
    assert_eq!(read(&r.target.join(".active-projects/Banc")), b"song.k7x");
}

#[test]
fn a_marker_moves_only_when_this_device_has_none_of_that_name() {
    let r = Rig::new();
    r.put(".active-projects/New", b"new");
    r.put(".active-projects/Same", b"container");
    std::fs::create_dir_all(r.target.join(".active-projects")).unwrap();
    std::fs::write(r.target.join(".active-projects/Same"), b"local").unwrap();
    migrate(&r.source, &r.target);
    assert_eq!(read(&r.target.join(".active-projects/New")), b"new");
    assert_eq!(read(&r.target.join(".active-projects/Same")), b"local");
    assert_eq!(read(&r.source.join(".active-projects/Same")), b"container");
    assert!(!r.source.join(".active-projects/New").exists());
}

#[test]
fn a_missing_container_is_nothing_to_do() {
    let r = Rig::new();
    let report = migrate(&r.source.join("absent"), &r.target);
    assert!(report.moved.is_empty() && report.kept.is_empty());
}
