// pictures_cover.rs — the backward sweep's Cover equals the TypeScript's subtract per shot, and holds its edge cases (506, 507).
// A reader would get wrong what is compared: the free pieces of each shot, over seeded lists whose spans touch, nest and repeat.

use crate::pictures_support;

use pictures_support::Rng;
use song::pictures::{Cover, Spend};

/// `[a, b)` with `[c, d)` cut out of it, as `subtract` in videoSound.ts.
fn subtract(spans: &[(f64, f64)], c: f64, d: f64) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    for &(a, b) in spans {
        if d <= a || c >= b {
            out.push((a, b));
        } else {
            if c > a {
                out.push((a, c));
            }
            if d < b {
                out.push((d, b));
            }
        }
    }
    out
}

type Answer = (Vec<(f64, f64)>, Vec<Option<f64>>);

/// The shots as (start, end, fade_in): the TypeScript's loop over every later shot per shot, and the cross at each piece's end.
fn naive(shots: &[(f64, f64, f64)]) -> Vec<Answer> {
    let mut out = Vec::new();
    for (i, &(start, end, _)) in shots.iter().enumerate() {
        let mut spans = vec![(start, end)];
        let mut cross: Vec<(f64, f64)> = Vec::new();
        for &(s, e, f) in &shots[i + 1..] {
            let from = s + f;
            if from < e {
                spans = subtract(&spans, from, e);
            }
            // `crossAt.set` in file order: the last later shot of a key wins.
            if f > 0.0 {
                match cross.iter_mut().find(|(k, _)| *k == from) {
                    Some(slot) => slot.1 = f,
                    None => cross.push((from, f)),
                }
            }
        }
        spans.retain(|&(a, b)| b > a);
        let at = spans
            .iter()
            .map(|&(_, b)| cross.iter().find(|(k, _)| *k == b).map(|&(_, f)| f))
            .collect();
        out.push((spans, at));
    }
    out
}

fn sweep(shots: &[(f64, f64, f64)]) -> Vec<Answer> {
    let mut spend = Spend::with_limit(usize::MAX);
    let mut cover = Cover::new();
    let mut out = Vec::new();
    for &(start, end, fade) in shots.iter().rev() {
        let free = cover.free(start, end, &mut spend).expect("within the budget");
        let at = free.iter().map(|&(_, b)| cover.cross_at(b)).collect();
        out.push((free, at));
        assert!(cover.hold(start, end, fade, &mut spend));
    }
    out.reverse();
    out
}

#[test]
fn the_sweep_equals_a_subtract_per_shot_over_twenty_thousand_seeded_lists() {
    // decision 506: the union's complement is what subtracting half-open spans one by one leaves; 507: the last in the file wins a tie
    let grid = [0.0, 0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0, 8.0];
    let mut rng = Rng(0x2e7f_1b33);
    for case in 0..20_000 {
        let n = 1 + rng.below(9) as usize;
        let shots: Vec<(f64, f64, f64)> = (0..n)
            .map(|_| {
                let start = rng.pick(&grid);
                let end = if rng.below(8) == 0 {
                    f64::INFINITY
                } else {
                    start + rng.pick(&[0.5, 1.0, 1.5, 2.0, 4.0])
                };
                let fade: f64 = if rng.below(2) == 0 {
                    0.0
                } else {
                    rng.pick(&[0.5, 1.0, 1.5, 2.0])
                };
                (start, end, fade.min(end - start))
            })
            .collect();
        assert_eq!(naive(&shots), sweep(&shots), "case {case}: {shots:?}");
    }
}

#[test]
fn a_cover_joins_touching_spans_and_answers_what_is_left() {
    // decision 506: adjacent, nested and identical spans are one covered set
    let mut spend = Spend::new();
    let mut c = Cover::new();
    for (a, b) in [(1.0, 2.0), (2.0, 3.0), (1.5, 2.5), (1.0, 3.0), (5.0, 6.0)] {
        assert!(c.hold(a, b, 0.0, &mut spend));
    }
    let free = c.free(0.0, 10.0, &mut spend).expect("free");
    assert_eq!(free, [(0.0, 1.0), (3.0, 5.0), (6.0, 10.0)]);
    assert_eq!(c.free(1.0, 3.0, &mut spend).expect("free"), []);
    assert_eq!(
        c.free(0.0, f64::INFINITY, &mut spend).expect("free").last(),
        Some(&(6.0, f64::INFINITY))
    );
}

#[test]
fn an_open_span_covers_to_infinity_and_minus_zero_is_zero() {
    // decision 506: `+inf` ends and a `-0` key
    let mut spend = Spend::new();
    let mut c = Cover::new();
    assert!(c.hold(2.0, f64::INFINITY, 0.0, &mut spend));
    assert_eq!(c.free(0.0, f64::INFINITY, &mut spend).expect("free"), [(0.0, 2.0)]);
    let mut d = Cover::new();
    assert!(d.hold(-0.0, 1.0, 0.0, &mut spend));
    assert_eq!(d.free(0.0, 2.0, &mut spend).expect("free"), [(1.0, 2.0)]);
    assert_eq!(d.free(-0.0, 2.0, &mut spend).expect("free"), [(1.0, 2.0)]);
}

#[test]
fn a_fade_spanning_the_whole_shot_adds_a_cross_and_covers_nothing() {
    // decision 507: `from == end` is not covered, yet `crossAt` has it
    let mut spend = Spend::new();
    let mut c = Cover::new();
    assert!(c.hold(1.0, 3.0, 2.0, &mut spend));
    assert_eq!(c.free(0.0, 10.0, &mut spend).expect("free"), [(0.0, 10.0)]);
    assert_eq!(c.cross_at(3.0), Some(2.0));
    assert_eq!(c.cross_at(2.0), None);
}

#[test]
fn of_two_fades_starting_at_one_point_the_last_in_the_file_wins() {
    // decision 507: swept backward, the first inserted stays
    let mut spend = Spend::new();
    let mut c = Cover::new();
    assert!(c.hold(3.0, 9.0, 1.0, &mut spend));
    assert!(c.hold(2.0, 9.0, 2.0, &mut spend));
    assert_eq!(c.cross_at(4.0), Some(1.0));
    assert_eq!(c.cross_at(-0.0), None);
}

#[test]
fn a_number_that_is_not_one_holds_and_covers_nothing() {
    // decision 505: a NaN bound is no span and no cross
    let mut spend = Spend::new();
    let mut c = Cover::new();
    assert!(c.hold(f64::NAN, 3.0, 1.0, &mut spend));
    assert!(c.hold(1.0, f64::NAN, 1.0, &mut spend));
    assert_eq!(c.free(0.0, 10.0, &mut spend).expect("free"), [(0.0, 10.0)]);
    assert_eq!(c.cross_at(f64::NAN), None);
}

#[test]
fn the_spans_a_query_touches_are_charged_and_a_spent_budget_answers_none() {
    // decision 522
    let mut spend = Spend::new();
    let mut c = Cover::new();
    for i in 0..100 {
        assert!(c.hold(f64::from(i) * 2.0, f64::from(i) * 2.0 + 1.0, 0.0, &mut spend));
    }
    let before = spend.spent();
    assert_eq!(c.free(0.0, 200.0, &mut spend).expect("free").len(), 100);
    assert!(spend.spent() - before >= 100);
    let mut small = Spend::with_limit(10);
    assert_eq!(c.free(0.0, 200.0, &mut small), None);
    assert!(small.capped());
}
