//! The technical branch over a bare `file://` remote: snapshots, their push, two devices of one person converging,
//! a Save's squash and reset, another person's branch, and a push retried once the remote answers again.

mod common;

use std::path::Path;
use std::sync::Arc;

use common::{Dir, bare, me, net};
use k7_git::autosave::{Owner, Picked, Sent, branch_name};
use k7_git::{Direct, Identity, Net, Origin, Repo};

const DEVICE: &str = "Mac";

fn branch() -> String {
    branch_name(Owner::Email(&me().email))
}

fn origin(at: &Path) -> Origin {
    Origin {
        url: format!("file://{}", at.display()),
        user: None,
    }
}

/// A remote holding one Save of `song.k7x` on `main`.
fn seeded() -> (Dir, Origin, Arc<Net>) {
    let remote = Dir::new("auto-remote");
    bare(remote.path());
    let to = origin(remote.path());
    let n = net(true);
    let (d, repo) = common::workspace(&[("song.k7x", b"a: 1\n")]);
    repo.push_blocking(&n, &to).unwrap();
    drop(d);
    (remote, to, n)
}

struct Device {
    dir: Dir,
    repo: Repo,
    who: Identity,
    _parent: Dir,
}

fn device(n: &Arc<Net>, to: &Origin, who: Identity) -> Device {
    let dir = Dir::new("auto-dev");
    let at = dir.path().join("w");
    let repo = Repo::clone_blocking(n, to, &at).unwrap();
    Device {
        dir: Dir(at),
        repo,
        who,
        _parent: dir,
    }
}

impl Device {
    fn write(&self, rel: &str, text: &str) {
        self.dir.write(rel, text.as_bytes());
    }

    fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.dir.path().join(rel)).unwrap_or_default()
    }

    fn branch(&self) -> String {
        branch_name(Owner::Email(&self.who.email))
    }

    fn snapshot(&self) -> Option<String> {
        self.repo
            .autosave_commit(&self.branch(), &self.who, DEVICE)
            .unwrap()
    }

    /// What the front does at each turn: fetch, take in, push, and again while the remote moved meanwhile.
    fn sync(&self, n: &Arc<Net>, to: &Origin) -> Vec<Picked> {
        let mut seen = Vec::new();
        for _ in 0..3 {
            self.repo.fetch_blocking(n, to).unwrap();
            let fetch = |_: &[(String, k7_git::Pointer)]| Ok(());
            let landing = Direct(self.dir.path().to_path_buf());
            let picked = self
                .repo
                .autosave_integrate(&self.branch(), &self.who, DEVICE, &landing, &fetch)
                .unwrap();
            seen.push(picked);
            match self
                .repo
                .autosave_push_blocking(n, to, &self.branch())
                .unwrap()
            {
                Sent::Behind => continue,
                _ => break,
            }
        }
        seen
    }

    fn remote_tip(&self, to: &Origin) -> Option<String> {
        let r = git2::Repository::open(to.url.trim_start_matches("file://")).ok()?;
        let id = r
            .refname_to_id(&format!("refs/heads/{}", self.branch()))
            .ok()?;
        Some(id.to_string())
    }

    fn tip(&self) -> Option<String> {
        let r = git2::Repository::open(self.dir.path().join(".git")).ok()?;
        let id = r
            .refname_to_id(&format!("refs/heads/{}", self.branch()))
            .ok()?;
        Some(id.to_string())
    }

    fn head(&self) -> String {
        let r = git2::Repository::open(self.dir.path().join(".git")).unwrap();
        r.refname_to_id("HEAD").unwrap().to_string()
    }
}

fn other() -> Identity {
    Identity {
        name: "Bob".into(),
        email: "bob@example.com".into(),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unchanged_tree_is_never_a_snapshot_and_head_never_moves() {
    let (_r, to, n) = seeded();
    let a = device(&n, &to, me());
    assert_eq!(a.snapshot(), None, "nothing changed since the clone");
    let head = a.head();
    a.write("song.k7x", "a: 2\n");
    let first = a.snapshot().expect("an edit is a snapshot");
    assert_eq!(a.snapshot(), None, "the same tree again is nothing");
    assert_eq!(a.head(), head, "HEAD stays on main");
    assert_eq!(
        a.repo.status().unwrap().len(),
        1,
        "the edit is still unsaved"
    );
    a.write("song.k7x", "a: 3\n");
    let second = a.snapshot().unwrap();
    assert_ne!(first, second);
    assert_eq!(
        common::blob_at(&a.dir.path().join(".git"), &second, "song.k7x"),
        b"a: 3\n"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_technical_branch_is_pushed_under_the_persons_name() {
    let (_r, to, n) = seeded();
    let a = device(&n, &to, me());
    a.write("take.txt", "one\n");
    a.snapshot().unwrap();
    assert_eq!(
        a.repo.autosave_push_blocking(&n, &to, &branch()).unwrap(),
        Sent::Pushed
    );
    assert_eq!(a.remote_tip(&to), a.tip());
    assert_eq!(
        a.repo.autosave_push_blocking(&n, &to, &branch()).unwrap(),
        Sent::Nothing
    );
    assert_eq!(
        a.repo.autosave_info(&branch()).map(|(_, unsent)| unsent),
        Some(false)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn another_device_of_the_same_person_picks_up_where_the_first_left_off() {
    let (_r, to, n) = seeded();
    let (a, b) = (device(&n, &to, me()), device(&n, &to, me()));
    a.write("song.k7x", "a: from A\n");
    a.write("new/take.txt", "A's take\n");
    a.sync(&n, &to);
    let picked = b.sync(&n, &to);
    assert!(
        matches!(
            picked[0],
            Picked::Landed {
                files: 2,
                main_moved: false
            }
        ),
        "{picked:?}"
    );
    assert_eq!(b.read("song.k7x"), "a: from A\n");
    assert_eq!(b.read("new/take.txt"), "A's take\n");
    assert_eq!(b.tip(), a.tip(), "B is on A's snapshot, nothing of its own");
    assert_eq!(b.sync(&n, &to), vec![Picked::Nothing]);
}

#[tokio::test(flavor = "multi_thread")]
async fn concurrent_edits_of_two_devices_converge_on_one_branch() {
    let (_r, to, n) = seeded();
    let (a, b) = (device(&n, &to, me()), device(&n, &to, me()));
    a.write("a.txt", "A\n");
    b.write("b.txt", "B\n");
    a.write("song.k7x", "a: A\n");
    b.write("song.k7x", "a: B\n");
    a.sync(&n, &to);
    let seen = b.sync(&n, &to);
    assert!(
        matches!(seen[0], Picked::Landed { .. }),
        "B merges A's: {seen:?}"
    );
    a.sync(&n, &to);
    for d in [&a, &b] {
        assert_eq!(d.read("a.txt"), "A\n");
        assert_eq!(d.read("b.txt"), "B\n");
    }
    let song = a.read("song.k7x");
    assert_eq!(
        b.read("song.k7x"),
        song,
        "one version of the file both changed"
    );
    assert!(song == "a: A\n" || song == "a: B\n");
    assert_eq!(a.tip(), b.tip());
    assert_eq!(a.remote_tip(&to), a.tip());
    let r = git2::Repository::open(a.dir.path().join(".git")).unwrap();
    let merge = r
        .find_commit(git2::Oid::from_str(&a.tip().unwrap()).unwrap())
        .unwrap();
    assert_eq!(
        merge.parent_count(),
        2,
        "both snapshots stay in the history"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_save_is_one_squash_commit_on_main_and_the_branch_restarts_from_it() {
    let (_r, to, n) = seeded();
    let a = device(&n, &to, me());
    let before = a.head();
    for i in 0..3 {
        a.write("song.k7x", &format!("a: {i}\n"));
        a.snapshot().unwrap();
    }
    a.sync(&n, &to);
    let save = a
        .repo
        .commit("Saved on 26/11/2026 10:00", &me(), DEVICE)
        .unwrap();
    a.repo.push_blocking(&n, &to).unwrap();
    let r = git2::Repository::open(a.dir.path().join(".git")).unwrap();
    let c = r.find_commit(git2::Oid::from_str(&save).unwrap()).unwrap();
    assert_eq!(c.parent_count(), 1);
    assert_eq!(
        c.parent_id(0).unwrap().to_string(),
        before,
        "one commit over the old main"
    );
    assert!(
        c.message()
            .unwrap()
            .starts_with("Saved on 26/11/2026 10:00")
    );
    assert!(a.repo.autosave_reset(&branch()).unwrap());
    assert_eq!(a.tip(), Some(save.clone()));
    a.repo.fetch_blocking(&n, &to).unwrap();
    assert_eq!(
        a.repo.autosave_push_blocking(&n, &to, &branch()).unwrap(),
        Sent::Pushed
    );
    assert_eq!(
        a.remote_tip(&to),
        Some(save.clone()),
        "the reset replaced the remote's snapshots"
    );
    assert_eq!(a.snapshot(), None, "the saved tree is no new snapshot");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_device_follows_the_other_devices_save() {
    let (_r, to, n) = seeded();
    let (a, b) = (device(&n, &to, me()), device(&n, &to, me()));
    a.write("song.k7x", "a: saved\n");
    a.sync(&n, &to);
    b.sync(&n, &to);
    let save = a.repo.commit("Save", &me(), DEVICE).unwrap();
    a.repo.push_blocking(&n, &to).unwrap();
    a.repo.autosave_reset(&branch()).unwrap();
    a.sync(&n, &to);
    let seen = b.sync(&n, &to);
    assert!(
        matches!(
            seen[0],
            Picked::Landed {
                main_moved: true,
                ..
            }
        ),
        "{seen:?}"
    );
    assert_eq!(b.head(), save, "B's main follows A's Save");
    assert!(
        b.repo.status().unwrap().is_empty(),
        "nothing is left unsaved on B"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_reset_never_drops_a_snapshot_this_device_has_not_taken_in() {
    let (_r, to, n) = seeded();
    let (a, b) = (device(&n, &to, me()), device(&n, &to, me()));
    b.write("b.txt", "B\n");
    b.sync(&n, &to);
    a.write("a.txt", "A\n");
    a.repo.fetch_blocking(&n, &to).unwrap();
    a.repo.commit("Save", &me(), DEVICE).unwrap();
    assert!(
        !a.repo.autosave_reset(&branch()).unwrap(),
        "B's snapshot is not in A's branch"
    );
    a.sync(&n, &to);
    assert_eq!(
        a.read("b.txt"),
        "B\n",
        "B's work reaches A instead of being replaced"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn another_person_writes_another_branch() {
    let (_r, to, n) = seeded();
    let (a, b) = (device(&n, &to, me()), device(&n, &to, other()));
    assert_ne!(a.branch(), b.branch());
    assert_eq!(b.branch(), "k7/autosave/bob-at-example.com");
    a.write("a.txt", "A\n");
    a.sync(&n, &to);
    assert_eq!(b.sync(&n, &to), vec![Picked::Nothing]);
    assert_eq!(
        b.read("a.txt"),
        "",
        "another person's auto-saves never land here"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_push_to_a_remote_that_does_not_answer_is_sent_once_it_does() {
    let (_r, to, n) = seeded();
    let a = device(&n, &to, me());
    let gone = Dir::new("auto-gone");
    let away = origin(&gone.path().join("not-yet.git"));
    a.write("a.txt", "A\n");
    a.snapshot().unwrap();
    assert!(a.repo.autosave_push_blocking(&n, &away, &branch()).is_err());
    assert_eq!(
        a.repo.autosave_info(&branch()).map(|(_, unsent)| unsent),
        Some(true)
    );
    bare(&gone.path().join("not-yet.git"));
    assert_eq!(
        a.repo.autosave_push_blocking(&n, &away, &branch()).unwrap(),
        Sent::Pushed
    );
}
