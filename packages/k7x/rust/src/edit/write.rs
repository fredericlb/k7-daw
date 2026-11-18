// write.rs — setting a value at a path and inserting a list item, as the smallest splice the layout around them allows.
// A reader would get wrong what is printed: only the written node, in the canonical layout; every byte around it stays.

use super::layout::{Host, Res, column, fail, head_blank, indent_of, line_start, pad, tail_end};
use super::print::{Ctx, PEntry, PKind, PNode, print, print_key};
use super::tree::{Tree, index_of};
use super::value::{Value, json_text, path_text};
use crate::ast::{ListForm, MapForm, PathSeg};

/// A segment as the node it is applied to reads it: on a list, a string of digits is an index.
fn seg_for(node: &Tree, seg: &PathSeg) -> PathSeg {
    match (node, index_of(seg)) {
        (Tree::List { .. }, Some(i)) => PathSeg::Index(i),
        _ => seg.clone(),
    }
}

fn seg_text(seg: &PathSeg) -> String {
    path_text(std::slice::from_ref(seg))
}

/// Sets `leaf` at a path, creating the maps on the way; nothing is written when `unchanged` holds for the node there.
pub fn write_at(
    h: &mut dyn Host,
    path: &[PathSeg],
    leaf: &PNode,
    unchanged: &dyn Fn(&Tree, &str) -> bool,
) -> Res<()> {
    let root = h.root()?;
    let mut node: &Tree = &root;
    let mut ctx = Ctx::Entry;
    for (i, p) in path.iter().enumerate() {
        let seg = seg_for(node, p);
        let Some(child) = node.child_at(&seg) else {
            return insert(h, node, ctx, &seg, wrap(&path[i + 1..], leaf)?);
        };
        if i == path.len() - 1 {
            if !unchanged(child, &h.text()) {
                replace(h, node, child, leaf)?;
            }
            return Ok(());
        }
        if matches!(child, Tree::Scalar { .. }) {
            return fail(format!(
                "k7x: {} is not a collection",
                path_text(&path[..=i])
            ));
        }
        ctx = if matches!(node, Tree::List { .. }) {
            Ctx::Item
        } else {
            Ctx::Entry
        };
        node = child;
    }
    Ok(())
}

/// Inserts into the list at a path before item `index`, on a line of its own when that item has one.
pub fn insert_at(h: &mut dyn Host, path: &[PathSeg], index: usize, leaf: &PNode) -> Res<()> {
    let root = h.root()?;
    let Some(node @ Tree::List { items, form, .. }) = root.node_at(path) else {
        return fail(format!("k7x: {} is not a list", path_text(path)));
    };
    let _ = node;
    if index >= items.len() {
        let mut p = path.to_vec();
        p.push(PathSeg::Index(items.len()));
        return write_at(h, &p, leaf, &|_, _| false);
    }
    let t = h.text();
    let at = items[index].start();
    let ind = indent_of(&t, at);
    if *form == ListForm::Brackets && head_blank(&t, at) {
        let text = print(leaf, ind, Ctx::Item, ind)?.text;
        return h.splice(at, at, &format!("{text},\n{}", pad(ind)));
    }
    let used = column(&t, at);
    let text = print(leaf, ind, Ctx::Item, used)?.text;
    h.splice(at, at, &format!("{text}, "))
}

/// The node to write where a path's tail is missing: the leaf inside the maps (or one-item list) that hold it.
fn wrap(rest: &[PathSeg], leaf: &PNode) -> Res<PNode> {
    let mut v = leaf.clone();
    for seg in rest.iter().rev() {
        v = match seg {
            PathSeg::Key(k) => PNode::new(PKind::Map(vec![PEntry {
                key: k.clone(),
                value: v,
                lead: vec![],
                blank: false,
            }])),
            PathSeg::Index(0) => PNode::new(PKind::List(vec![v])),
            PathSeg::Index(i) => return fail(format!("k7x: no item {i} to create")),
        };
    }
    Ok(v)
}

fn replace(h: &mut dyn Host, parent: &Tree, child: &Tree, leaf: &PNode) -> Res<()> {
    let t = h.text();
    let as_args = matches!(
        child,
        Tree::List {
            form: ListForm::Args,
            ..
        }
    ) && matches!(&leaf.kind, PKind::List(i) if i.len() >= 2);
    let ctx = if as_args {
        Ctx::Args
    } else if matches!(parent, Tree::List { .. }) {
        Ctx::Item
    } else {
        Ctx::Entry
    };
    let used = column(&t, child.start());
    let out = print(leaf, indent_of(&t, child.start()), ctx, used)?;
    h.splice(child.start(), child.end(), &out.text)
}

fn insert(h: &mut dyn Host, parent: &Tree, ctx: Ctx, seg: &PathSeg, leaf: PNode) -> Res<()> {
    let t = h.text();
    match parent {
        Tree::Scalar { .. } => fail("k7x: cannot insert into a scalar"),
        Tree::List {
            items,
            form,
            start,
            end,
        } => {
            if !matches!(seg, PathSeg::Index(i) if *i == items.len()) {
                return fail(format!("k7x: no item {} to set", seg_text(seg)));
            }
            let item = |ind: usize, used: usize| print(&leaf, ind, Ctx::Item, used).map(|o| o.text);
            if items.is_empty() && fill_empty(h, *start, *end, &item)? {
                return Ok(());
            }
            if items.is_empty() {
                let list = PNode::new(PKind::List(vec![leaf.clone()]));
                let ind = indent_of(&t, *start);
                return h.splice(*start, *end, &print(&list, ind, ctx, 0)?.text);
            }
            let last = &items[items.len() - 1];
            if *form == ListForm::Args {
                let text = print(&leaf, indent_of(&t, last.end()), Ctx::Item, 0)?.text;
                return h.splice(last.end(), last.end(), &format!(", {text}"));
            }
            let spans: Vec<(usize, usize)> = items.iter().map(|i| (i.start(), i.end())).collect();
            append_listed(h, *end, *start, &spans, &item)
        }
        Tree::Map(m) => {
            let key = seg_text(seg);
            let entries = &m.entries;
            if m.form == MapForm::Object {
                let head = format!("{}: ", print_key(&key));
                let hl = super::value::u16len(&head);
                let entry = |ind: usize, used: usize| {
                    print(&leaf, ind, Ctx::Entry, used + hl).map(|o| format!("{head}{}", o.text))
                };
                if entries.is_empty() && fill_empty(h, m.start, m.end, &entry)? {
                    return Ok(());
                }
                if entries.is_empty() {
                    let whole = PNode::new(PKind::Map(vec![PEntry {
                        key,
                        value: leaf.clone(),
                        lead: vec![],
                        blank: false,
                    }]));
                    let ind = indent_of(&t, m.start);
                    return h.splice(m.start, m.end, &print(&whole, ind, ctx, 0)?.text);
                }
                let spans: Vec<(usize, usize)> = entries.iter().map(|e| (e.start, e.end)).collect();
                return append_listed(h, m.end, m.start, &spans, &entry);
            }
            if m.form == MapForm::Root {
                let head = format!("{}(", print_key(&key));
                let line = format!(
                    "{head}{})",
                    print(&leaf, 0, Ctx::Entry, super::value::u16len(&head))?.text
                );
                let Some(last) = entries.last() else {
                    let nl = if t.is_empty() || t.ends_with('\n') {
                        ""
                    } else {
                        "\n"
                    };
                    return h.splice(t.len(), t.len(), &format!("{nl}{line}\n"));
                };
                let at = tail_end(&t, last.end, false).unwrap_or(last.end);
                return h.splice(at, at, &format!("\n{line}"));
            }
            let Some(last) = entries.last() else {
                return fail("k7x: an empty chain");
            };
            let head = format!(".{}(", print_key(&key));
            let hl = super::value::u16len(&head);
            let broken = entries
                .iter()
                .any(|e| e.dot.is_some_and(|d| head_blank(&t, d)));
            if !broken {
                let used = column(&t, last.end) + hl;
                let text = print(&leaf, indent_of(&t, last.end), Ctx::Entry, used)?.text;
                return h.splice(last.end, last.end, &format!("{head}{text})"));
            }
            let ind = match last.dot {
                Some(d) if head_blank(&t, d) => indent_of(&t, d),
                _ => indent_of(&t, m.start) + 2,
            };
            let at = tail_end(&t, last.end, false).unwrap_or(last.end);
            let text = print(&leaf, ind, Ctx::Entry, ind + hl)?.text;
            h.splice(at, at, &format!("\n{}{head}{text})", pad(ind)))
        }
    }
}

type Printer<'a> = &'a dyn Fn(usize, usize) -> Res<String>;

/// Fills an empty list or object whose brackets hold a comment, keeping it; false when they hold only blanks.
fn fill_empty(h: &mut dyn Host, start: usize, end: usize, text: Printer) -> Res<bool> {
    let t = h.text();
    let close = end - 1;
    let inner = &t[start + 1..close];
    if super::value::js_trim(inner).is_empty() {
        return Ok(false);
    }
    if inner.contains('\n') && head_blank(&t, close) {
        let ind = indent_of(&t, close) + 2;
        let at = line_start(&t, close);
        h.splice(at, at, &format!("{}{},\n", pad(ind), text(ind, ind)?))?;
        return Ok(true);
    }
    let sep = if inner
        .chars()
        .next_back()
        .is_some_and(super::value::is_js_space)
    {
        ""
    } else {
        " "
    };
    let used = column(&t, close) + sep.len();
    let after = if inner.chars().next().is_some_and(super::value::is_js_space) {
        " "
    } else {
        ""
    };
    h.splice(
        close,
        close,
        &format!("{sep}{}{after}", text(indent_of(&t, close), used)?),
    )?;
    Ok(true)
}

/// Appends to a bracketed list or an object, on a line of its own when the closing bracket has one.
fn append_listed(
    h: &mut dyn Host,
    end: usize,
    start: usize,
    items: &[(usize, usize)],
    text: Printer,
) -> Res<()> {
    let t = h.text();
    let (last_start, last_end) = items[items.len() - 1];
    let close = end - 1;
    if !t[last_end..close].contains('\n') || !head_blank(&t, close) {
        let used = column(&t, last_end) + 2;
        return h.splice(
            last_end,
            last_end,
            &format!(", {}", text(indent_of(&t, last_end), used)?),
        );
    }
    let ind = if head_blank(&t, last_start) {
        indent_of(&t, last_start)
    } else {
        indent_of(&t, start) + 2
    };
    let line = format!("{}{},\n", pad(ind), text(ind, ind)?);
    let rest = &t[last_end..];
    let comma = rest.trim_start_matches([' ', '\t']).starts_with(',');
    let at = line_start(&t, close);
    h.splice(
        last_end,
        at,
        &format!("{}{}{line}", if comma { "" } else { "," }, &t[last_end..at]),
    )
}

/// Whether a node already holds this plain value, so writing it would change nothing.
#[must_use]
pub fn holds(node: &Tree, value: &Value) -> bool {
    let a = node.to_value();
    json_text(&a) == json_text(value) && a.type_of() == value.type_of()
}
