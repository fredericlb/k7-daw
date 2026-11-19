//! diff3.rs — node-diff3 3.2.1's `diff3Merge` as live-sync's `mergeTexts` calls it, ported step for step: Hunt and
//! McIlroy's LCS, `diffIndices`, `diff3MergeRegions`, the false-conflict rule. Held by the merge tests below and the settle vectors.
//! What a reader gets wrong: this is not "a" diff3. Another LCS finds other matches among repeated lines, so other
//! conflict boundaries; only the candidate search is faster (a gallop where node scans), its answer the same.
//! Choices and the work budget: apps/k7/backend-rs/docs/parity.md, "The text merge".

use std::collections::HashMap;

/// Past this many steps of both LCS a merge gives up (about 1.3 s): a text of equal lines is cubic in node.
pub const BUDGET: u64 = 1 << 29;

/// Past this many candidates kept for the chain, too: 48 MB.
const MAX_CANDIDATES: usize = 1 << 22;

/// The merge ran past `BUDGET` steps, or `MAX_CANDIDATES`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TooLong;

/// One block of `diff3Merge`'s answer, its lines borrowed from the inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block<'t> {
    Ok(Vec<&'t str>),
    Conflict {
        a: Vec<&'t str>,
        a_index: i64,
        o: Vec<&'t str>,
        o_index: i64,
        b: Vec<&'t str>,
        b_index: i64,
    },
}

/// `text.split(/(?<=\n)/)`: each line with its newline, and `[""]` for an empty text as JavaScript gives.
pub fn lines(text: &str) -> Vec<&str> {
    if text.is_empty() {
        return vec![""];
    }
    text.split_inclusive('\n').collect()
}

/// `mergeTexts(base, mine, theirs)`: one text, or None when any region conflicts.
pub fn merge_texts(base: &str, mine: &str, theirs: &str) -> Result<Option<String>, TooLong> {
    let blocks = diff3_merge(&lines(mine), &lines(base), &lines(theirs), true)?;
    let mut out = String::new();
    for b in blocks {
        match b {
            Block::Ok(l) => l.iter().for_each(|s| out.push_str(s)),
            Block::Conflict { .. } => return Ok(None),
        }
    }
    Ok(Some(out))
}

/// One k-candidate of the LCS: a match and the candidate it extends (`NONE` ends the chain).
#[derive(Clone, Copy)]
struct Cand {
    b1: i32,
    b2: i32,
    chain: u32,
}

const NONE: u32 = u32::MAX;

/// `LCS(buffer1, buffer2)` over interned lines: the chain from the last candidate, NULLRESULT last.
fn lcs(buffer1: &[u32], buffer2: &[u32], steps: &mut u64) -> Result<Vec<(i64, i64)>, TooLong> {
    let mut classes: HashMap<u32, Vec<i64>> = HashMap::new();
    for (j, item) in buffer2.iter().enumerate() {
        classes.entry(*item).or_default().push(j as i64);
    }
    let mut arena = vec![Cand {
        b1: -1,
        b2: -1,
        chain: NONE,
    }];
    // Each slot: the candidate's id in `arena` and its buffer2 index, increasing along the slots.
    let mut candidates: Vec<(u32, i64)> = vec![(0, -1)];
    let empty = Vec::new();
    for (i, item) in buffer1.iter().enumerate() {
        let js = classes.get(item).unwrap_or(&empty);
        let len = candidates.len();
        let mut r = 0usize;
        let mut c = candidates[0];
        let mut q = 0usize;
        let mut p = 0usize;
        while p < js.len() {
            *steps = steps.checked_sub(1).ok_or(TooLong)?;
            let j = js[p];
            // node's scan from r finds nothing for a j at or below the r-th candidate: skip them all at once.
            let tr = candidates[r].1;
            if tr >= j {
                p += js[p..].partition_point(|&x| x <= tr);
                continue;
            }
            p += 1;
            let s = gallop(&candidates[..len], q.max(r), j, steps);
            if s + 1 < len && candidates[s + 1].1 == j {
                q = s;
                continue;
            }
            let id = arena.len() as u32;
            arena.push(Cand {
                b1: i as i32,
                b2: j as i32,
                chain: candidates[s].0,
            });
            if arena.len() > MAX_CANDIDATES {
                return Err(TooLong);
            }
            // Slots below the new r are never read again in this pass, so writing now reads what node reads.
            candidates[r] = c;
            r = s + 1;
            q = r;
            c = (id, j);
            if r == len {
                break;
            }
        }
        if r == candidates.len() {
            candidates.push(c);
        } else {
            candidates[r] = c;
        }
    }
    let mut chain = Vec::new();
    let mut at = candidates[candidates.len() - 1].0;
    while at != NONE {
        let k = arena[at as usize];
        chain.push((i64::from(k.b1), i64::from(k.b2)));
        at = k.chain;
    }
    Ok(chain)
}

/// The largest `s` from `lo` on whose buffer2 index is below `j`, given that `lo`'s is.
fn gallop(slots: &[(u32, i64)], lo: usize, j: i64, steps: &mut u64) -> usize {
    let len = slots.len();
    let mut lo = lo;
    let mut step = 1usize;
    while lo + step < len && slots[lo + step].1 < j {
        *steps = steps.saturating_sub(1);
        lo += step;
        step *= 2;
    }
    let mut hi = (lo + step).min(len);
    while hi - lo > 1 {
        *steps = steps.saturating_sub(1);
        let mid = lo + (hi - lo) / 2;
        if slots[mid].1 < j {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo
}

/// `diffIndices(buffer1, buffer2)`: each mismatch as `(start1, length1, start2, length2)`, in order.
fn diff_indices(
    buffer1: &[u32],
    buffer2: &[u32],
    steps: &mut u64,
) -> Result<Vec<[i64; 4]>, TooLong> {
    let mut tail1 = buffer1.len() as i64;
    let mut tail2 = buffer2.len() as i64;
    let mut out = Vec::new();
    for (b1, b2) in lcs(buffer1, buffer2, steps)? {
        let m1 = tail1 - b1 - 1;
        let m2 = tail2 - b2 - 1;
        tail1 = b1;
        tail2 = b2;
        if m1 != 0 || m2 != 0 {
            out.push([tail1 + 1, m1, tail2 + 1, m2]);
        }
    }
    out.reverse();
    Ok(out)
}

/// `Array.prototype.slice(start, end)`, negative indices from the end, clamped.
fn slice<'t>(v: &[&'t str], start: i64, end: i64) -> Vec<&'t str> {
    let n = v.len() as i64;
    let at = |k: i64| if k < 0 { (n + k).max(0) } else { k.min(n) } as usize;
    let (s, e) = (at(start), at(end));
    if s < e { v[s..e].to_vec() } else { Vec::new() }
}

/// A hunk of `diff3MergeRegions`: where `a` or `b` differs from `o`.
#[derive(Clone, Copy)]
struct Hunk {
    b_side: bool,
    o_start: i64,
    o_length: i64,
    ab_start: i64,
    ab_length: i64,
}

/// `diff3Merge(a, o, b, { excludeFalseConflicts })` over lines.
pub fn diff3_merge<'t>(
    a: &[&'t str],
    o: &[&'t str],
    b: &[&'t str],
    exclude_false_conflicts: bool,
) -> Result<Vec<Block<'t>>, TooLong> {
    diff3_merge_within(a, o, b, exclude_false_conflicts, BUDGET)
}

/// `diff3_merge` given up after `budget` steps of the LCS.
fn diff3_merge_within<'t>(
    a: &[&'t str],
    o: &[&'t str],
    b: &[&'t str],
    exclude_false_conflicts: bool,
    budget: u64,
) -> Result<Vec<Block<'t>>, TooLong> {
    let mut ids: HashMap<&str, u32> = HashMap::new();
    let mut intern = |v: &[&'t str]| -> Vec<u32> {
        v.iter()
            .map(|l| {
                let n = ids.len() as u32;
                *ids.entry(*l).or_insert(n)
            })
            .collect()
    };
    let (ia, io, ib) = (intern(a), intern(o), intern(b));
    let mut steps = budget;
    let mut hunks: Vec<Hunk> = Vec::new();
    for (side, other) in [(false, &ia), (true, &ib)] {
        for [os, ol, abs, abl] in diff_indices(&io, other, &mut steps)? {
            hunks.push(Hunk {
                b_side: side,
                o_start: os,
                o_length: ol,
                ab_start: abs,
                ab_length: abl,
            });
        }
    }
    // A stable sort, as V8's: a's hunks before b's at one offset.
    hunks.sort_by_key(|h| h.o_start);

    let mut out = Vec::new();
    let mut ok: Vec<&'t str> = Vec::new();
    let flush = |ok: &mut Vec<&'t str>, out: &mut Vec<Block<'t>>| {
        if !ok.is_empty() {
            out.push(Block::Ok(std::mem::take(ok)));
        }
    };
    let mut curr = 0i64;
    let mut k = 0usize;
    while k < hunks.len() {
        let hunk = hunks[k];
        k += 1;
        let region_start = hunk.o_start;
        let mut region_end = hunk.o_start + hunk.o_length;
        let first = k - 1;
        if region_start > curr {
            ok.extend(slice(o, curr, region_start));
        }
        while k < hunks.len() && hunks[k].o_start <= region_end {
            region_end = region_end.max(hunks[k].o_start + hunks[k].o_length);
            k += 1;
        }
        if k - first == 1 {
            if hunk.ab_length > 0 {
                let buffer = if hunk.b_side { b } else { a };
                ok.extend(slice(buffer, hunk.ab_start, hunk.ab_start + hunk.ab_length));
            }
        } else {
            let mut bounds = [
                [a.len() as i64, -1, o.len() as i64, -1],
                [b.len() as i64, -1, o.len() as i64, -1],
            ];
            for h in &hunks[first..k] {
                let x = &mut bounds[h.b_side as usize];
                x[0] = x[0].min(h.ab_start);
                x[1] = x[1].max(h.ab_start + h.ab_length);
                x[2] = x[2].min(h.o_start);
                x[3] = x[3].max(h.o_start + h.o_length);
            }
            let [ba, bb] = bounds;
            let a_start = ba[0] + (region_start - ba[2]);
            let a_end = ba[1] + (region_end - ba[3]);
            let b_start = bb[0] + (region_start - bb[2]);
            let b_end = bb[1] + (region_end - bb[3]);
            let a_content = slice(a, a_start, a_end);
            let b_content = slice(b, b_start, b_end);
            if exclude_false_conflicts && a_content == b_content {
                ok.extend(a_content);
            } else {
                flush(&mut ok, &mut out);
                out.push(Block::Conflict {
                    a: a_content,
                    a_index: a_start,
                    o: slice(o, region_start, region_end),
                    o_index: region_start,
                    b: b_content,
                    b_index: b_start,
                });
            }
        }
        curr = region_end;
    }
    if (o.len() as i64) > curr {
        ok.extend(slice(o, curr, o.len() as i64));
    }
    flush(&mut ok, &mut out);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;



    #[test]
    fn merges_lines_apart_and_refuses_one_line_changed_twice() {
        let base = "# Todo\n\n- bass\n- drums\n- vocals\n- mix\n";
        let a = base.replace("- bass", "- bass (done)");
        let b = base.replace("- mix", "- mix and master");
        let both = base
            .replace("- bass", "- bass (done)")
            .replace("- mix", "- mix and master");
        assert_eq!(merge_texts(base, &a, &b), Ok(Some(both)));
        assert_eq!(
            merge_texts(base, &a, &base.replace("- bass", "- bass!")),
            Ok(None)
        );
        assert_eq!(merge_texts(base, &a, &a), Ok(Some(a.clone())));
        assert_eq!(
            merge_texts("a\nb\nc", "A\nb\nc", "a\nb\nC"),
            Ok(Some("A\nb\nC".into()))
        );
        assert_eq!(lines(""), vec![""]);
    }

    #[test]
    fn a_degenerate_text_runs_out_of_budget_instead_of_hours() {
        let same = "x\n".repeat(5_000);
        let (mine, theirs) = (format!("y\n{same}"), format!("{same}z\n"));
        let (a, o, b) = (lines(&mine), lines(&same), lines(&theirs));
        assert_eq!(diff3_merge_within(&a, &o, &b, true, 1 << 20), Err(TooLong));
        assert!(diff3_merge_within(&a, &o, &b, true, 1 << 28).is_ok());
    }
}
