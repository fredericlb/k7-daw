//! The bench and the slot's guards at the front: an empty libraries home is handed back untouched, the bench waits for the journal's gate, the hub's ask is native.

use std::time::Duration;

use super::bench::slot_for_hub;
use super::tests_bench::{bench_file, fixture, run_in};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_machine_with_no_library_is_handed_back_with_nothing_written() {
    let bare = fixture("b-nolib", true);
    std::fs::remove_dir_all(bare.join("libraries/k7-core")).unwrap();
    let run = run_in(&bare);
    assert!(run.bench("@k7-core/solo/live.k7").await.is_none());
    assert!(run.slot("@k7-core/solo").await.is_none());
    assert!(!bare.join("home/workspace/Banc/live.k7").exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_bench_waits_for_the_journals_gate() {
    let dir = fixture("b-gate", true);
    let run = run_in(&dir);
    let root = run.home.studio.clone() + "/workspace/Banc";
    let held = k7_file_service::journal::Journals::global()
        .exclusive(&k7_file_service::paths::resolve_all(&[&root]))
        .await;
    let task = tokio::spawn(async move { run.bench("@k7-core/solo/live.k7").await });
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!task.is_finished(), "the bench opened inside a held gate");
    assert!(!bench_file(&dir, "graph.k7x").exists());
    drop(held);
    assert_eq!(task.await.unwrap().unwrap().0, 200);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_hubs_slot_is_answered_natively() {
    let dir = fixture("b-hub", true);
    let (status, slot) = slot_for_hub(run_in(&dir), "@k7-core/solo")
        .await
        .expect("answered natively");
    assert_eq!((status, slot["id"].as_str()), (200, Some("solo")));
}
