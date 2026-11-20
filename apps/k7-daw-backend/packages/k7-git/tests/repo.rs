//! G1: the repository, its ignore list, status, commit and log, against a folder in the temp directory.

mod common;

use std::sync::Arc;

use common::{Dir, blob_at, me, tree_paths};
use k7_git::{Change, Error, Identity, IdentityStore, Present, Repo};

fn fresh(tag: &str) -> (Dir, Repo) {
    let d = Dir::new(tag);
    let repo = Repo::init(d.path(), &d.path().join(".git")).unwrap();
    (d, repo)
}

#[test]
fn init_makes_main_and_the_ignore_files() {
    let (d, repo) = fresh("init");
    assert_eq!(repo.branch().as_deref(), Some("main"));
    let gi = std::fs::read_to_string(d.path().join(".gitignore")).unwrap();
    assert!(gi.contains(".k7-git.json") && gi.contains("stems/"));
    let ex = std::fs::read_to_string(d.path().join(".git/info/exclude")).unwrap();
    assert!(ex.contains(".*.icloud"));
}

#[test]
fn a_git_folder_elsewhere_writes_no_dot_git_in_the_workspace() {
    let w = Dir::new("ws");
    let g = Dir::new("gd");
    let gitdir = g.path().join("repo");
    let repo = Repo::init(w.path(), &gitdir).unwrap();
    w.write("a.k7x", b"x: 1\n");
    let id = repo.commit("first", &me(), "Mac").unwrap();
    assert!(
        !w.path().join(".git").exists(),
        "no .git file in the workspace"
    );
    let again = Repo::open(w.path(), &gitdir).unwrap();
    assert!(again.status().unwrap().is_empty());
    assert!(tree_paths(&gitdir, &id).contains(&"a.k7x".to_string()));
}

#[test]
fn a_cloned_repository_keeps_its_own_ignore_file_and_k7s_list_still_applies() {
    let d = Dir::new("own-ignore");
    d.write(".gitignore", b"mine.txt\n");
    let repo = Repo::init(d.path(), &d.path().join(".git")).unwrap();
    assert_eq!(
        std::fs::read_to_string(d.path().join(".gitignore")).unwrap(),
        "mine.txt\n"
    );
    d.write("stems/drums.wav", b"RIFF");
    d.write("song.k7/renders/amp.wav", b"RIFF");
    d.write("a.k7x", b"x\n");
    let paths: Vec<_> = repo.status().unwrap().into_iter().map(|e| e.path).collect();
    assert_eq!(paths, vec![".gitattributes", ".gitignore", "a.k7x"]);
}

#[test]
fn every_ignored_name_stays_out_of_status_and_commits() {
    let (d, repo) = fresh("ignored");
    for p in [
        ".k7-git.json",
        ".take.wav.icloud",
        ".a.k7x.123.0badf00d.tmp",
        ".take.wav.k7-part",
        "song.k7/clip.mov.part",
        "song.k7/take.wav.part-3f9a1c",
        "song.k7/peaks.cache",
        ".renders/x-1.wav",
        "song.k7/renders/amp.wav",
        "song.k7/stems/vocals.wav",
        "exports/cut.otio",
        "_recycle_bin/old.k7x",
        "_history/j.log",
        "node_modules/p/i.js",
        "deep/.DS_Store",
        "Thumbs.db",
    ] {
        d.write(p, b"x");
    }
    d.write("song.k7/song.k7x", b"a: 1\n");
    let entries = repo.status().unwrap();
    assert_eq!(
        entries.iter().map(|e| e.path.as_str()).collect::<Vec<_>>(),
        vec![".gitattributes", ".gitignore", "song.k7/song.k7x"]
    );
    let id = repo.commit("one", &me(), "Mac").unwrap();
    assert_eq!(
        tree_paths(d.path().join(".git").as_path(), &id),
        vec![".gitattributes", ".gitignore", "song.k7/song.k7x"]
    );
}

#[test]
fn a_removed_ignored_file_is_never_in_a_commit() {
    let (d, repo) = fresh("rm-ignored");
    d.write("a.k7x", b"1");
    d.write("stems/x.wav", b"w");
    let first = repo.commit("one", &me(), "Mac").unwrap();
    d.remove("stems/x.wav");
    assert_eq!(repo.status().unwrap().len(), 0);
    assert_eq!(
        repo.commit("two", &me(), "Mac"),
        Err(Error::NothingToCommit)
    );
    assert!(
        !tree_paths(&d.path().join(".git"), &first)
            .iter()
            .any(|p| p.contains("stems"))
    );
}

#[test]
fn status_reports_each_kind_of_change_with_size_and_binary() {
    let (d, repo) = fresh("status");
    d.write("keep.txt", b"same\n");
    d.write("edit.txt", b"one\n");
    d.write("gone.txt", b"bye bye\n");
    d.write("old-name.dat", b"RIFF\0\0\0\0data-of-the-take");
    repo.commit("base", &me(), "Mac").unwrap();
    d.write("edit.txt", b"one\ntwo\n");
    d.remove("gone.txt");
    std::fs::rename(d.path().join("old-name.dat"), d.path().join("new-name.dat")).unwrap();
    d.write("fresh.bin", b"a\0b");
    let by: std::collections::BTreeMap<_, _> = repo
        .status()
        .unwrap()
        .into_iter()
        .map(|e| (e.path.clone(), e))
        .collect();
    assert_eq!(by.len(), 4, "{by:?}");
    assert_eq!(
        (
            by["edit.txt"].change,
            by["edit.txt"].size,
            by["edit.txt"].binary
        ),
        (Change::Modified, 8, false)
    );
    assert_eq!(
        (by["gone.txt"].change, by["gone.txt"].size),
        (Change::Removed, 8)
    );
    assert_eq!(by["new-name.dat"].change, Change::Renamed);
    assert_eq!(by["new-name.dat"].from.as_deref(), Some("old-name.dat"));
    assert!(by["new-name.dat"].binary);
    assert_eq!(
        (by["fresh.bin"].change, by["fresh.bin"].binary),
        (Change::Added, true)
    );
}

struct Away(&'static str);

impl Present for Away {
    fn present(&self, rel: &str) -> bool {
        rel != self.0
    }
}

#[test]
fn a_file_not_downloaded_is_unchanged_and_never_committed_as_removed() {
    let (d, mut repo) = fresh("present");
    d.write("take.wav", b"RIFF-take");
    d.write("a.k7x", b"1");
    let first = repo.commit("one", &me(), "Mac").unwrap();
    d.remove("take.wav");
    repo.set_present(Arc::new(Away("take.wav")));
    assert!(repo.status().unwrap().is_empty());
    d.write("a.k7x", b"2");
    let second = repo.commit("two", &me(), "Mac").unwrap();
    assert!(tree_paths(&d.path().join(".git"), &second).contains(&"take.wav".to_string()));
    assert_ne!(first, second);
}

#[test]
fn a_commit_is_the_whole_tree_with_identity_and_device_trailer() {
    let (d, repo) = fresh("commit");
    d.write("a.k7x", b"a: 1\n");
    d.write("deep/b.txt", b"b\n");
    let id = repo
        .commit("First take\n\nwith a body", &me(), "Ada's Mac")
        .unwrap();
    let r = git2::Repository::open(d.path().join(".git")).unwrap();
    let c = r.find_commit(git2::Oid::from_str(&id).unwrap()).unwrap();
    assert_eq!(c.author().name().unwrap(), "Ada");
    assert_eq!(c.author().email().unwrap(), "ada@example.com");
    assert_eq!(
        c.message().unwrap(),
        "First take\n\nwith a body\n\nK7-Device: Ada's Mac\n"
    );
    assert_eq!(c.parent_count(), 0);
    d.write("a.k7x", b"a: 2\n");
    let next = repo.commit("Second", &me(), "Phone").unwrap();
    let c2 = r.find_commit(git2::Oid::from_str(&next).unwrap()).unwrap();
    assert_eq!(c2.parent_id(0).unwrap().to_string(), id);
    assert!(repo.status().unwrap().is_empty());
}

#[test]
fn a_commit_needs_an_identity_and_a_change() {
    let (d, repo) = fresh("refuse");
    d.write("a", b"1");
    let nobody = Identity {
        name: " ".into(),
        email: "a@b".into(),
    };
    assert_eq!(repo.commit("m", &nobody, "Mac"), Err(Error::NoIdentity));
    repo.commit("m", &me(), "Mac").unwrap();
    assert_eq!(repo.commit("m", &me(), "Mac"), Err(Error::NothingToCommit));
}

fn with_large(tag: &str) -> (Dir, Repo) {
    let (d, mut repo) = fresh(tag);
    repo.set_max_file(1000);
    d.write("a.txt", b"1");
    d.write("huge.wav", &[7u8; 1001]);
    (d, repo)
}

#[test]
fn a_file_over_the_limit_is_refused_when_the_commit_asks_for_it() {
    let (_d, repo) = with_large("big");
    match repo.commit("m", &me(), "Mac") {
        Err(Error::TooBig { path, size }) => assert_eq!((path.as_str(), size), ("huge.wav", 1001)),
        other => panic!("{other:?}"),
    }
    assert!(
        repo.log(5, None).unwrap().is_empty(),
        "nothing was committed"
    );
}

#[test]
fn a_commit_leaves_the_large_file_out_now_and_on_later_commits() {
    let (d, repo) = with_large("leave-out");
    let out = repo.too_large().unwrap();
    assert_eq!(out, vec!["huge.wav".to_string()]);
    let id = repo.commit_excluding("m", &me(), "Mac", &out).unwrap();
    let paths = tree_paths(&d.path().join(".git"), &id);
    assert!(paths.contains(&"a.txt".to_string()) && !paths.contains(&"huge.wav".to_string()));
    d.write("b.txt", b"2");
    let id2 = repo
        .commit_excluding("m2", &me(), "Mac", &repo.too_large().unwrap())
        .unwrap();
    assert!(!tree_paths(&d.path().join(".git"), &id2).contains(&"huge.wav".to_string()));
    assert_eq!(
        std::fs::read(d.path().join("huge.wav")).unwrap(),
        vec![7u8; 1001]
    );
}

#[test]
fn status_names_a_large_file_and_nothing_else_is_too_large() {
    let (_d, repo) = with_large("status");
    let flags: Vec<_> = repo
        .status()
        .unwrap()
        .into_iter()
        .map(|e| (e.path, e.too_large))
        .collect();
    assert!(flags.contains(&("huge.wav".into(), true)) && flags.contains(&("a.txt".into(), false)));
    repo.commit_excluding("m", &me(), "Mac", &repo.too_large().unwrap())
        .unwrap();
    let left = repo.status().unwrap();
    assert_eq!(left.len(), 1);
    assert!(left[0].too_large && left[0].path == "huge.wav");
}

#[test]
fn only_a_large_file_may_be_left_out_and_a_commit_of_nothing_else_is_empty() {
    let (_d, repo) = with_large("only");
    assert!(matches!(
        repo.commit_excluding("m", &me(), "Mac", &["a.txt".into()]),
        Err(Error::BadPath(_))
    ));
    repo.commit_excluding("m", &me(), "Mac", &repo.too_large().unwrap())
        .unwrap();
    assert_eq!(
        repo.commit_excluding("m", &me(), "Mac", &repo.too_large().unwrap()),
        Err(Error::NothingToCommit)
    );
}

#[test]
fn bytes_reach_git_unfiltered_whatever_the_attributes_say() {
    let (d, repo) = fresh("bytes");
    d.write(".gitattributes", b"* text eol=lf\n");
    d.write("crlf.txt", b"a\r\nb\r\n");
    let id = repo.commit("m", &me(), "Mac").unwrap();
    assert_eq!(
        blob_at(&d.path().join(".git"), &id, "crlf.txt"),
        b"a\r\nb\r\n"
    );
    assert!(repo.status().unwrap().is_empty());
}

#[cfg(unix)]
#[test]
fn the_executable_bit_is_kept() {
    use std::os::unix::fs::PermissionsExt;
    let (d, repo) = fresh("exec");
    d.write("run.sh", b"#!/bin/sh\n");
    std::fs::set_permissions(
        d.path().join("run.sh"),
        std::fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    let id = repo.commit("m", &me(), "Mac").unwrap();
    let r = git2::Repository::open(d.path().join(".git")).unwrap();
    let c = r.find_commit(git2::Oid::from_str(&id).unwrap()).unwrap();
    assert_eq!(
        c.tree().unwrap().get_name("run.sh").unwrap().filemode(),
        0o100755
    );
}

#[test]
fn the_log_pages_newest_first_with_device_and_changed_files() {
    let (d, repo) = fresh("log");
    assert!(repo.log(10, None).unwrap().is_empty(), "no commit yet");
    let mut ids = Vec::new();
    for i in 0..5 {
        d.write(&format!("f{i}.txt"), b"x");
        d.write("shared.txt", format!("{i}").as_bytes());
        ids.push(
            repo.commit(&format!("take {i}"), &me(), "Studio PC")
                .unwrap(),
        );
    }
    let page = repo.log(2, None).unwrap();
    assert_eq!(
        page.iter().map(|e| e.subject.as_str()).collect::<Vec<_>>(),
        ["take 4", "take 3"]
    );
    assert_eq!(page[0].device.as_deref(), Some("Studio PC"));
    assert_eq!((page[0].author.as_str(), page[0].files_changed), ("Ada", 2));
    let next = repo.log(10, Some(&page[1].id)).unwrap();
    assert_eq!(
        next.iter().map(|e| e.subject.as_str()).collect::<Vec<_>>(),
        ["take 2", "take 1", "take 0"]
    );
    assert_eq!(next[2].id, ids[0]);
    assert!(
        next[2].files_changed >= 3,
        "the first commit lists its files"
    );
}

#[test]
fn ahead_and_behind_read_the_remote_tracking_ref() {
    let (d, repo) = fresh("ahead");
    assert_eq!(repo.ahead_behind().unwrap(), None);
    d.write("a", b"1");
    let first = repo.commit("one", &me(), "Mac").unwrap();
    let raw = git2::Repository::open(d.path().join(".git")).unwrap();
    raw.reference(
        "refs/remotes/origin/main",
        git2::Oid::from_str(&first).unwrap(),
        true,
        "t",
    )
    .unwrap();
    assert_eq!(repo.ahead_behind().unwrap(), Some((0, 0)));
    d.write("a", b"2");
    repo.commit("two", &me(), "Mac").unwrap();
    d.write("a", b"3");
    repo.commit("three", &me(), "Mac").unwrap();
    assert_eq!(repo.ahead_behind().unwrap(), Some((2, 0)));
    let head = raw.refname_to_id("HEAD").unwrap();
    raw.reference(
        "refs/heads/main",
        git2::Oid::from_str(&first).unwrap(),
        true,
        "t",
    )
    .unwrap();
    raw.reference("refs/remotes/origin/main", head, true, "t")
        .unwrap();
    assert_eq!(repo.ahead_behind().unwrap(), Some((0, 2)));
}

#[test]
fn the_identity_is_per_device() {
    let d = Dir::new("ident");
    let store = IdentityStore::in_folder(d.path());
    assert_eq!(store.load().unwrap(), None);
    store
        .save(&Identity {
            name: "Ada".into(),
            email: String::new(),
        })
        .unwrap();
    assert!(!store.load().unwrap().unwrap().is_complete());
    d.write(".k7-git-identity.json", b"{nope");
    assert!(
        store.load().is_err(),
        "a file that does not parse is never nobody"
    );
    std::fs::remove_file(d.path().join(".k7-git-identity.json")).unwrap();
    std::fs::create_dir(d.path().join(".k7-git-identity.json")).unwrap();
    assert!(
        store.load().is_err(),
        "a file that does not read is never nobody"
    );
}
