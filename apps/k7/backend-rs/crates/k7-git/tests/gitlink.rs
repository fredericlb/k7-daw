//! A workspace once converted to `.git.nosync` behind a `.git` link gets its real `.git` folder back on open.
#![cfg(unix)]

mod common;

use common::Dir;
use k7_git::Repo;

fn converted() -> Dir {
    let d = Dir::new("gitlink");
    Repo::init(d.path(), &d.path().join(".git")).unwrap();
    d.write("song.k7x", b"a");
    std::fs::rename(d.path().join(".git"), d.path().join(".git.nosync")).unwrap();
    std::os::unix::fs::symlink(".git.nosync", d.path().join(".git")).unwrap();
    d
}

fn is_real_git(d: &Dir) -> bool {
    let m = std::fs::symlink_metadata(d.path().join(".git")).unwrap();
    m.is_dir() && !m.file_type().is_symlink() && !d.path().join(".git.nosync").exists()
}

#[test]
fn an_open_turns_the_link_back_into_a_real_git_folder() {
    let d = converted();
    Repo::open(d.path(), &d.path().join(".git")).unwrap();
    assert!(is_real_git(&d));
    assert!(d.path().join(".git/HEAD").is_file());
}

#[test]
fn a_second_open_changes_nothing() {
    let d = converted();
    assert!(k7_git::restore_gitdir(d.path()).unwrap());
    assert!(!k7_git::restore_gitdir(d.path()).unwrap());
    Repo::open(d.path(), &d.path().join(".git")).unwrap();
    assert!(is_real_git(&d));
}

#[test]
fn a_cut_between_the_two_steps_is_finished_by_the_next_open() {
    let d = converted();
    std::fs::remove_file(d.path().join(".git")).unwrap();
    Repo::open(d.path(), &d.path().join(".git")).unwrap();
    assert!(is_real_git(&d));
}

#[test]
fn a_real_git_folder_beside_a_nosync_folder_is_never_touched() {
    let d = Dir::new("gitlink-real");
    Repo::init(d.path(), &d.path().join(".git")).unwrap();
    std::fs::create_dir(d.path().join(".git.nosync")).unwrap();
    d.write(".git.nosync/keep", b"mine");
    assert!(!k7_git::restore_gitdir(d.path()).unwrap());
    assert_eq!(
        std::fs::read(d.path().join(".git.nosync/keep")).unwrap(),
        b"mine"
    );
}

#[test]
fn another_link_is_the_persons_and_stays() {
    let d = Dir::new("gitlink-other");
    std::fs::create_dir(d.path().join("elsewhere")).unwrap();
    std::os::unix::fs::symlink("elsewhere", d.path().join(".git")).unwrap();
    assert!(!k7_git::restore_gitdir(d.path()).unwrap());
    assert!(
        std::fs::symlink_metadata(d.path().join(".git"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
}
