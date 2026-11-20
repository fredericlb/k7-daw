//! A pull whose names differ from a tracked file by case alone must never write over it on a case-insensitive volume.

mod common;
use common::{me, workspace};
use k7_git::{Direct, Error};

/// Whether the temp folder reads `A` and `a` as one name; the cases below need that to be true.
fn folds_case(dir: &std::path::Path) -> bool {
    std::fs::write(dir.join("k7-probe-a"), b"").unwrap();
    let same = dir.join("K7-PROBE-A").exists();
    std::fs::remove_file(dir.join("k7-probe-a")).unwrap();
    same
}

/// A commit parented on HEAD holding `song.k7x` and `Song.k7x`, made without touching the work tree.
fn theirs_with_both(d: &std::path::Path) -> String {
    let raw = git2::Repository::open(d).unwrap();
    let head = raw.head().unwrap().peel_to_commit().unwrap();
    let blob = raw.blob(b"theirs\n").unwrap();
    let entry = |path: &str, id| git2::IndexEntry {
        ctime: git2::IndexTime::new(0, 0),
        mtime: git2::IndexTime::new(0, 0),
        dev: 0,
        ino: 0,
        mode: 0o100644,
        uid: 0,
        gid: 0,
        file_size: 7,
        id,
        flags: 0,
        flags_extended: 0,
        path: path.as_bytes().to_vec(),
    };
    let mine = head.tree().unwrap().get_name("song.k7x").unwrap().id();
    let mut idx = git2::Index::new().unwrap();
    idx.add(&entry("song.k7x", mine)).unwrap();
    idx.add(&entry("Song.k7x", blob)).unwrap();
    let tree = raw.find_tree(idx.write_tree_to(&raw).unwrap()).unwrap();
    let sig = git2::Signature::now("T", "t@example.com").unwrap();
    raw.commit(None, &sig, &sig, "theirs", &tree, &[&head])
        .unwrap()
        .to_string()
}

#[test]
fn a_pull_adding_a_name_that_differs_by_case_is_refused_and_the_local_file_stays() {
    let (d, repo) = workspace(&[("song.k7x", b"mine\n")]);
    if !folds_case(d.path()) {
        return;
    }
    let theirs = theirs_with_both(d.path());
    let err = repo.merge_or_forward(&theirs, &Direct(d.path().into()));
    assert!(matches!(err, Err(Error::CaseClash { .. })), "{err:?}");
    assert_eq!(std::fs::read(d.path().join("song.k7x")).unwrap(), b"mine\n");
    d.write("other.txt", b"x");
    repo.commit("mine moves on", &me(), "Mac").unwrap();
    let err = repo.merge_begin(&theirs);
    assert!(matches!(err, Err(Error::CaseClash { .. })), "{err:?}");
    assert_eq!(std::fs::read(d.path().join("song.k7x")).unwrap(), b"mine\n");
}
