//! L1: the clean step, the pointer in git, the store, status by hash, and the file remote's own LFS store.

mod common;

use common::{Dir, bare, blob_at, me, merge_fx, net, tree_paths, workspace};
use k7_git::lfs::{Attributes, Pointer, store::Store};
use k7_git::{Begin, Change, Error, Origin, Repo};

fn origin(d: &std::path::Path) -> Origin {
    Origin {
        url: format!("file://{}", d.display()),
        user: None,
    }
}

fn pointer_at(w: &Dir, commit: &str, path: &str) -> Option<Pointer> {
    Pointer::parse(&blob_at(&w.path().join(".git"), commit, path))
}

#[test]
fn a_tracked_file_is_a_pointer_in_git_and_its_bytes_are_in_the_store() {
    let (w, repo) = workspace(&[]);
    w.write("take.wav", b"RIFF-the-take");
    w.write("song.k7x", b"a: 1\n");
    let id = repo.commit("one", &me(), "Mac").unwrap();
    let p = pointer_at(&w, &id, "take.wav").expect("a pointer");
    assert_eq!(p.size, 13);
    assert_eq!(
        blob_at(&w.path().join(".git"), &id, "song.k7x"),
        b"a: 1\n",
        "a text file stays plain"
    );
    assert_eq!(
        std::fs::read(Store::in_gitdir(&w.path().join(".git")).path(&p.oid)).unwrap(),
        b"RIFF-the-take"
    );
    assert_eq!(
        std::fs::read(w.path().join("take.wav")).unwrap(),
        b"RIFF-the-take",
        "the work tree is untouched"
    );
}

#[test]
fn status_judges_a_large_file_by_its_hash_not_by_its_bytes_or_its_time() {
    let (w, repo) = workspace(&[]);
    w.write("take.wav", b"RIFF-the-take");
    repo.commit("one", &me(), "Mac").unwrap();
    assert!(repo.status().unwrap().is_empty());
    let later = std::time::SystemTime::now() + std::time::Duration::from_secs(30);
    std::fs::File::options()
        .write(true)
        .open(w.path().join("take.wav"))
        .unwrap()
        .set_modified(later)
        .unwrap();
    assert!(
        repo.status().unwrap().is_empty(),
        "a new mtime with the same bytes is no change: {:?}",
        repo.status()
    );
    w.write("take.wav", b"RIFF-the-take-2");
    let st = repo.status().unwrap();
    assert_eq!(
        (st[0].change, st[0].lfs, st[0].size),
        (Change::Modified, true, 15)
    );
    w.remove("take.wav");
    let st = repo.status().unwrap();
    assert_eq!(
        (st[0].change, st[0].size, st[0].binary),
        (Change::Removed, 13, true),
        "a removal reports the file's size, not the pointer's"
    );
}

#[test]
fn a_large_binary_of_an_unforeseen_type_gets_its_own_line_and_a_large_text_does_not() {
    let (w, repo) = workspace(&[]);
    let mut big = vec![0u8; 1_200_000];
    big[7] = 9;
    w.write("kit/sample.dat", &big);
    w.write("notes.txt", &vec![b'a'; 1_200_000]);
    w.write("small.dat", &vec![0u8; 100_000]);
    let id = repo.commit("one", &me(), "Mac").unwrap();
    assert!(pointer_at(&w, &id, "kit/sample.dat").is_some());
    assert!(pointer_at(&w, &id, "notes.txt").is_none());
    assert!(pointer_at(&w, &id, "small.dat").is_none());
    let text = std::fs::read_to_string(w.path().join(".gitattributes")).unwrap();
    assert!(text.contains("\"kit/sample.dat\" filter=lfs"), "{text}");
    assert!(Attributes::read(w.path()).tracks("kit/sample.dat"));
    assert!(
        tree_paths(&w.path().join(".git"), &id).contains(&".gitattributes".to_string()),
        "the line travels in the same commit"
    );
    assert!(repo.status().unwrap().is_empty());
}

#[tokio::test]
async fn a_push_to_a_file_remote_carries_the_bytes_and_the_store_keeps_only_what_the_work_tree_lacks()
 {
    let remote = Dir::new("remote");
    bare(remote.path());
    let (w, repo) = workspace(&[]);
    w.write("take.wav", b"RIFF-take-one");
    repo.commit("one", &me(), "Mac").unwrap();
    let n = net(true);
    n.push(
        w.path().to_path_buf(),
        w.path().join(".git"),
        origin(remote.path()),
    )
    .await
    .unwrap();
    let local = Store::in_gitdir(&w.path().join(".git"));
    let theirs = Store::in_gitdir(remote.path());
    let p = repo.lfs_head().unwrap().remove(0).1;
    assert!(theirs.has(&p.oid), "the host holds the bytes");
    assert!(
        !local.has(&p.oid),
        "the bytes are on the host and in the work tree: the store lets go"
    );
    w.write("take.wav", b"RIFF-take-two-edited");
    repo.commit("two", &me(), "Mac").unwrap();
    w.write("take.wav", b"RIFF-take-three-uncommitted");
    n.push(
        w.path().to_path_buf(),
        w.path().join(".git"),
        origin(remote.path()),
    )
    .await
    .unwrap();
    let two = repo.lfs_head().unwrap().remove(0).1;
    assert!(theirs.has(&two.oid));
    assert!(
        local.has(&two.oid),
        "the work tree holds other bytes than commit two: its object stays"
    );
}

#[tokio::test]
async fn a_clone_brings_the_bytes_of_the_checked_out_commit_before_it_ends() {
    let remote = Dir::new("remote");
    bare(remote.path());
    let (w, _repo) = workspace(&[
        ("take.wav", b"RIFF-take"),
        ("deep/more.flac", b"fLaC-more"),
        ("a.k7x", b"x\n"),
    ]);
    let n = net(true);
    n.push(
        w.path().to_path_buf(),
        w.path().join(".git"),
        origin(remote.path()),
    )
    .await
    .unwrap();
    let into = Dir::new("into");
    let b = into.path().join("c");
    n.clone_into(origin(remote.path()), b.clone())
        .await
        .unwrap();
    assert_eq!(std::fs::read(b.join("take.wav")).unwrap(), b"RIFF-take");
    assert_eq!(
        std::fs::read(b.join("deep/more.flac")).unwrap(),
        b"fLaC-more"
    );
    let copy = Repo::open(&b, &b.join(".git")).unwrap();
    assert!(copy.status().unwrap().is_empty(), "{:?}", copy.status());
    assert!(
        Store::in_gitdir(&b.join(".git")).list().is_empty(),
        "held by the work tree and the host: not kept twice"
    );
}

#[tokio::test]
async fn a_clone_whose_object_is_not_on_the_host_is_offline_naming_the_file_and_leaves_nothing() {
    let remote = Dir::new("remote");
    bare(remote.path());
    let (w, repo) = workspace(&[("take.wav", b"RIFF-take")]);
    let n = net(true);
    n.push(
        w.path().to_path_buf(),
        w.path().join(".git"),
        origin(remote.path()),
    )
    .await
    .unwrap();
    let p = repo.lfs_head().unwrap().remove(0).1;
    Store::in_gitdir(remote.path()).remove(&p.oid).unwrap();
    let into = Dir::new("into");
    let err = n
        .clone_into(origin(remote.path()), into.path().join("c"))
        .await
        .unwrap_err();
    assert_eq!(
        err,
        Error::Offline {
            path: "take.wav".into()
        }
    );
    assert_eq!(err.code(), "offline");
    assert!(std::fs::read_dir(into.path()).unwrap().next().is_none());
}

#[tokio::test]
async fn a_file_removed_from_the_work_tree_is_pushed_from_the_store_or_refused_naming_it() {
    let remote = Dir::new("remote");
    bare(remote.path());
    let (w, repo) = workspace(&[("take.wav", b"RIFF-take")]);
    let p = repo.lfs_head().unwrap().remove(0).1;
    Store::in_gitdir(&w.path().join(".git"))
        .remove(&p.oid)
        .unwrap();
    w.remove("take.wav");
    let err = net(true)
        .push(
            w.path().to_path_buf(),
            w.path().join(".git"),
            origin(remote.path()),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(&err, Error::Network(m) if m.contains("take.wav")),
        "{err:?}"
    );
    assert!(
        git2::Repository::open_bare(remote.path())
            .unwrap()
            .head()
            .is_err()
            || git2::Repository::open_bare(remote.path())
                .unwrap()
                .references()
                .unwrap()
                .count()
                == 0,
        "nothing was pushed"
    );
}

#[test]
fn prune_hashes_the_file_afresh_and_never_follows_a_link() {
    let (w, repo) = workspace(&[]);
    w.write("take.wav", b"RIFF-aaaa");
    repo.commit("one", &me(), "Mac").unwrap();
    let files = repo.lfs_head().unwrap();
    let store = Store::in_gitdir(&w.path().join(".git"));
    assert!(store.has(&files[0].1.oid));
    // Other bytes of the same size, the time put back as `cp -p` and `rsync -t` do.
    let at = std::fs::metadata(w.path().join("take.wav"))
        .unwrap()
        .modified()
        .unwrap();
    w.write("take.wav", b"RIFF-bbbb");
    std::fs::File::options()
        .write(true)
        .open(w.path().join("take.wav"))
        .unwrap()
        .set_modified(at)
        .unwrap();
    repo.lfs_prune(&files);
    assert!(
        store.has(&files[0].1.oid),
        "the work tree holds other bytes: the object stays"
    );
    #[cfg(unix)]
    {
        std::fs::remove_file(w.path().join("take.wav")).unwrap();
        std::os::unix::fs::symlink(store.path(&files[0].1.oid), w.path().join("take.wav")).unwrap();
        repo.lfs_prune(&files);
        assert!(
            store.has(&files[0].1.oid),
            "a link to the object is not a file that holds it"
        );
    }
    std::fs::remove_file(w.path().join("take.wav")).unwrap();
    w.write("take.wav", b"RIFF-aaaa");
    repo.lfs_prune(&files);
    assert!(
        !store.has(&files[0].1.oid),
        "the same bytes: the object goes"
    );
}

#[test]
fn a_work_file_that_already_is_a_pointer_is_stored_as_that_pointer() {
    let (w, repo) = workspace(&[]);
    let own = Pointer {
        oid: "ab".repeat(32),
        size: 5,
    };
    w.write("take.wav", own.text().as_bytes());
    let id = repo.commit("one", &me(), "Mac").unwrap();
    assert_eq!(pointer_at(&w, &id, "take.wav"), Some(own));
    assert!(
        Store::in_gitdir(&w.path().join(".git")).list().is_empty(),
        "no object was made of the pointer text"
    );
}

#[tokio::test]
async fn a_merge_never_prunes_a_take_the_host_does_not_have_and_the_push_still_goes() {
    let f = merge_fx::diverge(
        &[("a.k7x", b"x: 1\n")],
        &[("take.wav", b"RIFF-v1")],
        &[("a.k7x", b"x: 2\n")],
    )
    .await;
    let v1 = f.repo_b.lfs_head().unwrap().remove(0).1;
    f.repo_b.merge_begin(&f.theirs).unwrap();
    f.repo_b
        .merge_resolve(&[], &k7_git::Direct(f.b.clone()), &me(), "B")
        .unwrap();
    f.repo_b.lfs_prune_held().unwrap();
    assert!(
        Store::in_gitdir(&f.b.join(".git")).has(&v1.oid),
        "the host lacks it: it stays"
    );
    f.b_dir.write("c/take.wav", b"RIFF-v2");
    f.repo_b.commit("v2", &me(), "B").unwrap();
    f.net
        .push(f.b.clone(), f.b.join(".git"), f.to.clone())
        .await
        .unwrap();
    let host = Store::in_gitdir(f.remote.path());
    assert!(host.has(&v1.oid), "the host holds v1 and v2");
    assert_eq!(host.list().len(), 2);
}

#[tokio::test]
async fn a_push_after_a_merge_does_not_ask_for_the_bytes_of_their_files() {
    let f = merge_fx::diverge(
        &[("a.k7x", b"x: 1\n")],
        &[("mine.k7x", b"m: 1\n")],
        &[("their.wav", b"RIFF-theirs")],
    )
    .await;
    let theirs = f.repo_a.lfs_head().unwrap().remove(0).1;
    let Begin::Merge(s) = f.repo_b.merge_begin(&f.theirs).unwrap() else {
        panic!("not a merge");
    };
    f.net
        .blocking({
            let (net, to, b, files) = (f.net.clone(), f.to.clone(), f.b.clone(), s.lfs.clone());
            move || Repo::open(&b, &b.join(".git"))?.lfs_fetch_objects(&net, &to, &files)
        })
        .await
        .unwrap();
    f.repo_b
        .merge_resolve(&[], &k7_git::Direct(f.b.clone()), &me(), "B")
        .unwrap();
    f.repo_b.lfs_prune_held().unwrap();
    let store = Store::in_gitdir(&f.b.join(".git"));
    assert!(!store.has(&theirs.oid), "pruned: the host has it");
    f.b_dir.remove("c/their.wav");
    f.repo_b.commit("drop it", &me(), "B").unwrap();
    f.net
        .push(f.b.clone(), f.b.join(".git"), f.to.clone())
        .await
        .unwrap();
}
