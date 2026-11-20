//! onsets.rs — the onsets a clip has: what the engine detected less what was removed, plus what was added (2422).
//! What a reader gets wrong: the TypeScript's set and order are kept exactly, in n log n; "near" is `|a - b| <= 0.025` computed in f64.

use std::cell::Cell;
use std::cmp::Ordering;
use std::collections::BTreeSet;

/// How close a removal must be to a detected onset to mean "that one".
const NEAR: f64 = 0.025;

/// The corrections a clip file stores, in source seconds; a list that is absent stays absent (`{}` is not `{added: []}`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Edits {
    pub added: Option<Vec<f64>>,
    pub removed: Option<Vec<f64>>,
}

thread_local! {
    // The `near` comparisons this thread has made, so a test counts them instead of timing them (2422).
    static NEARS: Cell<usize> = const { Cell::new(0) };
}

/// The `near` comparisons made by this thread since the last call; the counter starts again at zero.
pub fn take_nears() -> usize {
    NEARS.with(|c| c.replace(0))
}

fn near(a: f64, b: f64) -> bool {
    NEARS.with(|c| c.set(c.get() + 1));
    (a - b).abs() <= NEAR
}

/// A total order over f64 for the sets below: NaN last, then the numbers by value.
#[derive(Clone, Copy, Debug)]
struct Key(f64);

impl PartialEq for Key {
    fn eq(&self, o: &Key) -> bool {
        self.cmp(o) == Ordering::Equal
    }
}

impl Eq for Key {}

impl PartialOrd for Key {
    fn partial_cmp(&self, o: &Key) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

impl Ord for Key {
    fn cmp(&self, o: &Key) -> Ordering {
        match (self.0.is_nan(), o.0.is_nan()) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            (false, false) => self.0.partial_cmp(&o.0).unwrap_or(Ordering::Equal),
        }
    }
}

/// Whether some member of `set` is near `x`: the nearest below and above are the only ones that can be, since `|x - k|` grows away from `x`.
fn any_near(set: &BTreeSet<Key>, x: f64) -> bool {
    if x.is_nan() {
        return false;
    }
    let below = set.range(..=Key(x)).next_back();
    let above = set.range(Key(x)..).next();
    below.into_iter().chain(above).any(|k| near(k.0, x))
}

/// `resolveOnsets`: the onsets to draw, near-duplicates collapsed, sorted.
pub fn resolve_onsets(detected: &[f64], edits: &Edits) -> Vec<f64> {
    let removed: BTreeSet<Key> = edits.removed.iter().flatten().map(|&r| Key(r)).collect();
    let mut out: Vec<f64> = detected
        .iter()
        .copied()
        .filter(|&d| !any_near(&removed, d))
        .collect();
    let mut seen: BTreeSet<Key> = out.iter().map(|&k| Key(k)).collect();
    for &a in edits.added.iter().flatten() {
        if !any_near(&seen, a) {
            out.push(a);
            seen.insert(Key(a));
        }
    }
    // JavaScript's sort treats equal numbers as equal and keeps their order; a NaN goes last here.
    out.sort_by_key(|x| Key(*x));
    out
}
