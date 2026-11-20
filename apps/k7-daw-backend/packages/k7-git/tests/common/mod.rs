//! Shared test helpers: a temp folder that goes with the test, the identity, the git binary, and a `Net`.
#![allow(dead_code)]

pub mod httpd;
pub mod lfs_rig;
pub mod lfsd;
pub mod merge_fx;
pub mod ssh_keys;
pub mod sshd;

use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use k7_git::auth::{KnownHosts, MemorySecrets, Secrets};
use k7_git::{Identity, Net, Repo};

pub struct Dir(pub PathBuf);

static N: AtomicU32 = AtomicU32::new(0);

impl Dir {
    pub fn new(tag: &str) -> Dir {
        let n = N.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("k7-git-{tag}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Dir(p)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    pub fn write(&self, rel: &str, bytes: &[u8]) {
        let f = self.0.join(rel);
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(f, bytes).unwrap();
    }

    pub fn remove(&self, rel: &str) {
        std::fs::remove_file(self.0.join(rel)).unwrap();
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub fn me() -> Identity {
    Identity {
        name: "Ada".into(),
        email: "ada@example.com".into(),
    }
}

/// The paths a commit's tree holds, sorted.
pub fn tree_paths(gitdir: &Path, commit: &str) -> Vec<String> {
    let r = git2::Repository::open(gitdir).unwrap();
    let c = r.find_commit(git2::Oid::from_str(commit).unwrap()).unwrap();
    let mut out = Vec::new();
    c.tree()
        .unwrap()
        .walk(git2::TreeWalkMode::PreOrder, |dir, e| {
            if e.kind() == Some(git2::ObjectType::Blob) {
                out.push(format!("{dir}{}", e.name().unwrap()));
            }
            0
        })
        .unwrap();
    out.sort();
    out
}

pub fn blob_at(gitdir: &Path, commit: &str, path: &str) -> Vec<u8> {
    let r = git2::Repository::open(gitdir).unwrap();
    let c = r.find_commit(git2::Oid::from_str(commit).unwrap()).unwrap();
    let e = c.tree().unwrap().get_path(Path::new(path)).unwrap();
    r.find_blob(e.id()).unwrap().content().to_vec()
}

/// Whether the `git` binary the CGI and SSH servers run is there; a test without it says so and passes.
pub fn have_git(test: &str) -> bool {
    let ok = std::process::Command::new("git")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
    if !ok {
        eprintln!("skipped {test}: no git binary");
    }
    ok
}

pub fn tls() -> Arc<rustls::ClientConfig> {
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let cfg = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_root_certificates(roots)
    .with_no_client_auth();
    Arc::new(cfg)
}

/// A store that counts how often a secret is read, so a test can say "never" and "once".
#[derive(Default)]
pub struct Counting {
    pub inner: MemorySecrets,
    pub gets: AtomicUsize,
    pub asked: Mutex<Vec<String>>,
}

impl Secrets for Counting {
    fn get(&self, host: &str, user: &str) -> io::Result<Option<String>> {
        self.gets.fetch_add(1, Ordering::SeqCst);
        self.asked.lock().unwrap().push(format!("{user}@{host}"));
        self.inner.get(host, user)
    }

    fn set(&self, host: &str, user: &str, secret: &str) -> io::Result<()> {
        self.inner.set(host, user, secret)
    }

    fn delete(&self, host: &str, user: &str) -> io::Result<()> {
        self.inner.delete(host, user)
    }
}

pub fn net_with(secrets: Arc<dyn Secrets>, hosts: KnownHosts, allow_file: bool) -> Arc<Net> {
    Net::new(
        tokio::runtime::Handle::current(),
        tls(),
        secrets,
        Arc::new(hosts),
    )
    .with_file(allow_file)
}

pub fn net(allow_file: bool) -> Arc<Net> {
    net_with(
        Arc::new(MemorySecrets::default()),
        KnownHosts::memory(),
        allow_file,
    )
}

/// An empty bare repository on the branch `main`.
pub fn bare(at: &Path) {
    let mut o = git2::RepositoryInitOptions::new();
    o.bare(true).initial_head("main").mkpath(true);
    git2::Repository::init_opts(at, &o).unwrap();
}

/// A workspace with one commit of `files`, its git folder inside it.
pub fn workspace(files: &[(&str, &[u8])]) -> (Dir, Repo) {
    let d = Dir::new("ws");
    let repo = Repo::init(d.path(), &d.path().join(".git")).unwrap();
    for (p, b) in files {
        d.write(p, b);
    }
    if !files.is_empty() {
        repo.commit("first", &me(), "Mac").unwrap();
    }
    (d, repo)
}

/// A bare `remote` holding one commit of `files` on `main`, made with libgit2 alone (no k7 ignore file, no filters).
pub fn seed_raw(remote: &Path, files: &[(&str, &[u8])]) {
    bare(remote);
    let work = Dir::new("seed");
    let raw = git2::Repository::init(work.path()).unwrap();
    let mut idx = raw.index().unwrap();
    for (p, b) in files {
        work.write(p, b);
        idx.add_path(Path::new(p)).unwrap();
    }
    idx.write().unwrap();
    let tree = raw.find_tree(idx.write_tree().unwrap()).unwrap();
    let sig = git2::Signature::now("Raw", "raw@example.com").unwrap();
    raw.commit(Some("refs/heads/main"), &sig, &sig, "raw", &tree, &[])
        .unwrap();
    let mut r = raw
        .remote("origin", &format!("file://{}", remote.display()))
        .unwrap();
    r.push(&["refs/heads/main:refs/heads/main"], None).unwrap();
}

/// Makes the device's key in `store` and lets `server` sign it in; what the store was asked is counted from zero after.
pub fn authorize_device(server: &sshd::Sshd, store: &Counting) {
    server.authorize(&k7_git::auth::public_key(store).unwrap());
    store.gets.store(0, Ordering::SeqCst);
    store.asked.lock().unwrap().clear();
}
