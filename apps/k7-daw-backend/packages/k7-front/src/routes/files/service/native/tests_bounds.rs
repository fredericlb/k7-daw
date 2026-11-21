//! Tests of the bounds on memory (1057): what an answer may hold, and what it does when the gate is full.

use std::time::Duration;

use super::Tag;
use super::answering::{BASE_WEIGHT, BURST, Gate};
use super::reads::run;
use super::tests::{Lines, counted, ctx, scratch};

fn gate(process: usize, share: usize) -> &'static Gate {
    Box::leak(Box::new(
        Gate::new(process, share).waiting(Duration::from_millis(100)),
    ))
}

#[tokio::test]
async fn a_gate_holds_a_weight_for_the_process_and_a_share_for_each_account() {
    let g = gate(3 * BASE_WEIGHT, 2 * BASE_WEIGHT);
    let a1 = g.take("a", BASE_WEIGHT).await.unwrap();
    let a2 = g.take("a", BASE_WEIGHT).await.unwrap();
    // The account's share is spent; another account still has the process's third.
    assert!(g.take("a", BASE_WEIGHT).await.is_none());
    let b = g.take("b", BASE_WEIGHT).await.unwrap();
    assert_eq!(g.free(), 0);
    assert!(g.take("c", BASE_WEIGHT).await.is_none());
    drop((a1, a2, b));
    assert_eq!(g.free(), 3 * BASE_WEIGHT);
    // Growing takes nothing where either side lacks it.
    let mut held = g.take("a", BASE_WEIGHT).await.unwrap();
    assert!(!held.grow(2 * BASE_WEIGHT));
    assert_eq!(g.free(), 2 * BASE_WEIGHT);
    assert!(held.grow(BASE_WEIGHT));
    assert_eq!(g.free(), BASE_WEIGHT);
}

#[tokio::test]
async fn a_read_the_gate_cannot_weigh_is_a_busy_decline() {
    let dir = scratch("busy");
    let ws = dir.join("home/workspace/songs");
    std::fs::write(ws.join("small.txt"), "s").unwrap();
    // 6 MiB held three times over is more than the 8 MiB of the gate has beside the base.
    std::fs::write(ws.join("big.txt"), "b".repeat(6 << 20)).unwrap();
    let lines = Lines::default();
    let shed = || counted(&super::tests::stats_of(&dir).shed);
    let before = shed();
    let mut c = ctx(&dir, Tag::File, "small.txt", &lines);
    c.gate = gate(2 * BASE_WEIGHT, 2 * BASE_WEIGHT);
    assert!(run(&c).await.is_some());
    assert_eq!(c.gate.free(), 2 * BASE_WEIGHT, "the weight came back");
    c.rel = "big.txt".into();
    assert!(run(&c).await.is_none());
    assert_eq!(shed(), before + 1);
    assert_eq!(c.gate.free(), 2 * BASE_WEIGHT);
    // A tree weighs more than this gate has at all.
    let mut t = ctx(&dir, Tag::Tree, "", &lines);
    t.gate = c.gate;
    assert!(run(&t).await.is_none());
    assert_eq!(shed(), before + 2);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_shed_is_lone_below_the_burst_and_never_a_decline() {
    let stats = super::stats::Stats::default();
    stats.shed(BURST - 1);
    stats.shed(BURST);
    assert_eq!(
        (
            counted(&stats.shed),
            counted(&stats.shed_lone),
            counted(&stats.declined)
        ),
        (2, 1, 0)
    );
}
