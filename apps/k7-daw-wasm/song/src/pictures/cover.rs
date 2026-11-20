// pictures/cover.rs — the spans later shots hold and the crossfades they start, swept from the last shot to the first.
// A reader would get wrong that a shot asks BEFORE it adds: its own cover only matters to the shots before it (506).

use super::Spend;
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::ops::Bound::{Excluded, Unbounded};

/// A span's start ordered as `total_cmp` does, with `-0` taken as `0`.
#[derive(Clone, Copy, Debug)]
struct Key(f64);

fn key(v: f64) -> Key {
    Key(if v == 0.0 { 0.0 } else { v })
}

impl PartialEq for Key {
    fn eq(&self, other: &Key) -> bool {
        self.0.total_cmp(&other.0) == Ordering::Equal
    }
}

impl Eq for Key {}

impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Key) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Key {
    fn cmp(&self, other: &Key) -> Ordering {
        self.0.total_cmp(&other.0)
    }
}

/// A crossfade's key: the bits of a number, `-0` taken as `0`.
fn bits(v: f64) -> u64 {
    if v == 0.0 { 0 } else { v.to_bits() }
}

/// What the shots after the one asking hold: disjoint half-open spans, and `from -> fadeIn` for each fade.
#[derive(Default)]
pub struct Cover {
    spans: BTreeMap<Key, f64>,
    cross: HashMap<u64, f64>,
}

impl Cover {
    pub fn new() -> Cover {
        Cover::default()
    }

    /// The pieces of `[a, b)` no held span covers, ascending; None once the spans touched pass the budget.
    pub fn free(&self, a: f64, b: f64, spend: &mut Spend) -> Option<Vec<(f64, f64)>> {
        let mut out = Vec::new();
        if a.partial_cmp(&b) != Some(Ordering::Less) {
            return Some(out);
        }
        let mut cursor = a;
        if let Some((_, &end)) = self.spans.range(..=key(a)).next_back() {
            if !spend.charge(1) {
                return None;
            }
            if end > cursor {
                cursor = end;
            }
        }
        for (start, &end) in self.spans.range((Excluded(key(a)), Unbounded)) {
            if cursor >= b || start.0 >= b {
                break;
            }
            if !spend.charge(1) {
                return None;
            }
            if start.0 > cursor {
                out.push((cursor, start.0));
            }
            if end > cursor {
                cursor = end;
            }
        }
        if cursor < b {
            out.push((cursor, b));
        }
        Some(out)
    }

    /// The fade-in the last later shot starting a fade at `b` gives, which the earlier sound leaves over.
    pub fn cross_at(&self, b: f64) -> Option<f64> {
        self.cross.get(&bits(b)).copied()
    }

    /// Adds a shot: its fade is a crossfade from `start + fade_in` (the last in the file wins), and what follows the fade is held (507).
    pub fn hold(&mut self, start: f64, end: f64, fade_in: f64, spend: &mut Spend) -> bool {
        if start.is_nan() || end.is_nan() {
            return true;
        }
        let from = start + fade_in;
        if fade_in > 0.0 && !from.is_nan() {
            self.cross.entry(bits(from)).or_insert(fade_in);
        }
        if from < end {
            return self.add(from, end, spend);
        }
        true
    }

    /// Unions `[x, y)` in, joining the spans it touches.
    fn add(&mut self, x: f64, y: f64, spend: &mut Spend) -> bool {
        let (mut lo, mut hi) = (x, y);
        if let Some((&k, &end)) = self.spans.range(..=key(x)).next_back()
            && end >= x
        {
            if !spend.charge(1) {
                return false;
            }
            lo = k.0;
            hi = hi.max(end);
            self.spans.remove(&k);
        }
        while let Some((&k, &end)) = self.spans.range((Excluded(key(lo)), Unbounded)).next() {
            if k.0 > hi {
                break;
            }
            if !spend.charge(1) {
                return false;
            }
            hi = hi.max(end);
            self.spans.remove(&k);
        }
        self.spans.insert(key(lo), hi);
        true
    }
}
