//! The Mac's two keys over SSH: the shared key first, the Mac's old key once when a host refuses it; skipped without `git`.

mod common;

use std::sync::Arc;
use std::sync::atomic::Ordering;

use common::ssh_keys::HOST_A;
use common::sshd::{Sshd, fingerprint};
use common::{Dir, bare, have_git, workspace};
use k7_git::auth::{self, KnownHosts, MemorySecrets, Synced, is_shared};
use k7_git::{Error, Net, Origin};

#[tokio::test(flavor = "multi_thread")]
async fn a_host_that_knows_only_the_old_key_takes_it_until_it_gets_the_shared_one() {
    if !have_git("shared-key") {
        return;
    }
    let root = Dir::new("shared-key-root");
    bare(&root.path().join("r.git"));
    let server = Sshd::start(root.path().to_path_buf()).await;
    let (synced, local) = (MemorySecrets::default(), MemorySecrets::default());
    let phone = auth::public_key(&synced).unwrap();
    let own = auth::public_key(&local).unwrap();
    server.authorize(&own);
    let store = Arc::new(Synced {
        synced,
        local,
        travels: is_shared,
        keeps_old: true,
    });
    assert_eq!(auth::public_key(&*store).unwrap(), phone, "adopted");
    let hosts = Arc::new(KnownHosts::memory());
    hosts
        .remember("127.0.0.1", server.port(), &fingerprint(HOST_A))
        .unwrap();
    let net = Net::new(
        tokio::runtime::Handle::current(),
        common::tls(),
        store.clone(),
        hosts,
    );
    let to = || Origin {
        url: server.url("r.git"),
        user: None,
    };
    let (a, _repo) = workspace(&[("a", b"1")]);
    let at = format!("127.0.0.1:{}", server.port());
    net.fetch(a.path().to_path_buf(), a.path().join(".git"), to())
        .await
        .unwrap();
    assert_eq!(
        server.attempts.load(Ordering::SeqCst),
        2,
        "shared, then old"
    );
    assert_eq!(net.old_key_hosts(), vec![at.clone()]);
    // Once the host holds the shared key too, the old one is no longer needed there.
    server.authorize(&phone);
    net.fetch(a.path().to_path_buf(), a.path().join(".git"), to())
        .await
        .unwrap();
    assert!(net.old_key_hosts().is_empty());
    assert_eq!(server.attempts.load(Ordering::SeqCst), 3);
}

#[tokio::test(flavor = "multi_thread")]
async fn without_the_old_key_a_host_that_knows_only_it_refuses() {
    if !have_git("shared-key-forgotten") {
        return;
    }
    let root = Dir::new("shared-key-root");
    bare(&root.path().join("r.git"));
    let server = Sshd::start(root.path().to_path_buf()).await;
    let (synced, local) = (MemorySecrets::default(), MemorySecrets::default());
    auth::public_key(&synced).unwrap();
    server.authorize(&auth::public_key(&local).unwrap());
    let store = Arc::new(Synced {
        synced,
        local,
        travels: is_shared,
        keeps_old: true,
    });
    auth::public_key(&*store).unwrap();
    assert!(auth::forget_old_key(&*store).unwrap());
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
    let (a, _repo) = workspace(&[("a", b"1")]);
    let origin = Origin {
        url: server.url("r.git"),
        user: None,
    };
    let err = net
        .fetch(a.path().to_path_buf(), a.path().join(".git"), origin)
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Auth(_)), "{err:?}");
    assert_eq!(server.attempts.load(Ordering::SeqCst), 1);
}
