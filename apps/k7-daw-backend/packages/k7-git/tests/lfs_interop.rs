//! L1: a repository k7 pushed read by the `git-lfs` binary, and the other way. `#[ignore]`: run by hand with
//! `cargo test -p k7-git --test lfs_interop -- --ignored` where `git` and `git-lfs` are installed.

mod common;

use std::path::Path;
use std::process::Command;

use common::{Dir, bare, me, net, workspace};
use k7_git::Origin;

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .args(["-c", "user.name=Raw", "-c", "user.email=raw@example.com"])
        .args(args)
        .output()
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).to_string()
}

fn have_lfs() -> bool {
    Command::new("git")
        .args(["lfs", "version"])
        .output()
        .is_ok_and(|o| o.status.success())
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs git and git-lfs"]
async fn git_lfs_reads_what_k7_pushed() {
    if !have_lfs() {
        return;
    }
    let root = Dir::new("interop-root");
    bare(&root.path().join("r.git"));
    let url = format!("file://{}", root.path().join("r.git").display());
    let n = net(true);
    let (w, _repo) = workspace(&[("take.wav", b"RIFF-k7-wrote-this"), ("a.k7x", b"x\n")]);
    let to = Origin {
        url: url.clone(),
        user: None,
    };
    n.push(w.path().to_path_buf(), w.path().join(".git"), to)
        .await
        .unwrap();
    let into = Dir::new("interop-clone");
    git(into.path(), &["clone", &url, "c"]);
    let c = into.path().join("c");
    git(&c, &["lfs", "install", "--local"]);
    git(&c, &["lfs", "pull"]);
    assert_eq!(
        std::fs::read(c.join("take.wav")).unwrap(),
        b"RIFF-k7-wrote-this"
    );
    assert!(git(&c, &["lfs", "ls-files"]).contains("take.wav"));
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs git and git-lfs"]
async fn k7_reads_what_git_lfs_pushed() {
    if !have_lfs() {
        return;
    }
    let root = Dir::new("interop-root");
    bare(&root.path().join("r.git"));
    let url = format!("file://{}", root.path().join("r.git").display());
    let w = Dir::new("interop-work");
    git(w.path(), &["init", "-b", "main"]);
    git(w.path(), &["lfs", "install", "--local"]);
    git(w.path(), &["lfs", "track", "*.wav"]);
    w.write("take.wav", b"RIFF-git-lfs-wrote-this");
    git(w.path(), &["add", "-A"]);
    git(w.path(), &["commit", "-m", "one"]);
    git(w.path(), &["remote", "add", "origin", &url]);
    git(w.path(), &["push", "origin", "main"]);
    let into = Dir::new("interop-k7");
    let to = Origin {
        url: url.clone(),
        user: None,
    };
    net(true)
        .clone_into(to, into.path().join("c"))
        .await
        .unwrap();
    assert_eq!(
        std::fs::read(into.path().join("c/take.wav")).unwrap(),
        b"RIFF-git-lfs-wrote-this"
    );
    let _ = me();
}
