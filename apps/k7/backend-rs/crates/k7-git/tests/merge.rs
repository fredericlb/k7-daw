//! G3: the merge of a diverged pull, its pending store, and the choices that finish it.

mod common;
use common::me;
use common::merge_fx as fx;
use fx::diverge;
use k7_git::{Begin, Choice, Direct, Error, Kind, Repo};

const EQ: &[u8] = b"plugin(\"eq\")\nparams({ low: 0, high: 0 })\nbands([1, 2])\n";

fn summary(b: Begin) -> k7_git::Summary {
    match b {
        Begin::Merge(s) => s,
        other => panic!("not a merge: {other:?}"),
    }
}

#[tokio::test]
async fn two_sides_that_changed_different_keys_merge_with_nothing_to_choose() {
    let mine: &[u8] = b"plugin(\"eq\")\nparams({ low: 0.25, high: 0 })\nbands([1, 2])\n";
    let theirs: &[u8] = b"plugin(\"eq\")\nparams({ low: 0, high: 0 })\nbands([8])\n";
    let f = diverge(
        &[("s/eq.k7x", EQ)],
        &[("s/eq.k7x", mine)],
        &[("s/eq.k7x", theirs)],
    )
    .await;
    let s = summary(f.repo_b.merge_begin(&f.theirs).unwrap());
    assert!(s.conflicts.is_empty());
    assert_eq!(
        f.read("s/eq.k7x").as_bytes(),
        mine,
        "nothing moves before resolve"
    );
    let id = f
        .repo_b
        .merge_resolve(&[], &Direct(f.b.clone()), &me(), "B")
        .unwrap();
    let text = f.read("s/eq.k7x");
    assert!(
        text.contains("low: 0.25") && text.contains("bands([8])"),
        "{text}"
    );
    let raw = git2::Repository::open(f.b.join(".git")).unwrap();
    let c = raw.find_commit(git2::Oid::from_str(&id).unwrap()).unwrap();
    assert_eq!(c.parent_count(), 2, "a merge commit has both parents");
    assert!(f.repo_b.merge_pending().unwrap().is_none());
    assert!(f.repo_b.status().unwrap().is_empty());
}

#[tokio::test]
async fn the_same_key_changed_on_both_sides_is_a_conflict_at_its_key_path() {
    let mine: &[u8] = b"plugin(\"eq\")\nparams({ low: 0.25, high: 0 })\nbands([1, 2])\n";
    let theirs: &[u8] = b"plugin(\"eq\")\nparams({ low: 0.75, high: 0 })\nbands([1, 2])\n";
    let f = diverge(
        &[("s/eq.k7x", EQ)],
        &[("s/eq.k7x", mine)],
        &[("s/eq.k7x", theirs)],
    )
    .await;
    let s = summary(f.repo_b.merge_begin(&f.theirs).unwrap());
    assert_eq!(s.conflicts.len(), 1);
    let c = &s.conflicts[0];
    assert_eq!(
        (
            c.kind,
            c.key_path.as_deref(),
            c.base.as_deref(),
            c.mine.as_deref(),
            c.theirs.as_deref()
        ),
        (
            Kind::K7x,
            Some("params.low"),
            Some("0"),
            Some("0.25"),
            Some("0.75")
        )
    );
    assert!(!c.both);
    let unresolved = f
        .repo_b
        .merge_resolve(&[], &Direct(f.b.clone()), &me(), "B")
        .unwrap_err();
    assert_eq!(unresolved, Error::Unresolved { n: 1 });
    f.repo_b
        .merge_resolve(
            &[(c.id.clone(), Choice::Theirs)],
            &Direct(f.b.clone()),
            &me(),
            "B",
        )
        .unwrap();
    assert!(f.read("s/eq.k7x").contains("low: 0.75"));
}

#[tokio::test]
async fn a_text_changed_on_the_same_lines_offers_mine_theirs_and_both() {
    let base: &[u8] = b"one\ntwo\nthree\n";
    let f = diverge(
        &[("n.md", base)],
        &[("n.md", b"one\nMINE\nthree\n")],
        &[("n.md", b"one\nTHEIRS\nthree\n")],
    )
    .await;
    let s = summary(f.repo_b.merge_begin(&f.theirs).unwrap());
    let c = &s.conflicts[0];
    assert_eq!(
        (
            c.kind,
            c.line,
            c.mine.as_deref(),
            c.theirs.as_deref(),
            c.both
        ),
        (Kind::Text, Some(2), Some("MINE\n"), Some("THEIRS\n"), true)
    );
    f.repo_b
        .merge_resolve(
            &[(c.id.clone(), Choice::Both)],
            &Direct(f.b.clone()),
            &me(),
            "B",
        )
        .unwrap();
    assert_eq!(f.read("n.md"), "one\nMINE\nTHEIRS\nthree\n");
}

#[tokio::test]
async fn an_audio_file_changed_on_both_sides_keeps_both_when_asked_with_theirs_beside() {
    let f = diverge(
        &[("take.wav", b"RIFF-base")],
        &[("take.wav", b"RIFF-mine")],
        &[("take.wav", b"RIFF-theirs")],
    )
    .await;
    let s = summary(f.repo_b.merge_begin(&f.theirs).unwrap());
    let c = &s.conflicts[0];
    assert_eq!((c.kind, c.both), (Kind::Binary, true));
    assert_eq!(s.lfs.len(), 1, "their bytes are to be brought in first");
    let mut ln = Vec::new();
    let store = k7_git::lfs::store::Store::in_gitdir(&f.b.join(".git"));
    ln.push(store.has(&s.lfs[0].1.oid));
    f.net
        .blocking({
            let (net, to, b, files) = (f.net.clone(), f.to.clone(), f.b.clone(), s.lfs.clone());
            move || {
                let r = Repo::open(&b, &b.join(".git"))?;
                r.lfs_fetch_objects(&net, &to, &files)
            }
        })
        .await
        .unwrap();
    f.repo_b
        .merge_resolve(
            &[(c.id.clone(), Choice::Both)],
            &Direct(f.b.clone()),
            &me(),
            "B",
        )
        .unwrap();
    assert_eq!(std::fs::read(f.b.join("take.wav")).unwrap(), b"RIFF-mine");
    assert_eq!(
        std::fs::read(f.b.join("take (Ada).wav")).unwrap(),
        b"RIFF-theirs"
    );
}

#[tokio::test]
async fn a_file_one_side_removed_and_the_other_changed_is_keep_or_remove() {
    let f = fx::diverge_with(
        &[("a.k7x", b"x: 1\n"), ("b.k7x", b"y: 1\n")],
        &[("a.k7x", b"x: 2\n")],
        &[],
        &[],
        &["a.k7x"],
    )
    .await;
    let s = summary(f.repo_b.merge_begin(&f.theirs).unwrap());
    let c = &s.conflicts[0];
    assert_eq!(
        (c.kind, c.path.as_str(), c.both),
        (Kind::Presence, "a.k7x", false)
    );
    assert!(
        f.repo_b.merge_choose(&c.id, Choice::Both).is_err(),
        "only keep and remove are offered"
    );
    f.repo_b
        .merge_resolve(
            &[(c.id.clone(), Choice::Remove)],
            &Direct(f.b.clone()),
            &me(),
            "B",
        )
        .unwrap();
    assert!(!f.b.join("a.k7x").exists());
    assert!(f.b.join("b.k7x").exists());
}

#[tokio::test]
async fn a_restart_between_the_plan_and_the_end_keeps_the_choices() {
    let base: &[u8] = b"one\ntwo\nthree\nfour\nfive\nsix\nseven\n";
    let f = diverge(
        &[("n.md", base)],
        &[("n.md", b"one\nM2\nthree\nfour\nfive\nM6\nseven\n")],
        &[("n.md", b"one\nT2\nthree\nfour\nfive\nT6\nseven\n")],
    )
    .await;
    let s = summary(f.repo_b.merge_begin(&f.theirs).unwrap());
    assert_eq!(s.conflicts.len(), 2);
    f.repo_b
        .merge_choose(&s.conflicts[0].id, Choice::Theirs)
        .unwrap();
    drop(f.repo_b);
    let again = k7_git::Repo::open(&f.b, &f.b.join(".git")).unwrap();
    let pending = again.merge_pending().unwrap().unwrap();
    assert_eq!(pending.conflicts[0].choice, Some(Choice::Theirs));
    assert_eq!(pending.conflicts[1].choice, None);
    again
        .merge_resolve(
            &[(s.conflicts[1].id.clone(), Choice::Mine)],
            &Direct(f.b.clone()),
            &me(),
            "B",
        )
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(f.b.join("n.md")).unwrap(),
        "one\nT2\nthree\nfour\nfive\nM6\nseven\n"
    );
}

fn snapshot(dir: &std::path::Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(d).unwrap().flatten() {
            let p = e.path();
            if p.file_name().is_some_and(|n| n == ".git") {
                continue;
            }
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push((
                    p.strip_prefix(dir).unwrap().display().to_string(),
                    std::fs::read(&p).unwrap(),
                ));
            }
        }
    }
    out.sort();
    out
}

#[tokio::test]
async fn an_abort_leaves_head_and_every_file_byte_for_byte() {
    let f = diverge(
        &[("n.md", b"one\ntwo\n"), ("take.wav", b"RIFF-1")],
        &[("n.md", b"one\nM\n"), ("take.wav", b"RIFF-2")],
        &[("n.md", b"one\nT\n"), ("take.wav", b"RIFF-3")],
    )
    .await;
    let (files, head) = (snapshot(&f.b), f.repo_b.log(1, None).unwrap()[0].id.clone());
    let s = summary(f.repo_b.merge_begin(&f.theirs).unwrap());
    f.repo_b
        .merge_choose(&s.conflicts[0].id, Choice::Theirs)
        .unwrap();
    f.repo_b.merge_abort().unwrap();
    assert!(f.repo_b.merge_pending().unwrap().is_none());
    assert_eq!(snapshot(&f.b), files);
    assert_eq!(f.repo_b.log(1, None).unwrap()[0].id, head);
    assert!(f.repo_b.status().unwrap().is_empty());
}

#[tokio::test]
async fn a_pull_over_uncommitted_changes_is_refused() {
    let f = diverge(
        &[("n.md", b"one\n")],
        &[("m.md", b"mine\n")],
        &[("t.md", b"theirs\n")],
    )
    .await;
    std::fs::write(f.b.join("n.md"), "edited\n").unwrap();
    assert_eq!(
        f.repo_b.merge_begin(&f.theirs).unwrap_err(),
        Error::Uncommitted
    );
    assert_eq!(Error::Uncommitted.code(), "uncommitted");
    assert!(f.repo_b.merge_pending().unwrap().is_none());
}

#[tokio::test]
async fn a_both_that_does_not_parse_is_refused_with_the_parsers_line_and_not_kept() {
    let f = diverge(
        &[("s/eq.k7x", EQ)],
        &[(
            "s/eq.k7x",
            b"plugin(\"eq\")\nparams({ low: 1, high: 0 }\nbands([1, 2])\n",
        )],
        &[(
            "s/eq.k7x",
            b"plugin(\"eq\")\nparams({ low: 2, high: 0 })\nbands([1, 2",
        )],
    )
    .await;
    let s = summary(f.repo_b.merge_begin(&f.theirs).unwrap());
    let c = s
        .conflicts
        .iter()
        .find(|c| c.both)
        .expect("a text conflict offers both");
    let err = f.repo_b.merge_choose(&c.id, Choice::Both).unwrap_err();
    assert!(matches!(err, Error::Unparsable { .. }), "{err:?}");
    assert_eq!(err.code(), "unparsable");
    let kept = f.repo_b.merge_pending().unwrap().unwrap();
    assert!(
        kept.conflicts.iter().all(|c| c.choice.is_none()),
        "the refused choice is not kept"
    );
}

#[tokio::test]
async fn a_branch_only_behind_is_fast_forwarded_through_the_landing_and_up_to_date_is_nothing() {
    let f = diverge(
        &[("n.md", b"one\n"), ("gone.md", b"g\n")],
        &[],
        &[("n.md", b"two\n"), ("new/x.md", b"x\n")],
    )
    .await;
    assert!(
        matches!(f.repo_b.merge_begin(&f.theirs).unwrap(), Begin::FastForward { theirs, head: Some(_) } if theirs == f.theirs)
    );
    assert!(
        f.repo_b.merge_pending().unwrap().is_none(),
        "a fast-forward keeps no pending file"
    );
    let landed = Landed {
        log: Default::default(),
        root: f.b.clone(),
    };
    f.repo_b.merge_or_forward(&f.theirs, &landed).unwrap();
    assert_eq!(f.read("n.md"), "two\n");
    assert_eq!(f.read("new/x.md"), "x\n");
    assert_eq!(
        landed.log.lock().unwrap().clone(),
        vec!["n.md".to_string(), "new/x.md".to_string()],
        "each file went through the landing"
    );
    assert_eq!(f.repo_b.log(1, None).unwrap()[0].id, f.theirs);
    assert!(f.repo_b.status().unwrap().is_empty());
    assert_eq!(f.repo_b.merge_begin(&f.theirs).unwrap(), Begin::UpToDate);
}

struct Landed {
    log: std::sync::Mutex<Vec<String>>,
    root: std::path::PathBuf,
}

impl k7_git::Landing for Landed {
    fn land(&self, rel: &str, from: &std::path::Path) -> std::io::Result<()> {
        self.log.lock().unwrap().push(rel.to_string());
        Direct(self.root.clone()).land(rel, from)
    }

    fn remove(&self, rel: &str) -> std::io::Result<()> {
        Direct(self.root.clone()).remove(rel)
    }
}

#[tokio::test]
async fn two_sides_that_wrote_the_same_audio_have_nothing_to_choose() {
    let f = diverge(
        &[("take.wav", b"RIFF-base")],
        &[("take.wav", b"RIFF-same")],
        &[("take.wav", b"RIFF-same")],
    )
    .await;
    let s = summary(f.repo_b.merge_begin(&f.theirs).unwrap());
    assert!(s.conflicts.is_empty(), "equal by oid: {:?}", s.conflicts);
}

#[tokio::test]
async fn a_second_merge_cannot_begin_over_a_pending_one() {
    let f = diverge(
        &[("n.md", b"one\n")],
        &[("n.md", b"M\n")],
        &[("n.md", b"T\n")],
    )
    .await;
    f.repo_b.merge_begin(&f.theirs).unwrap();
    assert_eq!(
        f.repo_b.merge_begin(&f.theirs).unwrap_err(),
        Error::MergeOpen
    );
    assert_eq!(
        f.repo_b
            .merge_choose("nope#0", Choice::Mine)
            .unwrap_err()
            .code(),
        "bad-path"
    );
}

#[tokio::test]
async fn their_audio_that_is_not_here_is_offline_and_nothing_moves() {
    let f = diverge(
        &[("take.wav", b"RIFF-base"), ("n.md", b"one\n")],
        &[("n.md", b"M\n")],
        &[("take.wav", b"RIFF-theirs"), ("n.md", b"T\n")],
    )
    .await;
    let s = summary(f.repo_b.merge_begin(&f.theirs).unwrap());
    assert_eq!(s.lfs.len(), 1);
    let before = snapshot(&f.b);
    let err = f
        .repo_b
        .merge_resolve(
            &[(s.conflicts[0].id.clone(), Choice::Theirs)],
            &Direct(f.b.clone()),
            &me(),
            "B",
        )
        .unwrap_err();
    assert!(matches!(err, Error::Offline { .. }), "{err:?}");
    assert_eq!(
        f.repo_b.log(1, None).unwrap()[0].subject,
        "mine",
        "no merge commit"
    );
    assert!(
        f.repo_b.merge_pending().unwrap().is_some(),
        "the merge waits for the bytes"
    );
    assert_eq!(
        snapshot(&f.b),
        before,
        "not one file moved before the bytes were all there"
    );
}

/// A remote commit holding the path `../planted.txt`, which libgit2's own checkout would refuse but a landing would not.
fn hostile_tree(remote: &std::path::Path, parent: Option<git2::Oid>) -> git2::Oid {
    let raw = git2::Repository::open_bare(remote).unwrap();
    let blob = raw.blob(b"planted\n").unwrap();
    let mut inner = raw.treebuilder(None).unwrap();
    inner.insert("planted.txt", blob, 0o100644).unwrap();
    let inner = inner.write().unwrap();
    let ok = raw.blob(b"x\n").unwrap();
    // Written as raw tree bytes: the tree builder refuses the name.
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"40000 ..\0");
    bytes.extend_from_slice(inner.as_bytes());
    bytes.extend_from_slice(b"100644 a.md\0");
    bytes.extend_from_slice(ok.as_bytes());
    let tree = raw
        .odb()
        .unwrap()
        .write(git2::ObjectType::Tree, &bytes)
        .unwrap();
    let tree = raw.find_tree(tree).unwrap();
    let sig = git2::Signature::now("Evil", "e@example.com").unwrap();
    let parents: Vec<git2::Commit> = parent
        .map(|p| raw.find_commit(p).unwrap())
        .into_iter()
        .collect();
    let refs: Vec<&git2::Commit> = parents.iter().collect();
    raw.commit(Some("refs/heads/main"), &sig, &sig, "evil", &tree, &refs)
        .unwrap()
}

#[tokio::test]
async fn a_path_that_climbs_out_of_the_workspace_refuses_the_whole_pull_before_a_file_lands() {
    let f = diverge(&[("a.md", b"base\n")], &[], &[("a.md", b"theirs\n")]).await;
    let tip = git2::Oid::from_str(&f.theirs).unwrap();
    let evil = hostile_tree(f.remote.path(), Some(tip));
    f.net
        .fetch(f.b.clone(), f.b.join(".git"), f.to.clone())
        .await
        .unwrap();
    let before = snapshot(&f.b);
    let err = f
        .repo_b
        .merge_or_forward(&evil.to_string(), &Direct(f.b.clone()))
        .unwrap_err();
    assert_eq!(err, Error::BadPath("../planted.txt".into()));
    assert!(
        !f.b.parent().unwrap().join("planted.txt").exists(),
        "nothing was written outside the workspace"
    );
    assert_eq!(snapshot(&f.b), before, "and nothing inside moved");
    // The same commit met as a diverged merge.
    std::fs::write(f.b.join("mine.md"), "mine\n").unwrap();
    f.repo_b.commit("mine", &me(), "B").unwrap();
    assert_eq!(
        f.repo_b.merge_begin(&evil.to_string()).unwrap_err(),
        Error::BadPath("../planted.txt".into())
    );
    assert!(f.repo_b.merge_pending().unwrap().is_none());
}

#[tokio::test]
async fn an_edit_made_while_the_merge_waits_is_never_overwritten_and_a_moved_branch_lands_nothing()
{
    let f = diverge(
        &[("n.md", b"one\ntwo\n"), ("y.md", b"y1\n")],
        &[("n.md", b"one\nM\n")],
        &[("n.md", b"one\nT\n"), ("y.md", b"y2\n")],
    )
    .await;
    let s = summary(f.repo_b.merge_begin(&f.theirs).unwrap());
    let pick = [(s.conflicts[0].id.clone(), Choice::Theirs)];
    std::fs::write(f.b.join("y.md"), "my edit while it waited\n").unwrap();
    let err = f
        .repo_b
        .merge_resolve(&pick, &Direct(f.b.clone()), &me(), "B")
        .unwrap_err();
    assert_eq!(err, Error::Uncommitted);
    assert_eq!(
        f.read("y.md"),
        "my edit while it waited\n",
        "the edit stays"
    );
    assert_eq!(f.read("n.md"), "one\nM\n", "nothing landed");
    std::fs::write(f.b.join("y.md"), "y1\n").unwrap();
    // The branch moves while the merge waits.
    std::fs::write(f.b.join("z.md"), "z\n").unwrap();
    // Another process commits behind the library's back: `Repo::commit` itself refuses while a merge waits.
    let raw = git2::Repository::open(f.b.join(".git")).unwrap();
    let mut index = raw.index().unwrap();
    index.add_path(std::path::Path::new("z.md")).unwrap();
    index.write().unwrap();
    let (tree, sig) = (
        raw.find_tree(index.write_tree().unwrap()).unwrap(),
        raw.signature()
            .unwrap_or_else(|_| git2::Signature::now("x", "x@y").unwrap()),
    );
    let parent = raw.head().unwrap().peel_to_commit().unwrap();
    raw.commit(Some("HEAD"), &sig, &sig, "moved on", &tree, &[&parent])
        .unwrap();
    let err = f
        .repo_b
        .merge_resolve(&pick, &Direct(f.b.clone()), &me(), "B")
        .unwrap_err();
    assert_eq!(err, Error::MergeMoved);
    assert_eq!(f.read("n.md"), "one\nM\n", "no file of a half merge");
}

#[tokio::test]
async fn a_fast_forward_whose_audio_is_missing_lands_nothing_and_moves_nothing() {
    let f = diverge(
        &[("a.txt", b"one\n")],
        &[],
        &[("a.txt", b"two\n"), ("z.wav", b"RIFF-z")],
    )
    .await;
    let before = snapshot(&f.b);
    let head = f.repo_b.log(1, None).unwrap()[0].id.clone();
    let err = f
        .repo_b
        .merge_or_forward(&f.theirs, &Direct(f.b.clone()))
        .unwrap_err();
    assert_eq!(
        err,
        Error::Offline {
            path: "z.wav".into()
        }
    );
    assert_eq!(snapshot(&f.b), before, "a.txt is not half pulled");
    assert_eq!(f.repo_b.log(1, None).unwrap()[0].id, head);
    assert!(
        f.repo_b.status().unwrap().is_empty(),
        "the next pull is not Uncommitted"
    );
}
