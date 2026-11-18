//! The Logs ring (2620): its bound, its cut text, the cursor rule and a hostile push, each answered without a hang.

use std::time::{Duration, Instant};

use k7_file_service::runtime::ring::{Level, Ring};
use k7_file_service::runtime::{LINES, TEXT};

#[test]
fn keeps_the_last_thousand_lines_numbered_from_one() {
    let mut r = Ring::new();
    for i in 0..LINES + 5 {
        r.push(Level::Info, &i.to_string(), 0);
    }
    let (seq, all) = r.since(None, 5000);
    assert_eq!((seq, all.len(), all[0].seq), (1005, LINES, 6));
}

#[test]
fn a_cursor_past_the_ring_answers_from_the_start_and_an_inside_one_the_rest() {
    let mut r = Ring::new();
    for i in 0..5 {
        r.push(Level::Warn, &i.to_string(), i);
    }
    assert_eq!(r.since(Some(9.0), 2).1.len(), 2);
    assert_eq!(r.since(Some(5.0), 2).1.len(), 0);
    assert_eq!(
        r.since(Some(2.5), 9)
            .1
            .iter()
            .map(|l| l.seq)
            .collect::<Vec<_>>(),
        [3, 4, 5]
    );
    assert_eq!(r.since(Some(-1.0), 9).1.len(), 5);
}

#[test]
fn a_ten_megabyte_line_is_cut_on_a_character_boundary() {
    let mut r = Ring::new();
    r.push(Level::Error, &"é".repeat(5 << 20), 1);
    let t = &r.since(None, 1).1[0].text;
    assert!(t.len() <= TEXT + 4 && t.ends_with(" …"), "{}", t.len());
    r.push(Level::Info, "a\0b", 2);
    assert_eq!(r.since(None, 1).1[0].text, "a\0b");
}

#[test]
fn a_push_is_constant_work_at_n_and_four_n() {
    let time = |n: usize| {
        let mut r = Ring::new();
        let t = Instant::now();
        for _ in 0..n {
            r.push(Level::Info, "line", 0);
        }
        t.elapsed()
    };
    let bound = Duration::from_secs(if cfg!(target_env = "musl") { 60 } else { 15 });
    let (a, b) = (time(100_000), time(400_000));
    assert!(b < bound && b < a * 40, "{a:?} then {b:?}");
}
