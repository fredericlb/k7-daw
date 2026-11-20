//! The G4 review's cases for the merge: landing files that never share a path, a fast-forward that re-checks its ground.

mod common;
use common::me;
use common::merge_fx as fx;
use fx::diverge;
use k7_git::{Begin, Direct, Error, Landing};

/// A landing that keeps the temp path each file came from.
struct Spy {
    from: std::sync::Mutex<Vec<std::path::PathBuf>>,
    root: std::path::PathBuf,
}

impl Landing for Spy {
    fn land(&self, rel: &str, from: &std::path::Path) -> std::io::Result<()> {
        self.from.lock().unwrap().push(from.to_path_buf());
        Direct(self.root.clone()).land(rel, from)
    }

    fn remove(&self, rel: &str) -> std::io::Result<()> {
        Direct(self.root.clone()).remove(rel)
    }
}

#[tokio::test]
async fn every_landing_file_has_a_path_of_its_own() {
    let f = diverge(
        &[("n.md", b"one\n")],
        &[],
        &[("n.md", b"two\n"), ("x.md", b"x\n")],
    )
    .await;
    let spy = Spy {
        from: Default::default(),
        root: f.b.clone(),
    };
    f.repo_b.merge_or_forward(&f.theirs, &spy).unwrap();
    let seen = spy.from.lock().unwrap().clone();
    assert_eq!(seen.len(), 2);
    assert_ne!(seen[0], seen[1], "two files never share a temp path");
}

async fn planned() -> (fx::Fx, Option<String>) {
    let f = diverge(
        &[("n.md", b"one\n")],
        &[],
        &[("n.md", b"two\n"), ("x.md", b"x\n")],
    )
    .await;
    let Begin::FastForward { head, .. } = f.repo_b.merge_begin(&f.theirs).unwrap() else {
        panic!("not a fast-forward");
    };
    (f, head)
}

#[tokio::test]
async fn an_edit_made_between_the_plan_and_the_fast_forward_is_never_lost() {
    let (f, head) = planned().await;
    std::fs::write(f.b.join("n.md"), "my edit during the download\n").unwrap();
    let err = f
        .repo_b
        .merge_fast_forward(&f.theirs, head.as_deref(), &Direct(f.b.clone()))
        .unwrap_err();
    assert_eq!(err, Error::Uncommitted);
    assert_eq!(f.read("n.md"), "my edit during the download\n");
    assert!(!f.b.join("x.md").exists(), "nothing landed");
}

#[tokio::test]
async fn a_commit_made_between_the_plan_and_the_fast_forward_stays_on_the_branch() {
    let (f, head) = planned().await;
    std::fs::write(f.b.join("z.md"), "z\n").unwrap();
    let mine = f.repo_b.commit("made meanwhile", &me(), "B").unwrap();
    let err = f
        .repo_b
        .merge_fast_forward(&f.theirs, head.as_deref(), &Direct(f.b.clone()))
        .unwrap_err();
    assert_eq!(err, Error::MergeMoved);
    assert_eq!(
        f.repo_b.log(1, None).unwrap()[0].id,
        mine,
        "the branch kept it"
    );
    assert!(!f.b.join("x.md").exists(), "nothing landed");
}

fn big(byte: u8) -> Vec<u8> {
    vec![byte; 1_200_000]
}

/// What the route does before a fast-forward: the objects the incoming tree names are brought in.
async fn bring(f: &fx::Fx) -> usize {
    let wanted = f.repo_b.lfs_wanted_by(&f.theirs).unwrap();
    let n = wanted.len();
    f.net
        .blocking({
            let (net, to, b) = (f.net.clone(), f.to.clone(), f.b.clone());
            move || {
                let r = k7_git::Repo::open(&b, &b.join(".git"))?;
                r.lfs_fetch_objects(&net, &to, &wanted)
            }
        })
        .await
        .unwrap();
    n
}

#[tokio::test]
async fn a_pull_lands_large_files_judged_by_the_incoming_tree_not_the_old_attributes() {
    let zeros = big(0);
    let f = diverge(
        &[("a.k7x", b"x: 1\n")],
        &[],
        &[("-early.dat", &zeros), ("kit/sample.dat", &zeros)],
    )
    .await;
    assert_eq!(bring(&f).await, 2, "both large files are wanted");
    f.repo_b
        .merge_or_forward(&f.theirs, &Direct(f.b.clone()))
        .unwrap();
    for p in ["-early.dat", "kit/sample.dat"] {
        assert_eq!(
            std::fs::metadata(f.b.join(p)).unwrap().len(),
            1_200_000,
            "{p}"
        );
    }
    assert!(
        f.repo_b.status().unwrap().is_empty(),
        "{:?}",
        f.repo_b.status()
    );
}

#[tokio::test]
async fn a_merge_lands_large_files_judged_by_the_incoming_tree_too() {
    let zeros = big(0);
    let f = diverge(
        &[("a.k7x", b"x: 1\n")],
        &[("mine.k7x", b"m: 1\n")],
        &[("-early.dat", &zeros), ("kit/sample.dat", &zeros)],
    )
    .await;
    let Begin::Merge(s) = f.repo_b.merge_begin(&f.theirs).unwrap() else {
        panic!("not a merge");
    };
    assert_eq!(s.lfs.len(), 2, "both large files are named");
    bring(&f).await;
    f.repo_b
        .merge_resolve(&[], &Direct(f.b.clone()), &me(), "B")
        .unwrap();
    for p in ["-early.dat", "kit/sample.dat"] {
        assert_eq!(
            std::fs::metadata(f.b.join(p)).unwrap().len(),
            1_200_000,
            "{p}"
        );
    }
    assert!(
        f.repo_b.status().unwrap().is_empty(),
        "{:?}",
        f.repo_b.status()
    );
}

#[tokio::test]
async fn a_failed_download_leaves_no_pending_merge() {
    let f = diverge(
        &[("a.k7x", b"x: 1\n")],
        &[("mine.k7x", b"m: 1\n")],
        &[("take.wav", b"RIFF-theirs")],
    )
    .await;
    let oid = f.repo_a.lfs_head().unwrap().remove(0).1.oid;
    k7_git::lfs::store::Store::in_gitdir(f.remote.path())
        .remove(&oid)
        .unwrap();
    let err = f
        .net
        .blocking({
            let (net, to, b, theirs) = (f.net.clone(), f.to.clone(), f.b.clone(), f.theirs.clone());
            move || {
                let r = k7_git::Repo::open(&b, &b.join(".git"))?;
                r.merge_begin_fetching(&theirs, |lfs| r.lfs_fetch_objects(&net, &to, lfs))
            }
        })
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Network(_)), "{err:?}");
    assert!(f.repo_b.merge_pending().unwrap().is_none(), "nothing waits");
    assert!(!f.b.join(".git/k7-merge.json").exists());
}

/// Writes the second planned file the moment the first lands, as an autosave would.
struct Editing {
    root: std::path::PathBuf,
}

impl Landing for Editing {
    fn land(&self, rel: &str, from: &std::path::Path) -> std::io::Result<()> {
        Direct(self.root.clone()).land(rel, from)?;
        std::fs::write(self.root.join("x.md"), "edited while landing\n")
    }

    fn remove(&self, rel: &str) -> std::io::Result<()> {
        Direct(self.root.clone()).remove(rel)
    }
}

#[tokio::test]
async fn an_edit_made_while_files_land_is_never_overwritten() {
    let (f, head) = planned().await;
    let before = f.repo_b.log(1, None).unwrap()[0].id.clone();
    let err = f
        .repo_b
        .merge_fast_forward(&f.theirs, head.as_deref(), &Editing { root: f.b.clone() })
        .unwrap_err();
    assert_eq!(err, Error::Uncommitted);
    assert_eq!(f.read("x.md"), "edited while landing\n");
    assert_eq!(f.repo_b.log(1, None).unwrap()[0].id, before, "HEAD stays");
}
