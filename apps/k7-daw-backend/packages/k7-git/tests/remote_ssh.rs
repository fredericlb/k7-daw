//! G2: the SSH subtransport against an in-process russh server running `git upload-pack`; skipped without `git`.

mod common;

use std::sync::Arc;
use std::sync::atomic::Ordering;

use common::ssh_keys::{HOST_A, HOST_B};
use common::sshd::{Sshd, fingerprint};
use common::{Counting, Dir, authorize_device, bare, have_git, me, workspace};
use k7_git::auth::KnownHosts;
use k7_git::{Error, Net, Origin, Repo};

struct Rig {
    _root: Dir,
    server: Sshd,
    store: Arc<Counting>,
    hosts: Arc<KnownHosts>,
    net: Arc<Net>,
}

async fn rig(trust: bool, known: bool) -> Rig {
    let root = Dir::new("ssh-root");
    bare(&root.path().join("r.git"));
    let server = Sshd::start(root.path().to_path_buf()).await;
    let store = Arc::new(Counting::default());
    if known {
        authorize_device(&server, &store);
    }
    let hosts = Arc::new(KnownHosts::memory());
    if trust {
        hosts
            .remember("127.0.0.1", server.port(), &fingerprint(HOST_A))
            .unwrap();
    }
    let net = Net::new(
        tokio::runtime::Handle::current(),
        common::tls(),
        store.clone(),
        hosts.clone(),
    );
    Rig {
        _root: root,
        server,
        store,
        hosts,
        net,
    }
}

fn to(r: &Rig) -> Origin {
    Origin {
        url: r.server.url("r.git"),
        user: None,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn push_clone_and_fetch_work_over_ssh_with_the_device_key() {
    if !have_git("ssh") {
        return;
    }
    let r = rig(true, true).await;
    let (a, repo) = workspace(&[("song.k7x", b"a: 1\n")]);
    r.net
        .push(a.path().to_path_buf(), a.path().join(".git"), to(&r))
        .await
        .unwrap();
    let into = Dir::new("into");
    let b = into.path().join("copy");
    r.net.clone_into(to(&r), b.clone()).await.unwrap();
    assert_eq!(std::fs::read(b.join("song.k7x")).unwrap(), b"a: 1\n");
    a.write("song.k7x", b"a: 2\n");
    repo.commit("two", &me(), "Mac").unwrap();
    r.net
        .push(a.path().to_path_buf(), a.path().join(".git"), to(&r))
        .await
        .unwrap();
    r.net
        .fetch(b.clone(), b.join(".git"), to(&r))
        .await
        .unwrap();
    assert_eq!(
        Repo::open(&b, &b.join(".git"))
            .unwrap()
            .ahead_behind()
            .unwrap(),
        Some((0, 1))
    );
    let cmds = r.server.commands.lock().unwrap().clone();
    assert!(
        cmds.iter().any(|c| c == "git-receive-pack '/r.git'"),
        "{cmds:?}"
    );
    assert!(
        cmds.iter().any(|c| c == "git-upload-pack '/r.git'"),
        "{cmds:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unknown_host_stops_before_any_credential_and_names_its_fingerprint() {
    if !have_git("ssh-unknown") {
        return;
    }
    let r = rig(false, true).await;
    let (a, _repo) = workspace(&[("a", b"1")]);
    let err = r
        .net
        .fetch(a.path().to_path_buf(), a.path().join(".git"), to(&r))
        .await
        .unwrap_err();
    assert_eq!(
        err,
        Error::UnknownHost {
            fingerprint: fingerprint(HOST_A)
        }
    );
    assert_eq!(err.code(), "unknown-host");
    assert_eq!(r.server.attempts.load(Ordering::SeqCst), 0);
    assert_eq!(
        r.store.gets.load(Ordering::SeqCst),
        0,
        "the key was never read"
    );
    // The person trusts it: the same call now goes through.
    r.hosts
        .remember("127.0.0.1", r.server.port(), &fingerprint(HOST_A))
        .unwrap();
    r.net
        .fetch(a.path().to_path_buf(), a.path().join(".git"), to(&r))
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_changed_host_key_sends_no_credential() {
    if !have_git("ssh-changed") {
        return;
    }
    let r = rig(true, true).await;
    r.server.change_host_key();
    let (a, _repo) = workspace(&[("a", b"1")]);
    let err = r
        .net
        .push(a.path().to_path_buf(), a.path().join(".git"), to(&r))
        .await
        .unwrap_err();
    assert_eq!(
        err,
        Error::HostKeyChanged {
            expected: fingerprint(HOST_A),
            got: fingerprint(HOST_B)
        }
    );
    assert_eq!(err.code(), "host-key-changed");
    assert_eq!(
        r.server.attempts.load(Ordering::SeqCst),
        0,
        "no sign-in was tried"
    );
    assert_eq!(
        r.store.gets.load(Ordering::SeqCst),
        0,
        "the key was never read"
    );
    assert_eq!(
        r.hosts.get("127.0.0.1", r.server.port()).unwrap(),
        Some(fingerprint(HOST_A)),
        "the listed key is never replaced"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_server_that_lacks_the_device_key_is_auth_and_the_key_shown_is_the_key_that_signs_in() {
    if !have_git("ssh-auth") {
        return;
    }
    let r = rig(true, false).await;
    let (a, _repo) = workspace(&[("a", b"1")]);
    let err = r
        .net
        .fetch(a.path().to_path_buf(), a.path().join(".git"), to(&r))
        .await
        .unwrap_err();
    assert!(
        matches!(&err, Error::Auth(m) if m.contains("public key")),
        "{err:?}"
    );
    assert_eq!(r.server.attempts.load(Ordering::SeqCst), 1);
    // The key made on that first need is the one the person is shown, and once added the same call goes through.
    r.server
        .authorize(&k7_git::auth::public_key(&*r.store).unwrap());
    r.net
        .fetch(a.path().to_path_buf(), a.path().join(".git"), to(&r))
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_push_behind_over_ssh_is_behind() {
    if !have_git("ssh-behind") {
        return;
    }
    let r = rig(true, true).await;
    let (a, repo_a) = workspace(&[("a", b"1")]);
    r.net
        .push(a.path().to_path_buf(), a.path().join(".git"), to(&r))
        .await
        .unwrap();
    let into = Dir::new("into");
    let b = into.path().join("b");
    r.net.clone_into(to(&r), b.clone()).await.unwrap();
    a.write("a", b"from A");
    repo_a.commit("A", &me(), "Mac").unwrap();
    r.net
        .push(a.path().to_path_buf(), a.path().join(".git"), to(&r))
        .await
        .unwrap();
    std::fs::write(b.join("a"), b"from B").unwrap();
    Repo::open(&b, &b.join(".git"))
        .unwrap()
        .commit("B", &me(), "Phone")
        .unwrap();
    assert_eq!(
        r.net.push(b.clone(), b.join(".git"), to(&r)).await,
        Err(Error::Behind)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_server_that_goes_silent_ends_the_call_within_the_idle_bound() {
    if !have_git("ssh-idle") {
        return;
    }
    let r = rig(true, true).await;
    r.server
        .stall
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let quick = r
        .net
        .clone()
        .with_idle(std::time::Duration::from_millis(400));
    let (a, _repo) = workspace(&[("a", b"1")]);
    let started = std::time::Instant::now();
    let err = quick
        .fetch(a.path().to_path_buf(), a.path().join(".git"), to(&r))
        .await
        .unwrap_err();
    assert!(
        started.elapsed() < std::time::Duration::from_secs(10),
        "{:?}",
        started.elapsed()
    );
    assert!(matches!(err, Error::Git(_) | Error::Network(_)), "{err:?}");
}
