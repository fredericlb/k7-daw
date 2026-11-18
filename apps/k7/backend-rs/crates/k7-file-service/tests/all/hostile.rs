//! Hostile trees (1051, 1052): a folder of 20,000 entries, a nest 300 deep, names at the file system's limit, and
//! hidden entries alone passing `max_seen`. Each answered fast, asserting the answer and the time.

use crate::support;

use std::time::{Duration, Instant};

use k7_file_service::space::{Declined, Space};
use k7_file_service::tree::{Limits, tree};
use support::Scratch;

const BOUND: Duration = Duration::from_secs(if cfg!(target_env = "musl") { 60 } else { 15 });

#[test]
fn a_folder_of_twenty_thousand_entries_is_listed_fast_and_sorted() {
    wide_listing(20_000);
}

#[test]
#[ignore = "scale: a quadratic listing hides at 20,000 entries and shows at 200,000"]
fn a_folder_of_two_hundred_thousand_entries_is_listed_fast_and_sorted_at_scale() {
    wide_listing(200_000);
}

fn wide_listing(n: u32) {
    let s = Scratch::new("wide");
    let dir = s.dir("ws/wide");
    // Eight writers: a folder this wide takes minutes to fill on one thread when the machine is busy.
    std::thread::scope(|scope| {
        for part in 0..8u32 {
            let dir = &dir;
            scope.spawn(move || {
                for i in (part * n / 8)..((part + 1) * n / 8) {
                    std::fs::File::create(format!("{dir}/f{i:06}.txt")).unwrap();
                }
            });
        }
    });
    let h = s.home(false);
    // The bound is on the best of three, so a busy machine's one slow turn is not the code's.
    let mut best = Duration::MAX;
    for _ in 0..3 {
        let t0 = Instant::now();
        let t = tree(&Space::open(&h, &s.path("ws")), &Limits::default()).unwrap();
        best = best.min(t0.elapsed());
        assert_eq!((t.entries, t.unlistable, t.depth), (u64::from(n) + 1, 0, 1));
        let v: serde_json::Value = serde_json::from_str(&t.json).unwrap();
        let last = format!("f{:06}.txt", n - 1);
        let kids = v[0]["children"].as_array().unwrap();
        assert_eq!(
            (
                kids[0]["name"].as_str(),
                kids[n as usize - 1]["name"].as_str()
            ),
            (Some("f000000.txt"), Some(last.as_str()))
        );
    }
    assert!(best < BOUND, "{best:?}");
    // Past the limit it is declined, and fast.
    let t0 = Instant::now();
    assert_eq!(
        tree(
            &Space::open(&h, &s.path("ws")),
            &Limits {
                max_seen: u64::from(n) / 2,
                ..Limits::default()
            }
        )
        .unwrap_err(),
        Declined::TooMany
    );
    assert!(t0.elapsed() < BOUND);
}

#[test]
fn a_nest_three_hundred_deep_is_declined_and_the_stack_holds() {
    let s = Scratch::new("deep");
    let rel = "d/".repeat(300);
    s.dir(&format!("ws/{rel}"));
    let h = s.home(false);
    let t0 = Instant::now();
    let r = tree(&Space::open(&h, &s.path("ws")), &Limits::default());
    assert_eq!(r.unwrap_err(), Declined::TooDeep);
    // Within the limit the walk finishes, on a thread with a small stack: nothing recurses.
    let limits = Limits {
        max_depth: 400,
        ..Limits::default()
    };
    let ws = s.path("ws");
    let home = s.home(false);
    let t = std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(move || tree(&Space::open(&home, &ws), &limits).unwrap())
        .unwrap()
        .join()
        .unwrap();
    assert_eq!((t.depth, t.entries), (300, 300));
    assert!(t0.elapsed() < BOUND);
}

#[test]
fn names_at_the_file_system_s_limit_are_written_whole() {
    let s = Scratch::new("long-names");
    let long = "n".repeat(250);
    s.file(&format!("ws/{long}.txt"), "");
    s.dir(&format!("ws/{}", "é".repeat(100)));
    let h = s.home(false);
    let t = tree(&Space::open(&h, &s.path("ws")), &Limits::default()).unwrap();
    let v: serde_json::Value = serde_json::from_str(&t.json).unwrap();
    assert_eq!(v[1]["name"].as_str().map(str::len), Some(254));
    assert_eq!(v[0]["name"].as_str().map(str::len), Some(200));
}

#[test]
fn hidden_entries_alone_pass_the_limit() {
    let s = Scratch::new("hidden-many");
    s.file("ws/shown.txt", "");
    for i in 0..300 {
        s.file(&format!("ws/.cache{i}"), "");
    }
    let h = s.home(false);
    let r = tree(
        &Space::open(&h, &s.path("ws")),
        &Limits {
            max_seen: 100,
            ..Limits::default()
        },
    );
    assert_eq!(r.unwrap_err(), Declined::TooMany);
}
