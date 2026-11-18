// apply.rs — turning the data at a path into new data with the fewest path edits, so comments and layout around them stay.
// A reader would get wrong what a changed list costs: its common head and tail stay, and only the middle is touched.

use super::layout::Res;
use super::value::{Value, same};
use crate::ast::PathSeg;

/// The edits applyData needs, which a document provides.
pub trait PathEditor {
    fn set_in(&mut self, path: &[PathSeg], value: &Value) -> Res<()>;
    fn delete_in(&mut self, path: &[PathSeg]) -> Res<bool>;
    fn insert_in(&mut self, path: &[PathSeg], index: usize, value: &Value) -> Res<()>;
    /// The source at a path, so a list emptied around a comment loses only its items.
    fn get_source(&self, path: &[PathSeg]) -> Option<String>;
}

fn at(path: &[PathSeg], seg: PathSeg) -> Vec<PathSeg> {
    let mut p = path.to_vec();
    p.push(seg);
    p
}

fn get<'a>(m: &'a [(String, Value)], key: &str) -> Option<&'a Value> {
    m.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

/// Turns `before` into `after` inside the document with the fewest path edits: changed leaves, added and removed keys, list splices.
///
/// # Errors
/// What the document's edits fail with.
pub fn apply_data(
    doc: &mut dyn PathEditor,
    before: &Value,
    after: &Value,
    path: &[PathSeg],
) -> Res<()> {
    if same(before, after) {
        return Ok(());
    }
    match (before, after) {
        (Value::Map(b), Value::Map(a)) => {
            for (key, _) in b {
                if get(a, key).is_none() {
                    doc.delete_in(&at(path, PathSeg::Key(key.clone())))?;
                }
            }
            for (key, value) in a {
                match get(b, key) {
                    Some(old) => apply_data(doc, old, value, &at(path, PathSeg::Key(key.clone())))?,
                    None => doc.set_in(&at(path, PathSeg::Key(key.clone())), value)?,
                }
            }
            Ok(())
        }
        (Value::List(b), Value::List(a)) if !path.is_empty() => apply_list(doc, b, a, path),
        _ => doc.set_in(path, after),
    }
}

/// A list edited at its middle: the common head and tail stay, an equal-length middle is edited item by item, else spliced around the items it keeps.
fn apply_list(
    doc: &mut dyn PathEditor,
    before: &[Value],
    after: &[Value],
    path: &[PathSeg],
) -> Res<()> {
    let mut head = 0;
    while head < before.len() && head < after.len() && same(&before[head], &after[head]) {
        head += 1;
    }
    let mut tail = 0;
    while tail < before.len() - head
        && tail < after.len() - head
        && same(
            &before[before.len() - 1 - tail],
            &after[after.len() - 1 - tail],
        )
    {
        tail += 1;
    }
    let old_mid = &before[head..before.len() - tail];
    let new_mid = &after[head..after.len() - tail];
    if old_mid.len() == new_mid.len() {
        for (i, v) in old_mid.iter().enumerate() {
            apply_data(doc, v, &new_mid[i], &at(path, PathSeg::Index(head + i)))?;
        }
        return Ok(());
    }
    if after.is_empty() && !commented(doc.get_source(path).as_deref()) {
        return doc.set_in(path, &Value::List(vec![]));
    }
    let anchors = common_items(old_mid, new_mid);
    // From the last stretch to the first, so an edit never shifts the index of one still to come.
    for s in (0..anchors.len()).rev() {
        let (a0, b0) = if s == 0 {
            (0, 0)
        } else {
            (anchors[s - 1].0 + 1, anchors[s - 1].1 + 1)
        };
        let (a1, b1) = anchors[s];
        apply_stretch(doc, &old_mid[a0..a1], &new_mid[b0..b1], path, head + a0)?;
    }
    Ok(())
}

/// One stretch between kept items: edited item by item when its length holds, else its old items deleted and the new inserted.
fn apply_stretch(
    doc: &mut dyn PathEditor,
    before: &[Value],
    after: &[Value],
    path: &[PathSeg],
    pos: usize,
) -> Res<()> {
    if before.len() == after.len() {
        for (i, v) in before.iter().enumerate() {
            apply_data(doc, v, &after[i], &at(path, PathSeg::Index(pos + i)))?;
        }
        return Ok(());
    }
    for i in (0..before.len()).rev() {
        doc.delete_in(&at(path, PathSeg::Index(pos + i)))?;
    }
    for (i, v) in after.iter().enumerate() {
        doc.insert_in(path, pos + i, v)?;
    }
    Ok(())
}

// Past this many comparisons a middle is spliced whole rather than matched.
const MATCH_BUDGET: usize = 1_000_000;

/// The index pairs of the longest run of items both lists hold in order, ending with the pair one past both ends.
fn common_items(before: &[Value], after: &[Value]) -> Vec<(usize, usize)> {
    let (n, m) = (before.len(), after.len());
    let end = (n, m);
    if n == 0 || m == 0 || n * m > MATCH_BUDGET {
        return vec![end];
    }
    let mut len = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            len[i][j] = if same(&before[i], &after[j]) {
                len[i + 1][j + 1] + 1
            } else {
                len[i + 1][j].max(len[i][j + 1])
            };
        }
    }
    let mut pairs = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if same(&before[i], &after[j]) {
            pairs.push((i, j));
            i += 1;
            j += 1;
        } else if len[i + 1][j] >= len[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    pairs.push(end);
    pairs
}

/// Whether source text holds a comment outside its strings: a list emptied item by item keeps it.
fn commented(source: Option<&str>) -> bool {
    let s = strip_strings(source.unwrap_or(""));
    s.contains("//") || s.contains("/*")
}

/// The text with each `"…"`, `'…'` and `` `…` `` removed, as `/"(?:[^"\\]|\\.)*"|…/g` matches them (`.` stops at a newline).
fn strip_strings(s: &str) -> String {
    let c: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < c.len() {
        let q = c[i];
        if q == '"' || q == '\'' || q == '`' {
            let mut j = i + 1;
            let mut closed = false;
            while j < c.len() {
                if c[j] == q {
                    closed = true;
                    break;
                }
                if c[j] == '\\' {
                    if j + 1 < c.len() && !is_line_end(c[j + 1]) {
                        j += 2;
                        continue;
                    }
                    break;
                }
                j += 1;
            }
            if closed {
                i = j + 1;
                continue;
            }
        }
        out.push(q);
        i += 1;
    }
    out
}

/// JS `.` does not match these.
fn is_line_end(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}
