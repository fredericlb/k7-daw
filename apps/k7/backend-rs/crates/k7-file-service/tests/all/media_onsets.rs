//! The onset corrections (2422): the TypeScript's set and order, in n log n. The reference below is `resolveOnsets` written as it is.

use std::time::{Duration, Instant};

use k7_file_service::media::onsets::{Edits, resolve_onsets, take_nears};

const BOUND: Duration = Duration::from_secs(if cfg!(target_env = "musl") { 60 } else { 15 });

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() <= 0.025
}

/// `resolveOnsets` as the TypeScript writes it: quadratic, and the order the arguments give.
fn reference(detected: &[f64], e: &Edits) -> Vec<f64> {
    let removed = e.removed.clone().unwrap_or_default();
    let added = e.added.clone().unwrap_or_default();
    let mut out: Vec<f64> = detected
        .iter()
        .copied()
        .filter(|d| !removed.iter().any(|r| near(*d, *r)))
        .collect();
    for a in added {
        if !out.iter().any(|k| near(*k, a)) {
            out.push(a);
        }
    }
    out.sort_by(|x, y| x.partial_cmp(y).unwrap());
    out
}

/// A tiny seeded generator, so a failure names its case.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % 1_000_000) as f64 / 1_000_000.0
    }
}

/// Half of the edits sit within 0.03 of a detected onset, where "near" decides.
fn part(rng: &mut Rng, detected: &[f64], span: f64, k: usize) -> Vec<f64> {
    (0..k)
        .map(|_| match detected.len() {
            0 => rng.next() * span,
            len if rng.next() < 0.5 => {
                detected[(rng.next() * len as f64) as usize] + (rng.next() - 0.5) * 0.06
            }
            _ => rng.next() * span,
        })
        .collect()
}

#[test]
fn equals_the_quadratic_reference_on_seeded_cases() {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    for case in 0..400 {
        let span = [2.0, 10.0, 60.0][case % 3];
        let n = (rng.next() * 60.0) as usize;
        let detected: Vec<f64> = (0..n)
            .map(|_| (rng.next() * span * 1e6).round() / 1e6)
            .collect();
        let (ka, kr) = ((rng.next() * 25.0) as usize, (rng.next() * 25.0) as usize);
        let edits = Edits {
            added: (case % 4 != 0).then(|| part(&mut rng, &detected, span, ka)),
            removed: (case % 5 != 0).then(|| part(&mut rng, &detected, span, kr)),
        };
        assert_eq!(
            resolve_onsets(&detected, &edits),
            reference(&detected, &edits),
            "case {case}: {detected:?} {edits:?}"
        );
    }
}

#[test]
fn keeps_a_detected_duplicate_and_drops_an_added_one_near_a_kept_one() {
    let e = Edits {
        added: Some(vec![3.0, 3.01, 7.0]),
        removed: None,
    };
    assert_eq!(
        resolve_onsets(&[5.0, 1.0, 5.0, 3.0, 1.0], &e),
        vec![1.0, 1.0, 3.0, 5.0, 5.0, 7.0]
    );
}

#[test]
fn an_edge_exactly_at_the_distance_is_near() {
    // 0.025 - 0 is exactly 0.025 in f64, so the edge is the comparison's own `<=`.
    let removed = Edits {
        added: None,
        removed: Some(vec![0.0]),
    };
    assert!(resolve_onsets(&[0.025], &removed).is_empty());
    assert_eq!(resolve_onsets(&[0.0251], &removed), vec![0.0251]);
    let added = Edits {
        added: Some(vec![0.025]),
        removed: None,
    };
    assert_eq!(resolve_onsets(&[0.0], &added), vec![0.0]);
}

#[test]
fn an_absent_list_is_not_an_empty_one_and_both_resolve_alike() {
    let none = Edits::default();
    let empty = Edits {
        added: Some(vec![]),
        removed: Some(vec![]),
    };
    assert_eq!(
        resolve_onsets(&[2.0, 1.0], &none),
        resolve_onsets(&[2.0, 1.0], &empty)
    );
    assert_ne!(none, empty);
}

#[test]
fn ten_thousand_and_forty_thousand_removals_answer_in_a_blink() {
    for n in [10_000usize, 40_000] {
        let detected: Vec<f64> = (0..n).map(|i| i as f64 * 0.1).collect();
        let removed: Vec<f64> = (0..n).map(|i| i as f64 * 0.1 + 0.01).collect();
        let added: Vec<f64> = (0..n).map(|i| i as f64 * 0.1 + 0.05).collect();
        let t = Instant::now();
        let out = resolve_onsets(
            &detected,
            &Edits {
                added: Some(added),
                removed: Some(removed),
            },
        );
        assert!(t.elapsed() < BOUND, "{n} removals took {:?}", t.elapsed());
        // Every detected onset is within 0.025 of its removal; each added one is 0.05 from a removed (dropped) one only.
        assert_eq!(out.len(), n, "{n}: only the added ones remain");
    }
}

#[test]
fn the_corrections_compare_a_logarithmic_number_of_onsets_not_a_linear_one() {
    let mut nears = Vec::new();
    for n in [2_000usize, 8_000] {
        let detected: Vec<f64> = (0..n).map(|i| i as f64 * 0.1).collect();
        let removed: Vec<f64> = (0..n).map(|i| i as f64 * 0.1 + 0.01).collect();
        let added: Vec<f64> = (0..n).map(|i| i as f64 * 0.1 + 0.05).collect();
        take_nears();
        let out = resolve_onsets(
            &detected,
            &Edits {
                added: Some(added),
                removed: Some(removed),
            },
        );
        assert_eq!(out.len(), n);
        nears.push(take_nears());
    }
    // A set lookup compares at most two neighbours per onset: 4n onsets cost four times n's, and a scan sixteen times.
    assert!(nears[0] > 0, "the comparisons were counted");
    assert!(
        nears[1] < 5 * nears[0],
        "{} comparisons against {} at a quarter of the onsets",
        nears[1],
        nears[0]
    );
}
