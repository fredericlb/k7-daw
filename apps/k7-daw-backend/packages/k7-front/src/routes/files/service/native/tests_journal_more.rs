//! More of the journal's routes through a real account: the guards of a record the front began, and a verb that stays pending.

use k7_file_service::journal::{Journals, Mode as Bracket, Owner};

use serde_json::json;

use super::Tag;
use super::answering::{ACCOUNT_SHARE, BURST, Gate, PROCESS_WEIGHT, PUT_BASE};
use super::tests::counted;
use super::tests_journal::Bed;

// 1745: a save's task that panics after its record began drops the guard, and the record leaves the gate.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_save_task_that_panics_lets_its_record_go() {
    let b = Bed::new("j-guard").await;
    let j = Journals::global();
    let begun = j
        .begin(
            &b.home(),
            &b.gate_root(),
            Some(&b.project),
            "edit",
            Bracket::Step,
            Owner::mint(),
        )
        .await;
    assert_eq!(j.in_flight(&b.gate_root()), 1);
    let ran = tokio::spawn(async move {
        let _open = super::journal::Open(begun.id);
        panic!("the save died");
    })
    .await;
    assert!(ran.is_err());
    assert_eq!(j.in_flight(&b.gate_root()), 0);
    let _ = std::fs::remove_dir_all(&b.dir);
}

// 1799: an undo waits for the records in flight only so long, then says a command is still running; it works once they end.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_undo_behind_a_record_that_never_ends_is_a_409_after_its_wait() {
    let b = Bed::new("j-wait").await;
    assert_eq!(b.save("p.k7/a.k7x", "v1").await.0, 200);
    super::journal_undo::tests_wait::set(&b.gate_root(), std::time::Duration::from_millis(400));
    let j = Journals::global();
    let open = j
        .begin(
            &b.home(),
            &b.gate_root(),
            Some(&b.project),
            "verb",
            Bracket::Skip,
            Owner::mint(),
        )
        .await;
    let (status, body) = b.ans(Tag::Undo, json!({})).await;
    assert_eq!(
        (status, body["error"].as_str()),
        (409, Some("a command is still running on this workspace"))
    );
    assert_eq!(b.file("a.k7x"), "v1", "nothing was undone");
    j.abort(open.id);
    let (status, body) = b.ans(Tag::Undo, json!({})).await;
    assert_eq!((status, body["applied"].clone()), (200, json!(true)));
    let _ = std::fs::remove_dir_all(&b.dir);
}

/// `n` saves of a few bytes at once through the route under `gate`, each one's status or None where it is declined.
async fn burst(b: &Bed, gate: &'static Gate, n: usize) -> Vec<Option<u16>> {
    let saves = (0..n).map(|i| {
        let mut call = b.call(
            Tag::FilePut,
            json!({ "path": format!("p.k7/f{i}.txt"), "content": format!("file {i}\n") }),
        );
        call.native.as_mut().unwrap().gate = gate;
        async move {
            let native = call.native.as_ref().unwrap();
            super::journal::answer(&call, native)
                .await
                .map(|r| r.status().as_u16())
        }
    });
    futures_util::future::join_all(saves).await
}

// 1780a: two small saves at once fit a share that only their own bytes fill; the old fixed 4 MiB each would not fit it.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_small_saves_at_once_answer_natively() {
    let b = Bed::new("j-burst").await;
    let share = 2 * (PUT_BASE + 2_000);
    let gate: &'static Gate = Box::leak(Box::new(
        Gate::new(PROCESS_WEIGHT, share).waiting(std::time::Duration::from_millis(300)),
    ));
    let answers = burst(&b, gate, 2).await;
    assert!(answers.iter().all(|a| *a == Some(200)), "{answers:?}");
    assert_eq!(counted(&super::stats::global().shed), 0);
    let _ = std::fs::remove_dir_all(&b.dir);
}

// 1780a: the bed's burst, 340 real saves under the real share; minutes in a debug build under load, so it is a measurement.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "measures 340 real saves at once; run with --ignored"]
async fn measure_340_real_saves_at_once_all_answer_natively() {
    let b = Bed::new("j-burst340").await;
    let gate: &'static Gate = Box::leak(Box::new(Gate::new(PROCESS_WEIGHT, ACCOUNT_SHARE)));
    let answers = burst(&b, gate, 340).await;
    assert!(answers.iter().all(|a| *a == Some(200)), "{answers:?}");
    assert_eq!(counted(&super::stats::global().shed), 0);
    let _ = std::fs::remove_dir_all(&b.dir);
}

// 1780a: a save heavier than half a share still declines past it (a 1 MiB share stands for the real 128 MiB), counted shed and lone, since no burst held them.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_save_over_half_the_share_declines_busy_past_it() {
    let b = Bed::new("j-big").await;
    let share = 1 << 20;
    let gate: &'static Gate = Box::leak(Box::new(
        Gate::new(PROCESS_WEIGHT, share).waiting(std::time::Duration::from_millis(150)),
    ));
    let account = b.call(Tag::FilePut, json!({})).native.unwrap().account;
    let big = "x".repeat(400 << 10);
    let weight = super::journal::put_weight(big.len(), 0, 0);
    assert!(weight > share / 2 && weight <= share);
    let _held = gate.take(&account, weight).await.unwrap();
    let mut call = b.call(
        Tag::FilePut,
        json!({ "path": "p.k7/big.txt", "content": big }),
    );
    call.native.as_mut().unwrap().gate = gate;
    let native = call.native.as_ref().unwrap();
    assert!(
        super::journal::answer(&call, native).await.is_none(),
        "declined"
    );
    assert_eq!(
        (
            counted(&super::stats::global().shed),
            counted(&super::stats::global().shed_lone),
            counted(&super::stats::global().declined)
        ),
        (1, 1, 0)
    );
    // A burst of waiters around it is not lone.
    let _crowd: Vec<_> = {
        let mut v = Vec::new();
        for _ in 0..BURST {
            v.push(gate.take(&account, 1).await.unwrap());
        }
        v
    };
    assert!(super::journal::answer(&call, native).await.is_none());
    assert_eq!(
        (
            counted(&super::stats::global().shed),
            counted(&super::stats::global().shed_lone)
        ),
        (2, 1)
    );
    let _ = std::fs::remove_dir_all(&b.dir);
}

/// A save of `content` to `path` under `gate`, with `base` when given; whether the native side answered it.
async fn one_save(b: &Bed, gate: &'static Gate, path: &str, base: Option<&str>) -> bool {
    let mut body = json!({ "path": path, "content": "x\n" });
    if let Some(h) = base {
        body["base"] = json!(h);
    }
    let mut call = b.call(Tag::FilePut, body);
    call.native.as_mut().unwrap().gate = gate;
    let native = call.native.as_ref().unwrap();
    super::journal::answer(&call, native).await.is_some()
}

fn small_gate(share: usize) -> &'static Gate {
    Box::leak(Box::new(
        Gate::new(PROCESS_WEIGHT, share).waiting(std::time::Duration::from_millis(150)),
    ))
}

// 1780a: the base's file counts twice in a save's weight; the same save without a base fits what is left.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_save_with_a_big_base_file_declines_where_one_without_a_base_fits() {
    let b = Bed::new("j-base").await;
    std::fs::write(
        std::path::Path::new(&b.project).join("big.txt"),
        "y".repeat(400 << 10),
    )
    .unwrap();
    let share = 1 << 20;
    let gate = small_gate(share);
    let account = b.call(Tag::FilePut, json!({})).native.unwrap().account;
    let _half = gate.take(&account, share / 2).await.unwrap();
    assert!(!one_save(&b, gate, "p.k7/big.txt", Some("0")).await);
    assert_eq!(counted(&super::stats::global().shed), 1);
    assert!(one_save(&b, gate, "p.k7/small.txt", None).await);
    let _ = std::fs::remove_dir_all(&b.dir);
}

// 1780a: a save also carries the project's image: once the last image is big, saves decline past the share.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_save_on_a_project_with_a_big_image_declines_past_the_share() {
    let b = Bed::new("j-image").await;
    std::fs::write(
        std::path::Path::new(&b.project).join("big.txt"),
        "y".repeat(500 << 10),
    )
    .unwrap();
    let share = 1 << 20;
    let gate = small_gate(share);
    let account = b.call(Tag::FilePut, json!({})).native.unwrap().account;
    assert!(
        one_save(&b, gate, "p.k7/a.txt", None).await,
        "primes the image"
    );
    assert!(Journals::global().image_bytes(&b.gate_root()) >= 500 << 10);
    let _half = gate.take(&account, share / 2).await.unwrap();
    assert!(!one_save(&b, gate, "p.k7/b.txt", None).await);
    assert_eq!(counted(&super::stats::global().shed), 1);
    let _ = std::fs::remove_dir_all(&b.dir);
}

const SECONDS_LANE: &str = "extends(\"@meta/track\")\nevents([ref(\"hit\").start(4.5)])\n";

// O2: a save of a lane placing an event in seconds raises `format` natively, with no ask of the runtime.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_save_of_a_lane_in_seconds_raises_the_format_natively() {
    let b = Bed::new("j-seconds").await;
    std::fs::write(
        std::path::Path::new(&b.project).join("project.k7x"),
        "project({ name: \"p\" })\n",
    )
    .unwrap();
    assert!(!b.file("project.k7x").contains("format"));
    assert_eq!(b.save("p.k7/nodes/fx.k7x", SECONDS_LANE).await.0, 200);
    assert!(
        b.file("project.k7x").contains("format(2)"),
        "{}",
        b.file("project.k7x")
    );
    let calls = super::entries::calls::of(&b.root);
    assert!(calls.iter().any(|c| c == "stamp"));
    assert!(!calls.iter().any(|c| c == "stampFormat"));
    let _ = std::fs::remove_dir_all(&b.dir);
}

// O2: a lane in musical positions raises nothing.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_lane_in_musical_positions_raises_nothing() {
    let b = Bed::new("j-musical").await;
    let lane = "extends(\"@meta/track\")\nevents([ref(\"hit\").start(\"2:1:1\")])\n";
    assert_eq!(b.save("p.k7/nodes/fx.k7x", lane).await.0, 200);
    assert!(!b.file("project.k7x").contains("format"));
    let _ = std::fs::remove_dir_all(&b.dir);
}
