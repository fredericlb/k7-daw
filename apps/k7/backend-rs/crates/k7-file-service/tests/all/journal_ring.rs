//! The ring: `history.test.ts`' cases of `ProjectHistory` that need no disk — the redo tail dropped by a push, the list, the
//! eviction, the join, the peeks — and the blob names a push and an eviction hand back (1740).

use k7_file_service::journal::format::{Change, EngineRef, Journal};
use k7_file_service::journal::ring::{
    Joined, NewBlob, NewEntry, join, list, peek_redo, peek_undo, push, state,
};
use serde_json::json;

fn text(label: &str, p: &str, before: Option<&str>, after: Option<&str>) -> NewEntry {
    NewEntry {
        label: label.into(),
        changes: vec![Change::new(
            p,
            before.map(Into::into),
            after.map(Into::into),
        )],
        blobs: vec![],
    }
}

fn labels(j: &Journal) -> Vec<&str> {
    j.entries.iter().map(|e| e.label.as_str()).collect()
}

#[test]
fn a_fresh_push_truncates_the_redo_tail_and_the_ids_go_on_counting() {
    let mut j = Journal::empty();
    push(&mut j, text("a", "a.k7x", None, Some("A")), 1, 1000, None);
    push(&mut j, text("b", "b.k7x", None, Some("B")), 2, 1000, None);
    j.cursor -= 1;
    assert_eq!(peek_redo(&j).unwrap()["files"], json!(["b.k7x"]));
    let p = push(&mut j, text("c", "c.k7x", None, Some("C")), 3, 1000, None).unwrap();
    assert_eq!((p.id, labels(&j), j.cursor), (3, vec!["a", "c"], 2));
    assert!(peek_redo(&j).is_none());
    assert_eq!(peek_undo(&j).unwrap()["files"], json!(["c.k7x"]));
}

#[test]
fn lists_every_entry_oldest_first_with_paths_and_the_undone_marked() {
    let mut j = Journal::empty();
    push(
        &mut j,
        text("edit a.k7x", "a.k7x", None, Some("A")),
        1,
        1000,
        None,
    );
    push(
        &mut j,
        text("edit b.k7x", "b.k7x", None, Some("B")),
        2,
        1000,
        None,
    );
    j.cursor = 1;
    let l = list(&j);
    assert_eq!(
        l[0],
        json!({"id": 1, "ts": 1, "kind": "text", "label": "edit a.k7x", "paths": ["a.k7x"], "undone": false})
    );
    assert_eq!(l[1]["undone"], json!(true));
    assert!(l[0].get("changes").is_none());
    let s = state(&j);
    assert_eq!(
        (s["canUndo"].clone(), s["canRedo"].clone()),
        (json!(true), json!(true))
    );
    assert_eq!(s["nextUndo"]["label"], json!("edit a.k7x"));
    assert!(state(&Journal::empty()).as_object().unwrap().len() == 2);
}

#[test]
fn keeps_at_most_its_rings_size_evicting_the_oldest_and_naming_their_blobs() {
    let mut j = Journal::empty();
    let mut dropped = vec![];
    for n in 0..25 {
        let mut e = text(&format!("e{n}"), &format!("f{n}.k7x"), None, Some("x"));
        if n == 2 {
            e.blobs = vec![NewBlob {
                path: "r.wav".into(),
                before: true,
                after: true,
            }];
        }
        dropped.extend(push(&mut j, e, n, 20, None).unwrap().dropped);
    }
    assert_eq!(
        (
            j.entries.len(),
            j.cursor,
            j.entries[0].label.as_str(),
            j.entries[19].label.as_str()
        ),
        (20, 20, "e5", "e24")
    );
    assert_eq!(
        dropped,
        ["3-b0", "3-a0"],
        "entry 3 (id) was a process step evicted with its blobs"
    );
}

#[test]
fn a_process_step_names_its_blobs_and_the_engine_that_keeps_them() {
    let mut j = Journal::empty();
    let e = NewEntry {
        label: "bounce".into(),
        changes: vec![],
        blobs: vec![
            NewBlob {
                path: "r.wav".into(),
                before: true,
                after: true,
            },
            NewBlob {
                path: "new.wav".into(),
                before: false,
                after: true,
            },
        ],
    };
    let p = push(&mut j, e, 9, 1000, Some(EngineRef::new("e-a", "studio A"))).unwrap();
    assert_eq!(
        p.stored,
        [
            (Some("1-b0".into()), Some("1-a0".into())),
            (None, Some("1-a1".into()))
        ]
    );
    assert_eq!(
        j.entries[0].kept_on,
        Some(Some(EngineRef::new("e-a", "studio A")))
    );
    // A push with no blobs carries no keptOn at all.
    push(
        &mut j,
        text("t", "a", None, Some("A")),
        10,
        1000,
        Some(EngineRef::new("x", "y")),
    );
    assert_eq!(j.entries[1].kept_on, None);
    // Dropping the redo tail names the blobs of the steps it drops.
    j.cursor = 0;
    let p = push(&mut j, text("u", "a", None, Some("A")), 11, 1000, None).unwrap();
    assert_eq!(p.dropped, ["1-b0", "1-a0", "1-a1"]);
}

#[test]
fn joins_a_change_into_the_step_on_top_keeping_each_files_first_before() {
    let mut j = Journal::empty();
    let first = NewEntry {
        label: "node new".into(),
        changes: vec![
            Change::new("fx.k7x", None, Some("node".into())),
            Change::new("graph.k7x", Some("g1".into()), Some("g2".into())),
        ],
        blobs: vec![],
    };
    push(&mut j, first, 1, 1000, None);
    let r = join(
        &mut j,
        text("node insert", "graph.k7x", Some("g2"), Some("g3")),
        2,
        1000,
        None,
    );
    assert_eq!(r, Joined::Folded);
    assert_eq!(j.entries.len(), 1);
    let c = &j.entries[0].changes;
    assert_eq!(
        (c[1].before.as_deref(), c[1].after.as_deref()),
        (Some("g1"), Some("g3"))
    );
    assert_eq!(j.entries[0].ts, 2.into());
}

#[test]
fn joins_nothing_past_an_undo_and_drops_a_step_that_ends_where_it_began() {
    let mut j = Journal::empty();
    assert!(matches!(
        join(&mut j, text("a", "a.k7x", None, Some("A")), 1, 1000, None),
        Joined::Pushed(_)
    ));
    j.cursor = 0;
    assert!(matches!(
        join(&mut j, text("b", "b.k7x", None, Some("B")), 2, 1000, None),
        Joined::Pushed(_)
    ));
    assert_eq!(labels(&j), ["b"]);
    assert_eq!(
        join(&mut j, text("b", "b.k7x", Some("B"), None), 3, 1000, None),
        Joined::Dropped
    );
    assert!(j.entries.is_empty() && j.cursor == 0);
    assert_eq!(
        join(
            &mut j,
            NewEntry {
                label: "n".into(),
                changes: vec![],
                blobs: vec![]
            },
            4,
            1000,
            None
        ),
        Joined::Nothing
    );
    // Audio is never folded.
    push(&mut j, text("t", "a", None, Some("A")), 5, 1000, None);
    let audio = NewEntry {
        label: "w".into(),
        changes: vec![],
        blobs: vec![NewBlob {
            path: "r.wav".into(),
            before: false,
            after: true,
        }],
    };
    assert!(matches!(
        join(&mut j, audio, 6, 1000, None),
        Joined::Pushed(_)
    ));
    assert_eq!(j.entries.len(), 2);
}

#[test]
fn records_nothing_for_a_change_that_holds_nothing() {
    let mut j = Journal::empty();
    assert!(
        push(
            &mut j,
            NewEntry {
                label: "noop".into(),
                changes: vec![],
                blobs: vec![]
            },
            1,
            1000,
            None
        )
        .is_none()
    );
    assert_eq!(j.next_id, 1);
}
