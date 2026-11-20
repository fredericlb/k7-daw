// format_chain.rs — a chain written as a file's k7x text, keeping the old text's header and every unchanged item's source and comments.
// A reader would get wrong that an item is found by its `id`: a graph edit moves nodes rather than retyping them.

use crate::chain_data::{Chain, Item, Step, id_of, is_lane_item};
use crate::chain_item_edit::chain_item_paths;
use crate::error::Res;
use k7x::edit::{Document, PKind, PNode, PathSeg, Tree, Value, from_js, header_text, js_trim, print_file, same};
use std::collections::HashMap;

/// An item as the old file wrote it: its data, its source, the `//` lines above it and the column it started at.
struct Kept {
    value: Value,
    source: String,
    lead: Vec<String>,
    col: usize,
}

/// Every item of the previous text by id, with what a rewrite puts back when the item is unchanged.
fn kept_by_id(old: &Document) -> Res<HashMap<String, Kept>> {
    let mut out = HashMap::new();
    if old.to_js().is_err() || old.text().trim().is_empty() {
        return Ok(out);
    }
    for (id, path) in chain_item_paths(old)? {
        let Some((value, source)) = old.get_in(&path)?.zip(old.get_source(&path)?) else {
            continue;
        };
        let (parent, last) = path.split_at(path.len() - 1);
        let k = match last {
            [PathSeg::Index(k)] => *k,
            _ => continue,
        };
        let start = old.node_at(&path, |n| n.map(Tree::start))?.unwrap_or(0);
        let from = old.node_at(parent, |n| match n {
            Some(Tree::List { items, form, start, .. }) => Some(if k > 0 {
                items.get(k - 1).map_or(*start, Tree::end)
            } else {
                *start + usize::from(*form == k7x::ListForm::Brackets)
            }),
            _ => None,
        })?;
        let Some(from) = from else { continue };
        // The first line of the gap is the previous token's own, its trailing comment included: only whole lines are the item's.
        let lead = old.text()[from..start]
            .split('\n')
            .skip(1)
            .map(js_trim)
            .filter(|l| l.starts_with("//"))
            .map(|l| l[2..].to_owned())
            .collect();
        let col = old.position_of(&path)?.map_or(1, |p| p.1);
        out.insert(
            id,
            Kept {
                value,
                source,
                lead,
                col,
            },
        );
    }
    Ok(out)
}

/// The print node at a path of a `from_js` tree.
fn pnode_at<'a>(root: &'a mut PNode, path: &[PathSeg]) -> Option<&'a mut PNode> {
    let mut n = root;
    for seg in path {
        n = match (&mut n.kind, seg) {
            (PKind::Map(es), PathSeg::Key(k)) => &mut es.iter_mut().find(|e| e.key == *k)?.value,
            (PKind::List(items), PathSeg::Index(i)) => items.get_mut(*i)?,
            _ => return None,
        };
    }
    Some(n)
}

/// A source moved from one column to another, its continuation lines shifted by as much.
fn reindent(source: &str, from: usize, to: usize) -> String {
    if from == to || !source.contains('\n') {
        return source.to_owned();
    }
    let mut lines = source.split('\n');
    let first = lines.next().unwrap_or("");
    let mut out = vec![first.to_owned()];
    for l in lines {
        out.push(if to > from {
            if l.is_empty() {
                String::new()
            } else {
                format!("{}{l}", " ".repeat(to - from))
            }
        } else {
            let cut = l.bytes().take(from - to).take_while(|b| *b == b' ').count();
            l[cut..].to_owned()
        });
    }
    out.join("\n")
}

fn key(s: &str) -> PathSeg {
    PathSeg::Key(s.to_owned())
}

/// Every item of a chain's data with its path in the file: the chain's own steps, then each subchain's.
fn item_slots(chain: &Chain) -> Vec<(&Item, Vec<PathSeg>)> {
    let mut out = vec![];
    let mut lists: Vec<(Vec<PathSeg>, &[Step])> = vec![(vec![key("steps")], &chain.steps)];
    for (j, sc) in chain.subchains.iter().enumerate() {
        lists.push((vec![key("subchains"), PathSeg::Index(j), key("steps")], &sc.steps));
    }
    for (at, steps) in lists {
        for (s, step) in steps.iter().enumerate() {
            for (k, item) in step.items().iter().enumerate() {
                let mut path = at.clone();
                path.push(PathSeg::Index(s));
                if matches!(step, Step::Many(_)) {
                    path.push(PathSeg::Index(k));
                }
                out.push((item, path));
            }
        }
    }
    out
}

/// One line for what reads as one: the ports, a lane item, a subchain call, one link of an end step.
fn inline_at(path: &[PathSeg], value: &Value) -> bool {
    if path.len() == 1 {
        return path[0] == key("ports");
    }
    if !path.contains(&key("steps")) {
        return false;
    }
    let is_call = matches!(value, Value::Map(_)) && value.get("sub").is_some() && value.get("id").is_none();
    path.len() >= 2 && path[path.len() - 2] == key("to") || is_lane_item(value) || is_call
}

/// Write `chain` as a file's k7x text, keeping `prev_text`'s header and every unchanged item's source and comments.
///
/// # Errors
/// A value that does not print.
pub fn format_chain(chain: &Chain, prev_text: &str) -> Res<String> {
    let old = Document::new(prev_text);
    let kept = kept_by_id(&old)?;
    let mut root = from_js(&chain.to_value(), Some(&inline_at), &mut vec![]);
    let mut reused: Vec<(Vec<PathSeg>, &Kept)> = vec![];
    for (item, path) in item_slots(chain) {
        let Some(prev) = kept.get(id_of(item)) else { continue };
        if !same(&prev.value, item) {
            continue;
        }
        // The comments go in the print tree, so the printer gives the item a line of its own under them.
        if !prev.lead.is_empty()
            && let Some(node) = pnode_at(&mut root, &path)
        {
            node.lead.clone_from(&prev.lead);
        }
        reused.push((path, prev));
    }
    let mut fresh = Document::new(&(header_text(&old.header()) + &print_file(&root, &[], &[])?));
    for (path, prev) in reused {
        if fresh.get_source(&path)?.as_deref() == Some(prev.source.as_str()) {
            continue;
        }
        let Some(col) = fresh.position_of(&path)?.map(|p| p.1) else {
            continue;
        };
        fresh.replace_source(&path, &reindent(&prev.source, prev.col, col))?;
        // A multi-line string moves with its indentation inside it: shifted, it would read differently, so it goes back as written.
        if !fresh.get_in(&path)?.is_some_and(|v| same(&v, &prev.value)) {
            fresh.replace_source(&path, &prev.source)?;
        }
    }
    Ok(fresh.text().to_owned())
}
