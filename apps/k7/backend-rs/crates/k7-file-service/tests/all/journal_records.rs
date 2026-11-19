//! Records side by side, undo alone: a record's two images from one walk, the rebase of a record still running, the gate an
//! undo and a record queue on, and the tokens (1730 to 1739), against `history.test.ts`' `Workspace.recordChange` cases.

use crate::support;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use k7_file_service::journal::{Mode, Owner};
use serde_json::{Value, json};
use support::journal::{Bed, ME, moved};

#[tokio::test]
async fn journals_a_text_edit_names_it_project_relative_and_undo_redo_reverts_it() {
    let b = Bed::new("jr-edit");
    b.put("a.k7x", "v1");
    let ended = b.step("edit a", || b.put("a.k7x", "v2!")).await;
    assert_eq!(
        ended.files,
        Some(vec![json!({ "path": "a.k7x", "kind": "edited" })])
    );
    assert_eq!(b.labels(), ["edit a"]);
    assert_eq!(moved(&b.undo().await), "edit a");
    assert_eq!(b.text("a.k7x"), "v1");
    assert_eq!(moved(&b.redo().await), "edit a");
    assert_eq!(b.text("a.k7x"), "v2!");
    // The image and its token come from one walk (1731): the memo's own walks are not taken a second time.
    assert_eq!(b.revs.walks(), 0);
}

#[tokio::test]
async fn tells_a_rewrite_from_an_appearance_and_a_file_that_went() {
    let b = Bed::new("jr-names");
    b.put("clips/a.k7x", "v1");
    let e = b.step("edit", || b.put("clips/a.k7x", "v2")).await;
    assert_eq!(
        e.files,
        Some(vec![json!({ "path": "clips/a.k7x", "kind": "edited" })])
    );
    let e = b.step("new", || b.put("clips/b.k7x", "v1")).await;
    assert_eq!(
        e.files,
        Some(vec![json!({ "path": "clips/b.k7x", "kind": "added" })])
    );
    let e = b
        .step("delete", || {
            std::fs::remove_file(b.at("clips/a.k7x")).unwrap()
        })
        .await;
    assert_eq!(
        e.files,
        Some(vec![json!({ "path": "clips/a.k7x", "kind": "removed" })])
    );
    let e = b.step("noop", || {}).await;
    assert_eq!(e.files, Some(vec![]));
    assert_eq!(b.labels(), ["edit", "new", "delete"]);
}

#[tokio::test]
async fn a_record_with_no_project_diffs_nothing_and_still_holds_the_gate() {
    let b = Bed::new("jr-none");
    let begun = b.j.begin(&b.home, &b.root, None, "x", Mode::Step, ME).await;
    assert!(begun.rev.starts_with("none:"), "{}", begun.rev);
    assert_eq!(b.j.in_flight(&b.root), 1);
    let ended = b.j.end(begun.id, None).await;
    assert_eq!((ended.files, ended.after), (None, begun.rev));
    assert_eq!(b.j.in_flight(&b.root), 0);
}

#[tokio::test]
async fn a_same_size_rewrite_inside_two_seconds_is_a_step_of_its_own() {
    // 1732: `v1` to `v2` inside one tick, the cache having seen `v1`.
    let b = Bed::new("jr-settle");
    b.put("a.k7x", "v1");
    b.step("first", || b.put("a.k7x", "v2")).await;
    b.step("second", || b.put("a.k7x", "v3")).await;
    assert_eq!(b.labels(), ["first", "second"]);
    assert_eq!(moved(&b.undo().await), "second");
    assert_eq!(b.text("a.k7x"), "v2");
}

#[tokio::test]
async fn a_write_that_lands_during_a_slow_verb_stays_out_of_the_verbs_step() {
    let b = Bed::new("jr-slow");
    b.put("a.k7x", "a1");
    b.put("b.k7x", "b1");
    let slow =
        b.j.begin(&b.home, &b.root, Some(&b.dir), "slow", Mode::Step, ME)
            .await;
    b.step("save", || b.put("b.k7x", "b2")).await;
    b.put("a.k7x", "a2");
    b.j.end(slow.id, None).await;
    assert_eq!(b.labels(), ["save", "slow"]);
    b.undo().await;
    assert_eq!(
        (b.text("a.k7x"), b.text("b.k7x")),
        ("a1".into(), "b2".into())
    );
    b.undo().await;
    assert_eq!(b.text("b.k7x"), "b1");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn holds_an_undo_until_the_command_in_flight_has_been_journalled() {
    let b = Arc::new(Bed::new("jr-hold"));
    b.put("a.k7x", "a1");
    b.step("first", || b.put("a.k7x", "a2")).await;
    let second =
        b.j.begin(&b.home, &b.root, Some(&b.dir), "second", Mode::Step, ME)
            .await;
    let b2 = Arc::clone(&b);
    let undo = tokio::spawn(async move { b2.undo().await });
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert!(
        !undo.is_finished(),
        "the undo must wait for the record in flight"
    );
    b.put("a.k7x", "a3");
    b.j.end(second.id, None).await;
    assert_eq!(moved(&undo.await.unwrap()), "second");
    assert_eq!(b.text("a.k7x"), "a2");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_record_begun_while_an_undo_waits_queues_behind_it() {
    let b = Arc::new(Bed::new("jr-fifo"));
    b.put("a.k7x", "a1");
    b.step("first", || b.put("a.k7x", "a2")).await;
    let running =
        b.j.begin(&b.home, &b.root, Some(&b.dir), "running", Mode::Step, ME)
            .await;
    let order = Arc::new(Mutex::new(Vec::new()));
    let (b1, o1) = (Arc::clone(&b), Arc::clone(&order));
    let undo = tokio::spawn(async move {
        b1.undo().await;
        o1.lock().unwrap().push("undo");
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    let (b2, o2) = (Arc::clone(&b), Arc::clone(&order));
    let later = tokio::spawn(async move {
        let begun =
            b2.j.begin(&b2.home, &b2.root, Some(&b2.dir), "later", Mode::Step, ME)
                .await;
        o2.lock().unwrap().push("later");
        b2.j.end(begun.id, None).await;
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        order.lock().unwrap().is_empty(),
        "both wait for the record in flight"
    );
    b.j.end(running.id, None).await;
    undo.await.unwrap();
    later.await.unwrap();
    assert_eq!(*order.lock().unwrap(), ["undo", "later"]);
}

#[tokio::test]
async fn joins_a_gesture_into_one_step_and_a_skipped_write_is_diffed_but_not_kept() {
    let b = Bed::new("jr-join");
    b.put("graph.k7x", "g1");
    b.record("node new", Mode::Step, |_| {
        b.put("fx.k7x", "node");
        b.put("graph.k7x", "g2");
    })
    .await;
    b.record("node insert", Mode::Join, |_| b.put("graph.k7x", "g3"))
        .await;
    assert_eq!(b.labels(), ["node new"]);
    let pin = b
        .record("graph pin", Mode::Skip, |_| b.put("graph.k7x", "g3 pinned"))
        .await;
    assert_eq!(
        pin.files,
        Some(vec![json!({ "path": "graph.k7x", "kind": "edited" })])
    );
    assert_eq!(b.labels(), ["node new"]);
    b.undo().await;
    assert_eq!(b.text("graph.k7x"), "g1");
    assert!(!b.exists("fx.k7x"));
}

#[tokio::test]
async fn a_process_step_journals_the_staged_pre_image_and_undoes_to_it() {
    let b = Bed::new("jr-process");
    b.put("r.wav", "old");
    b.put("r.k7x", "v1");
    let e = b
        .record("clip process", Mode::Step, |id| {
            b.j.snapshot(id, &b.at("r.wav"));
            b.put("r.wav", "new!");
            b.put("r.k7x", "v2");
        })
        .await;
    assert!(e.keep.is_empty(), "no engine bound: the copies stay here");
    let state = b.j.state_of(&b.home, &b.root, &b.dir);
    assert_eq!(state["nextUndo"]["kind"], json!("process"));
    assert_eq!(moved(&b.undo().await), "clip process");
    assert_eq!(
        (b.text("r.wav"), b.text("r.k7x")),
        ("old".into(), "v1".into())
    );
    b.redo().await;
    assert_eq!(b.text("r.wav"), "new!");
    // The staged copies are gone after the step: the store holds blobs and a journal.
    let staged =
        std::fs::read_dir(k7_file_service::journal::store::store_for(&b.home, &b.dir) + "/staged")
            .map(|d| d.count())
            .unwrap_or(0);
    assert_eq!(staged, 0);
}

#[tokio::test]
async fn a_binary_rewritten_to_the_same_bytes_is_no_step() {
    let b = Bed::new("jr-same");
    b.put("r.wav", "same");
    let e = b
        .record("render", Mode::Step, |id| {
            b.j.snapshot(id, &b.at("r.wav"));
            b.put("r.wav", "same");
        })
        .await;
    assert_eq!(e.files, Some(vec![]));
    assert!(b.labels().is_empty());
}

#[tokio::test]
async fn a_record_never_ended_is_abandoned_with_its_owner_and_the_gate_is_let_go() {
    let b = Bed::new("jr-abandon");
    b.put("a.k7x", "a1");
    let mut ids = Vec::new();
    for _ in 0..10 {
        ids.push(
            b.j.begin(&b.home, &b.root, Some(&b.dir), "lost", Mode::Step, Owner(7))
                .await
                .id,
        );
    }
    b.j.snapshot(ids[0], &b.at("a.k7x"));
    assert_eq!(b.j.in_flight(&b.root), 10);
    assert_eq!(b.j.abandon(Owner(7)), 10);
    assert_eq!(b.j.in_flight(&b.root), 0);
    // An undo no longer waits on them, and a late end of one is told it was abandoned.
    b.step("after", || b.put("a.k7x", "a2")).await;
    assert_eq!(
        moved(
            &tokio::time::timeout(Duration::from_secs(15), b.undo())
                .await
                .unwrap()
        ),
        "after"
    );
    assert!(b.j.end(ids[0], None).await.abandoned);
    // A lease runs out the same way, one record at a time, whoever began it (1745).
    let lone =
        b.j.begin(&b.home, &b.root, Some(&b.dir), "lone", Mode::Step, ME)
            .await;
    let other =
        b.j.begin(
            &b.home,
            &b.root,
            Some(&b.dir),
            "other",
            Mode::Step,
            Owner(9),
        )
        .await;
    assert!(b.j.holds(lone.id));
    assert!(b.j.abandon_record(lone.id));
    assert!(!b.j.abandon_record(lone.id));
    assert!(b.j.holds(other.id), "another owner's record stays");
    assert!(b.j.end(lone.id, None).await.abandoned);
    assert_eq!(b.j.abandon(Owner(9)), 1);
}

// 1726: a pre-image copied from an old file is dated now, so a sweep of crash leftovers never takes it from a live record.
#[tokio::test]
async fn a_staged_copy_of_an_old_file_is_dated_now_and_survives_the_sweep() {
    use k7_file_service::journal::store::{Store, store_for};
    let b = Bed::new("jr-stage-date");
    b.put("a.k7x", "a1");
    b.put("r.wav", "old audio");
    let day = std::time::SystemTime::now() - Duration::from_secs(86_400);
    std::fs::File::options()
        .write(true)
        .open(b.at("r.wav"))
        .unwrap()
        .set_modified(day)
        .unwrap();
    let begun =
        b.j.begin(&b.home, &b.root, Some(&b.dir), "bounce", Mode::Step, ME)
            .await;
    b.j.snapshot(begun.id, &b.at("r.wav"));
    let store = Store::new(store_for(&b.home, &b.dir));
    let staged: Vec<_> = std::fs::read_dir(store.staged_dir())
        .unwrap()
        .flatten()
        .collect();
    assert_eq!(staged.len(), 1);
    let age = staged[0]
        .metadata()
        .unwrap()
        .modified()
        .unwrap()
        .elapsed()
        .unwrap();
    assert!(age < Duration::from_secs(60), "{age:?}");
    assert_eq!(store.sweep(Duration::from_secs(3600)), 0);
    b.j.abort(begun.id);
}

// 1726: an old staged file a record in flight owns survives the sweep that a journal's first load runs.
#[tokio::test]
async fn an_old_staged_file_owned_by_an_open_record_survives_the_load_sweep() {
    use k7_file_service::journal::store::{Store, store_for};
    let b = Bed::new("jr-sweep-live");
    b.put("r.wav", "audio");
    let begun =
        b.j.begin(&b.home, &b.root, Some(&b.dir), "bounce", Mode::Step, ME)
            .await;
    b.j.snapshot(begun.id, &b.at("r.wav"));
    let store = Store::new(store_for(&b.home, &b.dir));
    let staged = std::fs::read_dir(store.staged_dir())
        .unwrap()
        .flatten()
        .next()
        .unwrap()
        .path();
    let old = std::time::SystemTime::now() - Duration::from_secs(86_400);
    std::fs::File::options()
        .write(true)
        .open(&staged)
        .unwrap()
        .set_modified(old)
        .unwrap();
    // Another project's journal loads first, so the next read of this one loads it afresh and sweeps.
    b.j.state_of(&b.home, &b.root, &format!("{}-other", b.dir));
    b.j.state_of(&b.home, &b.root, &b.dir);
    assert!(staged.exists(), "a live record's pre-image was swept");
    b.j.abort(begun.id);
}

#[tokio::test]
async fn many_records_of_one_root_at_once_are_all_journalled() {
    let b = Arc::new(Bed::new("jr-thousand"));
    let mut begun = Vec::new();
    for n in 0..20 {
        begun.push((
            n,
            b.j.begin(
                &b.home,
                &b.root,
                Some(&b.dir),
                &format!("r{n}"),
                Mode::Step,
                ME,
            )
            .await,
        ));
    }
    assert_eq!(b.j.in_flight(&b.root), 20);
    for (n, r) in begun {
        b.put(&format!("f{n}.k7x"), "x");
        b.j.end(r.id, None).await;
    }
    // Each record began before any file existed, so the first to land owns what it wrote and the rebase keeps the rest apart.
    let steps = b.labels().len();
    assert_eq!(steps, 20);
}

#[tokio::test]
async fn a_walk_past_its_limit_journals_nothing_and_says_so() {
    let limits = k7_file_service::tree::Limits {
        max_seen: 5,
        ..Default::default()
    };
    let b = Bed::with("jr-limit", limits, 1000);
    for n in 0..10 {
        b.put(&format!("f{n}.k7x"), "x");
    }
    let e = b.step("big", || b.put("f0.k7x", "y")).await;
    assert_eq!(e.files, None);
    assert!(b.labels().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_records_ending_at_once_journal_each_write_once() {
    for round in 0..8 {
        let b = Arc::new(Bed::new(&format!("jr-race-{round}")));
        // Enough files that one end's after walk is long enough for the other to overlap it.
        for n in 0..200 {
            b.put(&format!("pad/f{n}.k7x"), "x");
        }
        let mut ids = Vec::new();
        for label in ["one", "two"] {
            let r =
                b.j.begin(&b.home, &b.root, Some(&b.dir), label, Mode::Step, ME)
                    .await;
            ids.push((label, r.id));
        }
        let gate = Arc::new(std::sync::Barrier::new(2));
        let ends: Vec<_> = ids
            .into_iter()
            .map(|(label, id)| {
                let (b, gate) = (Arc::clone(&b), Arc::clone(&gate));
                std::thread::spawn(move || {
                    gate.wait();
                    b.put(&format!("{label}.k7x"), label);
                    b.j.end_now(id, None)
                })
            })
            .collect();
        for e in ends {
            e.join().unwrap();
        }
        let entries = b.j.list_of(&b.home, &b.root, &b.dir);
        let in_steps = |f: &str| {
            entries
                .iter()
                .filter(|e| {
                    let hit = |p: &Value| p.as_str().is_some_and(|p| p.ends_with(f));
                    e["paths"].as_array().unwrap().iter().any(hit)
                })
                .count()
        };
        assert_eq!(
            (in_steps("one.k7x"), in_steps("two.k7x")),
            (1, 1),
            "round {round}: {entries:?}"
        );
        // When the two writes are two steps, undoing the top one leaves the other's write in place.
        if entries.len() == 2 {
            let top = moved(&b.undo().await);
            let (gone, kept) = if top == "one" {
                ("one.k7x", "two.k7x")
            } else {
                ("two.k7x", "one.k7x")
            };
            assert!(
                !b.exists(gone) && b.exists(kept),
                "round {round}: undo of {top}"
            );
        }
    }
}

#[cfg(unix)]
#[tokio::test]
async fn a_step_that_fails_to_land_leaves_nothing_in_memory_the_disk_lacks() {
    use std::os::unix::fs::PermissionsExt;
    let b = Bed::new("jr-land-fails");
    b.put("a.k7x", "a1");
    b.step("first", || b.put("a.k7x", "a2")).await;
    let store = k7_file_service::journal::store::store_for(&b.home, &b.dir);
    let mode = |m| std::fs::set_permissions(&store, std::fs::Permissions::from_mode(m)).unwrap();
    mode(0o555);
    // A process that ignores permissions (root) writes anyway: nothing to prove there.
    if std::fs::File::create(format!("{store}/probe")).is_ok() {
        mode(0o755);
        return;
    }
    b.put("r.wav", "old");
    let failed = b
        .record("second", Mode::Step, |id| {
            b.j.snapshot(id, &b.at("r.wav"));
            b.put("r.wav", "new");
            b.put("a.k7x", "a3");
        })
        .await;
    mode(0o755);
    assert!(failed.failure.is_some(), "the save was refused");
    assert_eq!(b.labels(), ["first"], "the journal is the disk's again");
    b.step("third", || b.put("a.k7x", "a4")).await;
    assert_eq!(b.labels(), ["first", "third"]);
    assert_eq!(moved(&b.undo().await), "third");
    assert_eq!(moved(&b.undo().await), "first");
    assert_eq!(b.text("a.k7x"), "a1");
}
