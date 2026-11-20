//! Shared by the merge tests: two workspaces that diverged from one commit through a bare file remote.
#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::Arc;

use super::{Dir, bare, me, net};
use k7_git::{Net, Origin, Repo};

pub struct Fx {
    pub remote: Dir,
    pub a: Dir,
    pub b_dir: Dir,
    pub b: PathBuf,
    pub repo_a: Repo,
    pub repo_b: Repo,
    pub net: Arc<Net>,
    pub to: Origin,
    pub theirs: String,
}

pub type Files<'a> = &'a [(&'a str, &'a [u8])];

fn write(d: &std::path::Path, files: Files) {
    for (p, b) in files {
        let f = d.join(p);
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(f, b).unwrap();
    }
}

/// `base` committed and pushed by A and cloned by B; then A pushes `theirs` and B commits `mine` and fetches.
pub async fn diverge(base: Files<'_>, mine: Files<'_>, theirs: Files<'_>) -> Fx {
    diverge_with(base, mine, theirs, &[], &[]).await
}

/// The same, with files each side removes.
pub async fn diverge_with(
    base: Files<'_>,
    mine: Files<'_>,
    theirs: Files<'_>,
    mine_rm: &[&str],
    theirs_rm: &[&str],
) -> Fx {
    let remote = Dir::new("mrg-remote");
    bare(remote.path());
    let to = Origin {
        url: format!("file://{}", remote.path().display()),
        user: None,
    };
    let n = net(true);
    let a = Dir::new("mrg-a");
    let repo_a = Repo::init(a.path(), &a.path().join(".git")).unwrap();
    write(a.path(), base);
    repo_a.commit("base", &me(), "A").unwrap();
    n.push(a.path().to_path_buf(), a.path().join(".git"), to.clone())
        .await
        .unwrap();
    let b_dir = Dir::new("mrg-b");
    let b = b_dir.path().join("c");
    n.clone_into(to.clone(), b.clone()).await.unwrap();
    let repo_b = Repo::open(&b, &b.join(".git")).unwrap();
    write(a.path(), theirs);
    for p in theirs_rm {
        std::fs::remove_file(a.path().join(p)).unwrap();
    }
    repo_a.commit("theirs", &me(), "A").unwrap();
    n.push(a.path().to_path_buf(), a.path().join(".git"), to.clone())
        .await
        .unwrap();
    write(&b, mine);
    for p in mine_rm {
        std::fs::remove_file(b.join(p)).unwrap();
    }
    if !mine.is_empty() || !mine_rm.is_empty() {
        repo_b.commit("mine", &me(), "B").unwrap();
    }
    n.fetch(b.clone(), b.join(".git"), to.clone())
        .await
        .unwrap();
    let theirs = git2::Repository::open(b.join(".git"))
        .unwrap()
        .refname_to_id("refs/remotes/origin/main")
        .unwrap()
        .to_string();
    Fx {
        remote,
        a,
        b_dir,
        b,
        repo_a,
        repo_b,
        net: n,
        to,
        theirs,
    }
}

impl Fx {
    pub fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.b.join(rel)).unwrap()
    }
}
