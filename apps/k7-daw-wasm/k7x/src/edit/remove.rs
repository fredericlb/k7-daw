// remove.rs — deleting an entry or a list item: its whole line when it had one to itself, else just it and its separator.
// A reader would get wrong what a chain's last call leaves: `{}`, since a value cannot be written as nothing.

use super::layout::{Host, Res, comma_after, head_blank, line_start, tail_end};
use super::print::{Ctx, from_js, print};
use super::tree::{EntryTree, Tree, index_of};
use super::value::{Value, js_trim};
use crate::ast::{ListForm, MapForm, PathSeg};

/// Removes `parent`'s entry or item at `seg`; false when there was nothing there.
pub fn remove_at(h: &mut dyn Host, parent: &Tree, seg: &PathSeg) -> Res<bool> {
    if parent.child_at(seg).is_none() {
        return Ok(false);
    }
    match parent {
        Tree::List {
            items,
            form,
            start,
            end,
        } => {
            let Some(i) = index_of(seg) else {
                return Ok(false);
            };
            if *form == ListForm::Args && items.len() <= 2 {
                let rest: Vec<Value> = items
                    .iter()
                    .enumerate()
                    .filter(|(j, _)| *j != i)
                    .map(|(_, n)| n.to_value())
                    .collect();
                let text = print(
                    &from_js(&Value::List(rest), None, &mut Vec::new()),
                    0,
                    Ctx::Entry,
                    0,
                )?
                .text;
                h.splice(*start, *end, &text)?;
            } else {
                let spans: Vec<(usize, usize)> =
                    items.iter().map(|n| (n.start(), n.end())).collect();
                emptied(h, parent, |h| remove_listed(h, &spans, i))?;
            }
            Ok(true)
        }
        Tree::Map(m) => {
            let PathSeg::Key(k) = seg else {
                return Ok(false);
            };
            let Some(i) = m.entries.iter().position(|e| e.key == *k) else {
                return Ok(false);
            };
            if m.form == MapForm::Object {
                let spans: Vec<(usize, usize)> =
                    m.entries.iter().map(|e| (e.start, e.end)).collect();
                emptied(h, parent, |h| remove_listed(h, &spans, i))?;
            } else if m.form == MapForm::Chain && m.entries.len() == 1 {
                h.splice(m.start, m.end, "{}")?;
            } else {
                let (a, b) = m
                    .groups
                    .iter()
                    .copied()
                    .find(|(s, e)| i >= *s && i < *e)
                    .unwrap_or((0, m.entries.len()));
                remove_chained(h, &m.entries[a..b], i - a)?;
            }
            Ok(true)
        }
        Tree::Scalar { .. } => Ok(false),
    }
}

/// Runs a removal from a list or an object, then writes it `[]` or `{}` when that left only blanks between its brackets.
fn emptied(
    h: &mut dyn Host,
    node: &Tree,
    remove: impl FnOnce(&mut dyn Host) -> Res<()>,
) -> Res<()> {
    let size = match node {
        Tree::List { items, .. } => items.len(),
        Tree::Map(m) => m.entries.len(),
        Tree::Scalar { .. } => 0,
    };
    let before = h.text().len();
    remove(h)?;
    if size != 1 {
        return Ok(());
    }
    let t = h.text();
    let end = (node.end() + t.len()) - before;
    if !js_trim(&t[node.start() + 1..end - 1]).is_empty() {
        return Ok(());
    }
    h.splice(
        node.start(),
        end,
        if matches!(node, Tree::List { .. }) {
            "[]"
        } else {
            "{}"
        },
    )
}

/// After a deletion at `path`, removes each ancestor map it left empty, up to a list item or the root.
pub fn prune(h: &mut dyn Host, path: &[PathSeg]) -> Res<()> {
    for n in (1..path.len()).rev() {
        let root = h.root()?;
        let node = root.node_at(&path[..n]);
        let parent = root.node_at(&path[..n - 1]);
        match (node, parent) {
            (Some(Tree::Map(m)), Some(p @ Tree::Map(_))) if m.entries.is_empty() => {
                remove_at(h, p, &path[n - 1])?;
            }
            _ => return Ok(()),
        }
    }
    Ok(())
}

/// Removes one item or object entry with its comma; a whole line when it had one to itself.
fn remove_listed(h: &mut dyn Host, items: &[(usize, usize)], i: usize) -> Res<()> {
    let t = h.text();
    let (es, ee) = items[i];
    let tail = tail_end(&t, ee, true);
    if let (true, Some(tail)) = (head_blank(&t, es), tail) {
        let comma_here = t[ee..].trim_start_matches([' ', '\t']).starts_with(',');
        let next = if comma_here {
            None
        } else {
            comma_after(&t, ee)
        };
        // Written comma-first, the separator is on a later line: it goes too, and what sits before it stays.
        let keep = next.map_or("", |n| t.get(tail + 1..n).unwrap_or(""));
        let end = next.map_or_else(|| (tail + 1).min(t.len()), |n| past_comma(&t, n));
        return h.splice(line_start(&t, es), end, keep);
    }
    let one_line = |a: usize, b: usize| !t.get(a..b).unwrap_or("").contains('\n');
    if i + 1 < items.len() && one_line(ee, items[i + 1].0) {
        return h.splice(es, items[i + 1].0, "");
    }
    if i > 0 && one_line(items[i - 1].1, es) {
        return h.splice(items[i - 1].1, ee, "");
    }
    // A line break on both sides may carry a comment: only the item and its comma go, the trivia between them kept.
    let Some(comma) = comma_after(&t, ee) else {
        return h.splice(es, ee, "");
    };
    let between = &t[ee..comma];
    let kept = if between.starts_with([' ', '\t']) {
        format!(" {}", between.trim_start_matches([' ', '\t']))
    } else {
        between.to_owned()
    };
    h.splice(es, past_comma(&t, comma), &kept)
}

/// Just past the comma at `at` and the blanks that follow it on its line.
fn past_comma(t: &str, at: usize) -> usize {
    at + 1
        + t[at + 1..]
            .bytes()
            .take_while(|&b| b == b' ' || b == b'\t')
            .count()
}

/// Removes one call of a chain: its line when it had one, the call and its dot otherwise.
fn remove_chained(h: &mut dyn Host, group: &[EntryTree], i: usize) -> Res<()> {
    let t = h.text();
    let e = &group[i];
    let tail = tail_end(&t, e.end, false);
    if group.len() == 1 {
        if let (true, Some(tail)) = (head_blank(&t, e.start), tail) {
            return h.splice(line_start(&t, e.start), (tail + 1).min(t.len()), "");
        }
        return h.splice(e.start, e.end, "");
    }
    if i == 0 {
        return h.splice(e.start, group[1].key_start, "");
    }
    if let (Some(dot), Some(tail)) = (e.dot, tail)
        && head_blank(&t, dot)
    {
        return h.splice(line_start(&t, dot), (tail + 1).min(t.len()), "");
    }
    h.splice(e.start, e.end, "")
}
