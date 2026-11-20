// onsets.rs — the onsets a clip actually has: what the engine detected, plus the corrections made by hand.
// A reader would get wrong what is stored: only the CORRECTIONS, in seconds into the source; the detected set is derived.

/// A correction block as `clips/*.k7x` stores it, both lists in seconds.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OnsetEdits {
    pub added: Option<Vec<f64>>,
    pub removed: Option<Vec<f64>>,
}

/// How close a removal must be to a detected onset to mean "that one".
const ONSET_MATCH_S: f64 = 0.025;

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() <= ONSET_MATCH_S
}

fn list(x: &Option<Vec<f64>>) -> &[f64] {
    x.as_deref().unwrap_or(&[])
}

/// The onsets to draw: `detected` less what was removed, plus what was added, near-duplicates collapsed.
#[must_use]
pub fn resolve_onsets(detected: &[f64], edits: Option<&OnsetEdits>) -> Vec<f64> {
    let (removed, added) = edits.map_or((&[][..], &[][..]), |e| (list(&e.removed), list(&e.added)));
    let mut out: Vec<f64> = detected
        .iter()
        .copied()
        .filter(|d| !removed.iter().any(|r| near(*d, *r)))
        .collect();
    for a in added {
        if !out.iter().any(|k| near(*k, *a)) {
            out.push(*a);
        }
    }
    out.sort_by(f64::total_cmp);
    out
}

/// Sorted, deduplicated, and with empty lists dropped so an untouched clip writes no `onsets:` block at all.
fn tidy(added: Option<Vec<f64>>, removed: Option<Vec<f64>>) -> OnsetEdits {
    let clean = |xs: &Option<Vec<f64>>| {
        let mut v: Vec<f64> = list(xs).to_vec();
        v.sort_by(f64::total_cmp);
        v.dedup();
        (!v.is_empty()).then_some(v)
    };
    OnsetEdits {
        added: clean(&added),
        removed: clean(&removed),
    }
}

/// The edits after removing the onset at `at_sec`; removing an ADDED one undoes the addition.
#[must_use]
pub fn remove_onset(edits: Option<&OnsetEdits>, detected: &[f64], at_sec: f64) -> OnsetEdits {
    let old_added = edits.and_then(|e| e.added.clone()).unwrap_or_default();
    let added: Vec<f64> = old_added.iter().copied().filter(|a| !near(*a, at_sec)).collect();
    let removed = edits.and_then(|e| e.removed.clone());
    if added.len() != old_added.len() {
        return tidy(Some(added), removed);
    }
    // A detected one is named by the DETECTED time, so it keeps matching after a re-detection.
    let hit = detected.iter().copied().find(|d| near(*d, at_sec)).unwrap_or(at_sec);
    let mut rem = removed.unwrap_or_default();
    rem.push(hit);
    tidy(Some(added), Some(rem))
}

/// The edits after adding an onset at `at_sec`, dropping a removal it cancels rather than stacking on it.
#[must_use]
pub fn add_onset(edits: Option<&OnsetEdits>, at_sec: f64) -> OnsetEdits {
    let old_removed = edits.and_then(|e| e.removed.clone()).unwrap_or_default();
    let removed: Vec<f64> = old_removed.iter().copied().filter(|r| !near(*r, at_sec)).collect();
    let added = edits.and_then(|e| e.added.clone());
    if removed.len() != old_removed.len() {
        return tidy(added, Some(removed));
    }
    let mut add = added.unwrap_or_default();
    add.push(at_sec);
    tidy(Some(add), Some(removed))
}

/// Moving one is removing it and adding it back elsewhere.
#[must_use]
pub fn move_onset(edits: Option<&OnsetEdits>, detected: &[f64], from_sec: f64, to_sec: f64) -> OnsetEdits {
    let removed = remove_onset(edits, detected, from_sec);
    add_onset(Some(&removed), to_sec)
}
