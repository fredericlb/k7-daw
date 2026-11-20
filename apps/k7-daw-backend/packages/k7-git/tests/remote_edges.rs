//! G2: what only the real transports can get wrong: the remote's own default branch and an empty remote, over SSH.

mod common;

use std::path::Path;
use std::sync::Arc;

use common::ssh_keys::HOST_A;
use common::sshd::{Sshd, fingerprint};
use common::{Counting, Dir, authorize_device, bare, have_git, me};
use k7_git::auth::KnownHosts;
use k7_git::{Net, Origin, Repo};

/// A bare remote on `master` holding one commit, pushed by libgit2 over `file://`.
fn master_remote(at: &Path) {
    let mut o = git2::RepositoryInitOptions::new();
    o.bare(true).initial_head("master").mkpath(true);
    git2::Repository::init_opts(at, &o).unwrap();
    let work = Dir::new("edge-seed");
    work.write("a.k7x", b"x\n");
    let seed = Repo::init(work.path(), &work.path().join(".git")).unwrap();
    seed.commit("seed", &me(), "Mac").unwrap();
    let sg = git2::Repository::open(work.path().join(".git")).unwrap();
    let mut r = sg
        .remote("origin", &format!("file://{}", at.display()))
        .unwrap();
    r.push(&["refs/heads/main:refs/heads/master"], None)
        .unwrap();
}

async fn ssh(root: &Dir) -> (Sshd, Arc<Net>, Origin) {
    let server = Sshd::start(root.path().to_path_buf()).await;
    let store = Arc::new(Counting::default());
    authorize_device(&server, &store);
    let hosts = Arc::new(KnownHosts::memory());
    hosts
        .remember("127.0.0.1", server.port(), &fingerprint(HOST_A))
        .unwrap();
    let net = Net::new(
        tokio::runtime::Handle::current(),
        common::tls(),
        store,
        hosts,
    );
    let o = Origin {
        url: server.url("r.git"),
        user: None,
    };
    (server, net, o)
}

#[tokio::test(flavor = "multi_thread")]
async fn over_ssh_a_clone_takes_the_branch_the_remote_calls_default() {
    if !have_git("edge-ssh-master") {
        return;
    }
    let root = Dir::new("edge-root");
    master_remote(&root.path().join("r.git"));
    let (_s, net, o) = ssh(&root).await;
    let into = Dir::new("edge-into");
    let b = into.path().join("c");
    net.clone_into(o, b.clone()).await.unwrap();
    let copy = Repo::open(&b, &b.join(".git")).unwrap();
    assert_eq!(copy.branch().as_deref(), Some("master"));
    assert_eq!(std::fs::read(b.join("a.k7x")).unwrap(), b"x\n");
}

#[tokio::test(flavor = "multi_thread")]
async fn over_ssh_a_clone_of_an_empty_remote_is_an_empty_workspace_that_can_push() {
    if !have_git("edge-ssh-empty") {
        return;
    }
    let root = Dir::new("edge-root");
    bare(&root.path().join("r.git"));
    let (_s, net, o) = ssh(&root).await;
    let into = Dir::new("edge-into");
    let b = into.path().join("c");
    net.clone_into(o.clone(), b.clone()).await.unwrap();
    std::fs::write(b.join("song.k7x"), b"a: 1\n").unwrap();
    let copy = Repo::open(&b, &b.join(".git")).unwrap();
    copy.commit("first", &me(), "Mac").unwrap();
    net.push(b.clone(), b.join(".git"), o).await.unwrap();
    let raw = git2::Repository::open_bare(root.path().join("r.git")).unwrap();
    assert!(raw.refname_to_id("refs/heads/main").is_ok());
}
