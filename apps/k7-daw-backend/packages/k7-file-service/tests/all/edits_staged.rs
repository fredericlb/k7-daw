//! The server's staged edits (3j M1): a write is invisible on the disk before the commit, a failed commit leaves it as it was.

use crate::support;

use k7_file_service::atomic::Real;
use k7_file_service::changes::commit::commit;
use k7_file_service::edits::StagedFs;
use k7_file_service::edits::stamp::{stamp_format_at_open, stamp_format_for};
use song::{Kind, Source, Span};
use song_edit::Fs;
use support::Scratch;
use support::changes::{Flaky, tree};

fn seeded(name: &str) -> (Scratch, String) {
    let s = Scratch::new(name);
    let dir = s.dir("p.k7");
    s.file("p.k7/a.k7x", "a\n");
    s.file("p.k7/b.k7x", "b\n");
    s.file("p.k7/m.k7x", "m\n");
    (s, dir)
}

fn stage(dir: &str) -> StagedFs {
    StagedFs::new(&[dir]).unwrap()
}

// A staged write is invisible on the disk before the commit, and a read of the stage sees it.
#[test]
fn a_staged_write_is_invisible_before_the_commit() {
    let (s, dir) = seeded("e-invisible");
    let before = tree(&dir);
    let mut fs = stage(&dir);
    fs.write_atomic(&format!("{dir}/a.k7x"), b"new a\n")
        .unwrap();
    fs.write_atomic(&format!("{dir}/c.k7x"), b"new c\n")
        .unwrap();
    fs.remove(&format!("{dir}/b.k7x")).unwrap();
    assert_eq!(tree(&dir), before);
    assert_eq!(
        fs.read(&format!("{dir}/a.k7x"), Span::All).unwrap(),
        b"new a\n"
    );
    assert!(fs.stat(&format!("{dir}/b.k7x")).is_err());
    let names: Vec<_> = fs.list(&dir).unwrap().into_iter().map(|e| e.name).collect();
    assert_eq!(names, ["a.k7x", "c.k7x", "m.k7x"]);
    drop(s);
}

// The committed set lands every write, the removal and the move.
#[test]
fn the_committed_stage_lands_whole() {
    let (_s, dir) = seeded("e-lands");
    let mut fs = stage(&dir);
    fs.write_atomic(&format!("{dir}/a.k7x"), b"new a\n")
        .unwrap();
    fs.remove(&format!("{dir}/b.k7x")).unwrap();
    fs.mkdir_all(&format!("{dir}/moved")).unwrap();
    fs.rename(&format!("{dir}/m.k7x"), &format!("{dir}/moved/m.k7x"))
        .unwrap();
    // The moved file is read at its new path before anything lands.
    assert_eq!(
        fs.read(&format!("{dir}/moved/m.k7x"), Span::All).unwrap(),
        b"m\n"
    );
    assert!(fs.stat(&format!("{dir}/m.k7x")).is_err());
    commit(&Real, &fs.planned().unwrap(), &mut |_| {}).unwrap();
    let t = tree(&dir);
    assert_eq!(t["a.k7x"], "new a\n");
    assert!(!t.contains_key("b.k7x"));
    assert!(!t.contains_key("m.k7x"));
    assert_eq!(t["moved/m.k7x"], "m\n");
}

// A commit that fails at any rename leaves the disk as it was, whatever file it fails on.
#[test]
fn a_failed_commit_leaves_the_disk_as_it_was() {
    let (_s, dir) = seeded("e-rollback");
    let before = tree(&dir);
    for at in 0..8 {
        let mut fs = stage(&dir);
        fs.write_atomic(&format!("{dir}/a.k7x"), b"new a\n")
            .unwrap();
        fs.write_atomic(&format!("{dir}/b.k7x"), b"new b\n")
            .unwrap();
        fs.write_atomic(&format!("{dir}/m.k7x"), b"new m\n")
            .unwrap();
        let landed = commit(&Flaky::at(at), &fs.planned().unwrap(), &mut |_| {});
        if landed.is_ok() {
            break;
        }
        assert_eq!(tree(&dir), before, "fail at rename {at}");
    }
}

// A folder is renamed on the disk first: the stage refuses it, and a binary file is not carried.
#[test]
fn a_folder_and_a_binary_are_refused() {
    let (s, dir) = seeded("e-refused");
    s.dir("p.k7/sub");
    let mut fs = stage(&dir);
    assert!(
        fs.rename(&format!("{dir}/sub"), &format!("{dir}/other"))
            .is_err()
    );
    fs.write_atomic(&format!("{dir}/x.k7x"), &[0xff, 0xfe])
        .unwrap();
    assert!(fs.planned().is_err());
}

// A write outside the mounts is refused as a read is.
#[test]
fn the_stage_is_confined_to_its_mounts() {
    let (s, dir) = seeded("e-confined");
    s.file("other/x.k7x", "x\n");
    let fs = stage(&dir);
    let out = s.path("other/x.k7x");
    assert!(fs.read(&out, Span::All).is_err());
    assert_eq!(fs.stat(&dir).unwrap().kind, Kind::Dir);
}

// The stamp: a lane with a start in seconds raises the project's level; a musical one does not; a second pass writes nothing.
#[test]
fn the_stamp_raises_format_once() {
    let s = Scratch::new("e-stamp");
    let dir = s.dir("w/p.k7");
    s.file("w/p.k7/project.k7x", "project({ name: \"p\" })\n");
    let lane = s.file(
        "w/p.k7/nodes/fx.k7x",
        "extends(\"@meta/track\")\nevents([ref(\"hit\").start(\"2:1:1\")])\n",
    );
    let root = s.path("w");
    assert!(!stamp_format_for(&root, &lane));
    assert!(
        !std::fs::read_to_string(format!("{dir}/project.k7x"))
            .unwrap()
            .contains("format")
    );
    let lane = s.file(
        "w/p.k7/nodes/fx.k7x",
        "extends(\"@meta/track\")\nevents([ref(\"hit\").start(4.5)])\n",
    );
    assert!(stamp_format_for(&root, &lane));
    assert!(
        std::fs::read_to_string(format!("{dir}/project.k7x"))
            .unwrap()
            .contains("format(2)")
    );
    assert!(!stamp_format_for(&root, &lane));
    assert!(!stamp_format_at_open(&dir));
}

// A copy renamed keeps its source on the disk, and the name it had is gone.
#[test]
fn a_copy_renamed_keeps_its_source() {
    let (_s, dir) = seeded("e-copy-rename");
    let mut fs = stage(&dir);
    fs.copy(&format!("{dir}/a.k7x"), &format!("{dir}/c.k7x"))
        .unwrap();
    fs.rename(&format!("{dir}/c.k7x"), &format!("{dir}/d.k7x"))
        .unwrap();
    commit(&Real, &fs.planned().unwrap(), &mut |_| {}).unwrap();
    let t = tree(&dir);
    assert_eq!(t["a.k7x"], "a\n");
    assert_eq!(t["d.k7x"], "a\n");
    assert!(!t.contains_key("c.k7x"));
}

// A file written at a moved file's old path, the old path sorting before the new: the move takes the old file.
#[test]
fn a_write_at_a_moved_files_old_path_lands_after_the_move() {
    let (_s, dir) = seeded("e-move-order");
    let mut fs = stage(&dir);
    fs.rename(&format!("{dir}/a.k7x"), &format!("{dir}/z.k7x"))
        .unwrap();
    fs.write_atomic(&format!("{dir}/a.k7x"), b"stub\n").unwrap();
    commit(&Real, &fs.planned().unwrap(), &mut |_| {}).unwrap();
    let t = tree(&dir);
    assert_eq!(t["z.k7x"], "a\n");
    assert_eq!(t["a.k7x"], "stub\n");
}

// A folder made or a file written outside the mounts, even by a path climbing out, is refused.
#[test]
fn a_write_or_a_folder_outside_the_mounts_is_refused() {
    let (s, dir) = seeded("e-confined-write");
    let mut fs = stage(&dir);
    let out = s.path("elsewhere");
    assert!(fs.mkdir_all(&out).is_err());
    assert!(fs.mkdir_all(&format!("{dir}/../../x")).is_err());
    assert!(fs.write_atomic(&format!("{out}/x.k7x"), b"x").is_err());
    assert!(fs.write_atomic(&format!("{dir}/../x.k7x"), b"x").is_err());
    assert!(
        fs.copy(&format!("{dir}/a.k7x"), &format!("{out}/a.k7x"))
            .is_err()
    );
    assert!(fs.is_empty());
}

// A set holding a text, a move and a removal fails at any step of its landing and the disk returns to what it was.
#[test]
fn a_set_with_a_move_rolls_back_at_every_step() {
    let (_s, dir) = seeded("e-take-rollback");
    let before = tree(&dir);
    for at in 0..10 {
        let mut fs = stage(&dir);
        fs.write_atomic(&format!("{dir}/n.k7x"), b"new\n").unwrap();
        fs.rename(&format!("{dir}/m.k7x"), &format!("{dir}/m2.k7x"))
            .unwrap();
        fs.write_atomic(&format!("{dir}/a.k7x"), b"new a\n")
            .unwrap();
        if commit(&Flaky::at(at), &fs.planned().unwrap(), &mut |_| {}).is_ok() {
            break;
        }
        assert_eq!(tree(&dir), before, "fail at step {at}");
    }
}

// On Windows a staged write, a move and a long path land: the stage hands DiskSource `C:/x`, not `/C:/x`.
#[cfg(windows)]
#[test]
fn a_staged_edit_lands_on_a_windows_root() {
    let (_s, dir) = seeded("e-windows");
    let mut fs = stage(&dir);
    let long = format!("{dir}/{}deep.k7x", "dddddddddddddddddddd/".repeat(15));
    fs.write_atomic(&format!("{dir}/a.k7x"), b"new a\n")
        .unwrap();
    fs.rename(&format!("{dir}/m.k7x"), &format!("{dir}/n.k7x"))
        .unwrap();
    fs.mkdir_all(long.rsplit_once('/').unwrap().0).unwrap();
    fs.write_atomic(&long, b"deep\n").unwrap();
    commit(&Real, &fs.planned().unwrap(), &mut |_| {}).unwrap();
    let t = tree(&dir);
    assert_eq!(t["a.k7x"], "new a\n");
    assert_eq!(t["n.k7x"], "m\n");
    assert!(!t.contains_key("m.k7x"));
    assert!(t.keys().any(|k| k.ends_with("deep.k7x")));
}

// A repoint whose root cannot be listed fails, and a root that is absent is skipped.
#[test]
fn a_repoint_fails_on_a_root_it_may_not_list() {
    let (s, dir) = seeded("e-repoint-root");
    let outside = Scratch::new("e-repoint-outside").dir("lib");
    let mut fs = stage(&dir);
    let walk = |fs: &mut StagedFs, root: &str| {
        song_edit::module_move::repoint_refs(fs, &[root.to_owned()], "a", "b", false)
    };
    assert!(walk(&mut fs, &outside).is_err());
    assert_eq!(
        walk(&mut fs, &format!("{dir}/absent")).unwrap(),
        Vec::<String>::new()
    );
    drop(s);
}

// A removal and a move's source are confined to the write mounts: a file of a read mount is never marked gone or moved.
#[test]
fn a_read_mount_is_never_removed_or_moved() {
    let (s, dir) = seeded("e-read-mount");
    s.file("lib/x.k7x", "x\n");
    let lib = s.path("lib/x.k7x");
    let mut fs = StagedFs::reading(&[dir.as_str()], &[s.path("lib").as_str()]).unwrap();
    assert!(fs.read(&lib, Span::All).is_ok());
    assert!(fs.remove(&lib).is_err());
    assert!(fs.remove_tree(&s.path("lib")).is_err());
    assert!(fs.rename(&lib, &format!("{dir}/x.k7x")).is_err());
    assert!(fs.is_empty());
}
