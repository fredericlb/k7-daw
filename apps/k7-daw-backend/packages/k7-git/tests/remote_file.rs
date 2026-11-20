//! G2: fetch, push and clone against a bare repository over `file://`, and the rules that hold for every transport.

mod common;

use common::{Dir, bare, me, net, workspace};
use k7_git::{Error, Origin, Repo};

fn origin(bare_dir: &std::path::Path) -> Origin {
    Origin {
        url: format!("file://{}", bare_dir.display()),
        user: None,
    }
}

async fn push_first(remote: &Origin) -> (common::Dir, Repo) {
    let (d, repo) = workspace(&[("song.k7x", b"a: 1\n"), ("deep/take.wav", b"RIFF-take")]);
    net(true)
        .push(
            d.path().to_path_buf(),
            d.path().join(".git"),
            remote.clone(),
        )
        .await
        .unwrap();
    (d, repo)
}

#[tokio::test]
async fn a_push_then_a_clone_and_a_fetch_round_trip() {
    let remote = Dir::new("remote");
    bare(remote.path());
    let to = origin(remote.path());
    let (a, repo) = push_first(&to).await;
    let n = net(true);

    let into = Dir::new("into");
    let b = into.path().join("copy");
    n.clone_into(to.clone(), b.clone()).await.unwrap();
    assert_eq!(
        std::fs::read(b.join("deep/take.wav")).unwrap(),
        b"RIFF-take"
    );
    let copy = Repo::open(&b, &b.join(".git")).unwrap();
    assert_eq!(copy.branch().as_deref(), Some("main"));
    assert!(
        copy.status().unwrap().is_empty(),
        "a clone is clean: {:?}",
        copy.status()
    );
    assert_eq!(copy.ahead_behind().unwrap(), Some((0, 0)));

    a.write("song.k7x", b"a: 2\n");
    repo.commit("second", &me(), "Mac").unwrap();
    n.push(a.path().to_path_buf(), a.path().join(".git"), to.clone())
        .await
        .unwrap();
    n.fetch(b.clone(), b.join(".git"), to).await.unwrap();
    let copy = Repo::open(&b, &b.join(".git")).unwrap();
    assert_eq!(copy.ahead_behind().unwrap(), Some((0, 1)));
    assert_eq!(
        std::fs::read(b.join("song.k7x")).unwrap(),
        b"a: 1\n",
        "a fetch never moves the files"
    );
}

#[tokio::test]
async fn a_push_behind_the_remote_is_behind_and_the_remote_keeps_its_commit() {
    let remote = Dir::new("remote");
    bare(remote.path());
    let to = origin(remote.path());
    let (a, repo_a) = push_first(&to).await;
    let n = net(true);
    let into = Dir::new("into");
    let b = into.path().join("b");
    n.clone_into(to.clone(), b.clone()).await.unwrap();

    a.write("song.k7x", b"a: from A\n");
    repo_a.commit("A moves", &me(), "Mac").unwrap();
    n.push(a.path().to_path_buf(), a.path().join(".git"), to.clone())
        .await
        .unwrap();
    let a_tip = repo_a.log(1, None).unwrap()[0].id.clone();

    std::fs::write(b.join("song.k7x"), b"a: from B\n").unwrap();
    let repo_b = Repo::open(&b, &b.join(".git")).unwrap();
    repo_b.commit("B moves", &me(), "Phone").unwrap();
    let err = n
        .push(b.clone(), b.join(".git"), to.clone())
        .await
        .unwrap_err();
    assert_eq!(err, Error::Behind);
    assert_eq!(err.code(), "behind");
    let raw = git2::Repository::open_bare(remote.path()).unwrap();
    assert_eq!(
        raw.refname_to_id("refs/heads/main").unwrap().to_string(),
        a_tip
    );
}

#[tokio::test]
async fn file_remotes_are_refused_unless_allowed() {
    let remote = Dir::new("remote");
    bare(remote.path());
    let to = origin(remote.path());
    let (d, _repo) = workspace(&[("a", b"1")]);
    let closed = net(false);
    let gd = d.path().join(".git");
    assert_eq!(
        closed
            .push(d.path().to_path_buf(), gd.clone(), to.clone())
            .await,
        Err(Error::SshOnly)
    );
    assert_eq!(
        closed.fetch(d.path().to_path_buf(), gd, to.clone()).await,
        Err(Error::SshOnly)
    );
    let into = Dir::new("into");
    assert_eq!(
        closed.clone_into(to, into.path().join("x")).await,
        Err(Error::SshOnly)
    );
    let local = Origin {
        url: remote.path().display().to_string(),
        user: None,
    };
    let path_only = closed.clone_into(local, into.path().join("y")).await;
    assert_eq!(
        path_only,
        Err(Error::SshOnly),
        "a bare path is a file remote too"
    );
    assert!(std::fs::read_dir(into.path()).unwrap().next().is_none());
}

#[tokio::test]
async fn a_failed_clone_leaves_no_workspace() {
    let into = Dir::new("into");
    let ghost = Origin {
        url: format!("file://{}", into.path().join("nothing-here").display()),
        user: None,
    };
    let target = into.path().join("copy");
    assert!(net(true).clone_into(ghost, target.clone()).await.is_err());
    assert!(!target.exists());
    let leftovers: Vec<_> = std::fs::read_dir(into.path())
        .unwrap()
        .flatten()
        .map(|e| e.file_name())
        .collect();
    assert!(
        leftovers.is_empty(),
        "no dot-named sibling stays: {leftovers:?}"
    );
}

#[tokio::test]
async fn a_clone_never_writes_into_a_folder_that_exists() {
    let remote = Dir::new("remote");
    bare(remote.path());
    let into = Dir::new("into");
    std::fs::create_dir(into.path().join("taken")).unwrap();
    std::fs::write(into.path().join("taken/mine.txt"), "mine").unwrap();
    let err = net(true)
        .clone_into(origin(remote.path()), into.path().join("taken"))
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Exists(_)), "{err:?}");
    assert_eq!(
        std::fs::read(into.path().join("taken/mine.txt")).unwrap(),
        b"mine"
    );
}

#[tokio::test]
async fn a_clone_takes_the_remotes_default_branch_and_keeps_its_gitignore() {
    let remote = Dir::new("remote");
    let mut o = git2::RepositoryInitOptions::new();
    o.bare(true).initial_head("master");
    let raw = git2::Repository::init_opts(remote.path(), &o).unwrap();
    let work = Dir::new("seed");
    work.write(".gitignore", b"mine.tmp\n");
    work.write("a.k7x", b"x\n");
    let seed = Repo::init(work.path(), &work.path().join(".git")).unwrap();
    seed.commit("seed", &me(), "Mac").unwrap();
    let sg = git2::Repository::open(work.path().join(".git")).unwrap();
    let tip = sg.refname_to_id("HEAD").unwrap();
    let mut r = sg
        .remote("origin", &format!("file://{}", remote.path().display()))
        .unwrap();
    r.push(&["refs/heads/main:refs/heads/master"], None)
        .unwrap();
    assert_eq!(raw.refname_to_id("refs/heads/master").unwrap(), tip);

    let into = Dir::new("into");
    let b = into.path().join("c");
    net(true)
        .clone_into(origin(remote.path()), b.clone())
        .await
        .unwrap();
    let copy = Repo::open(&b, &b.join(".git")).unwrap();
    assert_eq!(copy.branch().as_deref(), Some("master"));
    assert_eq!(std::fs::read(b.join(".gitignore")).unwrap(), b"mine.tmp\n");
    assert!(copy.status().unwrap().is_empty(), "{:?}", copy.status());
}

#[tokio::test]
async fn a_web_address_with_a_password_in_it_is_refused_and_the_password_is_never_echoed() {
    let (d, _repo) = workspace(&[("a", b"1")]);
    let to = Origin {
        url: "https://ada:hunter2@example.com/r.git".into(),
        user: None,
    };
    let err = net(false)
        .fetch(d.path().to_path_buf(), d.path().join(".git"), to)
        .await
        .unwrap_err();
    assert_eq!(err, Error::SshOnly);
    assert!(!err.to_string().contains("hunter2"));
}

#[tokio::test]
async fn a_clone_of_a_remote_with_no_gitignore_adds_none_and_stays_clean() {
    let remote = Dir::new("remote");
    common::seed_raw(remote.path(), &[("a.k7x", b"x\n")]);
    let into = Dir::new("into");
    let b = into.path().join("c");
    net(true)
        .clone_into(origin(remote.path()), b.clone())
        .await
        .unwrap();
    assert!(!b.join(".gitignore").exists());
    assert!(
        Repo::open(&b, &b.join(".git"))
            .unwrap()
            .status()
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn a_clone_writes_the_bytes_whatever_the_remotes_attributes_say() {
    let remote = Dir::new("remote");
    common::seed_raw(
        remote.path(),
        &[
            (".gitattributes", b"* text eol=crlf\n"),
            ("a.txt", b"x\ny\n"),
        ],
    );
    let into = Dir::new("into");
    let b = into.path().join("c");
    net(true)
        .clone_into(origin(remote.path()), b.clone())
        .await
        .unwrap();
    assert_eq!(std::fs::read(b.join("a.txt")).unwrap(), b"x\ny\n");
    assert!(
        Repo::open(&b, &b.join(".git"))
            .unwrap()
            .status()
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn a_clone_never_plants_a_link_that_leaves_the_workspace() {
    let remote = Dir::new("remote");
    bare(remote.path());
    let raw = git2::Repository::open_bare(remote.path()).unwrap();
    let target = raw.blob(b"/etc").unwrap();
    let file = raw.blob(b"x\n").unwrap();
    let mut tb = raw.treebuilder(None).unwrap();
    tb.insert("link", target, 0o120000).unwrap();
    tb.insert("a.k7x", file, 0o100644).unwrap();
    let tree = raw.find_tree(tb.write().unwrap()).unwrap();
    let sig = git2::Signature::now("Raw", "raw@example.com").unwrap();
    raw.commit(Some("refs/heads/main"), &sig, &sig, "raw", &tree, &[])
        .unwrap();
    let into = Dir::new("into");
    let b = into.path().join("c");
    net(true)
        .clone_into(origin(remote.path()), b.clone())
        .await
        .unwrap();
    let meta = std::fs::symlink_metadata(b.join("link")).unwrap();
    assert!(!meta.file_type().is_symlink(), "a link lands as a file");
    assert!(b.join("a.k7x").is_file());
}

#[tokio::test]
async fn a_clone_removes_a_clone_folder_a_crash_left_but_not_a_fresh_one() {
    let remote = Dir::new("remote");
    common::seed_raw(remote.path(), &[("a.k7x", b"x\n")]);
    let into = Dir::new("into");
    let stale = into.path().join(".old.k7-clone-0badf00d");
    let fresh = into.path().join(".new.k7-clone-0badf00e");
    let other = into.path().join(".mine.k7-clone-notmine");
    for d in [&stale, &fresh, &other] {
        std::fs::create_dir(d).unwrap();
        std::fs::write(d.join("x"), "x").unwrap();
    }
    let old = std::time::SystemTime::now() - std::time::Duration::from_secs(7200);
    std::fs::File::open(&stale)
        .unwrap()
        .set_modified(old)
        .unwrap();
    std::fs::File::open(&other)
        .unwrap()
        .set_modified(old)
        .unwrap();
    net(true)
        .clone_into(origin(remote.path()), into.path().join("c"))
        .await
        .unwrap();
    assert!(!stale.exists(), "a stale clone folder goes");
    assert!(fresh.exists(), "a recent one may be another clone's");
    assert!(other.exists(), "a name that is not k7's is never touched");
}

#[tokio::test]
async fn https_http_git_and_other_schemes_are_refused_as_ssh_only_even_when_files_are_allowed() {
    let open = net(true);
    let (d, _repo) = workspace(&[("a", b"1")]);
    let gd = d.path().join(".git");
    for url in [
        "https://example.com/o/r.git",
        "http://127.0.0.1:1/r.git",
        "git://example.com/o/r.git",
        "ftp://example.com/r.git",
        "https://user:token@example.com/o/r.git",
    ] {
        let to = Origin {
            url: url.into(),
            user: None,
        };
        let err = open
            .fetch(d.path().to_path_buf(), gd.clone(), to)
            .await
            .unwrap_err();
        assert_eq!(err, Error::SshOnly, "{url}");
        assert_eq!(err.code(), "ssh-only");
        assert!(err.to_string().contains("ssh address"), "{err}");
    }
}
