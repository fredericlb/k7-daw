// print.rs — a value (with the comments a conversion carries) → k7x text, in the one canonical layout (docs/language.md).
// Canonical only for text written whole: an edit prints just the value it changes. A width is UTF-16 units, as JS counts it.

use super::layout::{Res, fail};
use super::value::{Value, js_number, js_trim_end, json_string, u16len};
use crate::ast::PathSeg;

/// A scalar to print; never a list or a map.
#[derive(Debug, Clone, PartialEq)]
pub enum PKind {
    Scalar(Value),
    List(Vec<PNode>),
    Map(Vec<PEntry>),
}

/// A value to print, with the comment lines that sit before it, after it, and inside it before its closing bracket.
#[derive(Debug, Clone, PartialEq)]
pub struct PNode {
    pub kind: PKind,
    pub lead: Vec<String>,
    pub after: Vec<String>,
    pub end: Vec<String>,
    pub blank: bool,
    /// Written on one line by its author: kept on one line whatever its width.
    pub inline: bool,
    /// Printed verbatim in place of the scalar: k7x source its writer already checked reads as one value.
    pub raw: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PEntry {
    pub key: String,
    pub value: PNode,
    pub lead: Vec<String>,
    pub blank: bool,
}

impl PNode {
    #[must_use]
    pub fn new(kind: PKind) -> PNode {
        PNode {
            kind,
            lead: vec![],
            after: vec![],
            end: vec![],
            blank: false,
            inline: false,
            raw: None,
        }
    }

    #[must_use]
    pub fn raw(source: &str) -> PNode {
        PNode {
            raw: Some(source.to_owned()),
            ..PNode::new(PKind::Scalar(Value::Null))
        }
    }
}

/// Which nodes a whole file prints on one line: by their path from the root and their value.
pub type InlineFn<'a> = &'a dyn Fn(&[PathSeg], &Value) -> bool;

/// What a whole file is printed with: comment lines on top, and the nodes written on one line.
#[derive(Default, Clone, Copy)]
pub struct StringifyOptions<'a> {
    pub header: &'a [String],
    pub inline: Option<InlineFn<'a>>,
}

/// Where a value sits: an entry's value prints a map as `{…}`, a list item as a chain, a call's lone argument list bare.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ctx {
    Entry,
    Item,
    Args,
}

const WIDTH: usize = 100;

fn ident_key(k: &str) -> bool {
    let mut b = k.bytes();
    matches!(b.next(), Some(c) if c.is_ascii_alphabetic() || c == b'_' || c == b'$')
        && b.all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'$' || c == b'-')
}

#[must_use]
pub fn print_key(key: &str) -> String {
    if ident_key(key) {
        key.to_owned()
    } else {
        json_string(key)
    }
}

fn print_scalar(v: &Value) -> Res<String> {
    Ok(match v {
        Value::Num(n) => {
            if !n.is_finite() {
                return fail(format!("k7x has no {}", js_number(*n)));
            }
            if *n == 0.0 { "0".into() } else { js_number(*n) }
        }
        Value::Str(s) if s.contains('\n') => {
            let mut out = String::from("`");
            for c in s.chars() {
                if c == '\\' || c == '`' {
                    out.push('\\');
                }
                out.push(c);
            }
            out.push('`');
            out
        }
        Value::Str(s) => json_string(s),
        Value::Bool(b) => b.to_string(),
        _ => "null".into(),
    })
}

/// A plain value as a node to print; `inline` marks the nodes to keep on one line, by their path from `value`.
#[must_use]
pub fn from_js(value: &Value, inline: Option<InlineFn>, path: &mut Vec<PathSeg>) -> PNode {
    let one = inline.is_some_and(|f| f(path, value));
    let mut node = match value {
        Value::List(items) => PNode::new(PKind::List(
            items
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    path.push(PathSeg::Index(i));
                    let n = from_js(v, inline, path);
                    path.pop();
                    n
                })
                .collect(),
        )),
        Value::Map(m) => PNode::new(PKind::Map(
            m.iter()
                .map(|(k, v)| {
                    path.push(PathSeg::Key(k.clone()));
                    let n = from_js(v, inline, path);
                    path.pop();
                    PEntry {
                        key: k.clone(),
                        value: n,
                        lead: vec![],
                        blank: false,
                    }
                })
                .collect(),
        )),
        scalar => PNode::new(PKind::Scalar(scalar.clone())),
    };
    if one {
        node.inline = true;
    }
    node
}

fn has_comments(n: &PNode) -> bool {
    !n.lead.is_empty()
        || !n.after.is_empty()
        || !n.end.is_empty()
        || match &n.kind {
            PKind::List(items) => items.iter().any(has_comments),
            PKind::Map(es) => es
                .iter()
                .any(|e| !e.lead.is_empty() || has_comments(&e.value)),
            PKind::Scalar(_) => false,
        }
}

fn as_chain(n: &PNode, ctx: Ctx) -> bool {
    matches!(&n.kind, PKind::Map(es) if ctx == Ctx::Item && !es.is_empty() && n.end.is_empty() && es.iter().all(|e| ident_key(&e.key)))
}

/// One-line text for a node with no comments in it, else None.
fn flat(n: &PNode, ctx: Ctx) -> Res<Option<String>> {
    if let Some(raw) = &n.raw {
        return Ok(Some(raw.clone()));
    }
    if has_comments(n) {
        return Ok(None);
    }
    Ok(Some(match &n.kind {
        PKind::Scalar(v) => print_scalar(v)?,
        PKind::List(items) => {
            let parts = items
                .iter()
                .map(|i| flat(i, Ctx::Item).map(|s| s.unwrap_or_else(|| "null".into())))
                .collect::<Res<Vec<_>>>()?;
            if ctx == Ctx::Args {
                parts.join(", ")
            } else {
                format!("[{}]", parts.join(", "))
            }
        }
        PKind::Map(es) => {
            let one =
                |e: &PEntry| flat(&e.value, Ctx::Entry).map(|s| s.unwrap_or_else(|| "null".into()));
            if as_chain(n, ctx) {
                let parts = es
                    .iter()
                    .map(|e| one(e).map(|v| format!("{}({v})", print_key(&e.key))))
                    .collect::<Res<Vec<_>>>()?;
                parts.join(".")
            } else if es.is_empty() {
                "{}".into()
            } else {
                let parts = es
                    .iter()
                    .map(|e| one(e).map(|v| format!("{}: {v}", print_key(&e.key))))
                    .collect::<Res<Vec<_>>>()?;
                format!("{{ {} }}", parts.join(", "))
            }
        }
    }))
}

fn comment(text: &str, indent: usize) -> String {
    if text.is_empty() {
        return String::new();
    }
    js_trim_end(&format!("{}//{text}", " ".repeat(indent))).to_owned()
}

/// ` //text` as a trailing comment, trimmed at the end.
fn trailing(first: Option<&String>) -> String {
    first.map_or_else(String::new, |f| js_trim_end(&format!(" //{f}")).to_owned())
}

/// A node's text, plus the comment lines it hands its container to place around its separator.
pub struct Out {
    pub text: String,
    pub lead: Vec<String>,
    pub after: Vec<String>,
}

/// `indent` is the indentation of the line the node starts on; `used` is how many columns precede it there.
///
/// # Errors
/// A number with no k7x spelling (NaN, infinity).
pub fn print(n: &PNode, indent: usize, ctx: Ctx, used: usize) -> Res<Out> {
    let lead = n.lead.clone();
    let after = n.after.clone();
    let bare = PNode {
        lead: vec![],
        after: vec![],
        ..n.clone()
    };
    let one = flat(&bare, ctx)?;
    if let Some(one) = &one {
        if !one.contains('\n') && (n.inline || used + u16len(one) <= WIDTH) {
            return Ok(Out {
                text: one.clone(),
                lead,
                after,
            });
        }
        if matches!(n.kind, PKind::Scalar(_)) {
            return Ok(Out {
                text: one.clone(),
                lead,
                after,
            });
        }
    }
    if ctx == Ctx::Args {
        return print(n, indent, Ctx::Entry, used);
    }
    if as_chain(n, ctx) {
        return chain(n, indent, used, lead, after);
    }
    let inner = indent + 2;
    let mut lines: Vec<String> = Vec::new();
    let push = |o: Out, head: &str, blank: bool, lines: &mut Vec<String>| {
        if blank && !lines.is_empty() {
            lines.push(String::new());
        }
        for c in &o.lead {
            lines.push(comment(c, inner));
        }
        lines.push(format!(
            "{}{head}{},{}",
            " ".repeat(inner),
            o.text,
            trailing(o.after.first())
        ));
        for c in o.after.iter().skip(1) {
            lines.push(comment(c, inner));
        }
    };
    match &n.kind {
        PKind::List(items) => {
            for item in items {
                let o = print(item, inner, Ctx::Item, inner)?;
                push(o, "", item.blank, &mut lines);
            }
        }
        PKind::Map(es) => {
            for e in es {
                let head = format!("{}: ", print_key(&e.key));
                let mut o = print(&e.value, inner, Ctx::Entry, inner + u16len(&head))?;
                o.lead = e.lead.iter().chain(o.lead.iter()).cloned().collect();
                push(o, &head, e.blank, &mut lines);
            }
        }
        PKind::Scalar(_) => {}
    }
    for c in &n.end {
        lines.push(comment(c, inner));
    }
    let (open, close) = if matches!(n.kind, PKind::List(_)) {
        ("[", "]")
    } else {
        ("{", "}")
    };
    Ok(Out {
        text: format!(
            "{open}\n{}\n{}{close}",
            lines.join("\n"),
            " ".repeat(indent)
        ),
        lead,
        after,
    })
}

fn chain(
    n: &PNode,
    indent: usize,
    used: usize,
    mut lead: Vec<String>,
    mut after: Vec<String>,
) -> Res<Out> {
    let PKind::Map(entries) = &n.kind else {
        return fail("k7x: a chain is a map");
    };
    let cont = indent + 2;
    let mut parts: Vec<String> = Vec::new();
    let mut pending: Vec<String> = Vec::new();
    for (i, e) in entries.iter().enumerate() {
        let head = format!("{}{}(", if i == 0 { "" } else { "." }, print_key(&e.key));
        let at = if i == 0 { used } else { cont };
        let o = print(
            &e.value,
            if i == 0 { indent } else { cont },
            Ctx::Entry,
            at + u16len(&head),
        )?;
        let entry_lead: Vec<String> = e.lead.iter().chain(o.lead.iter()).cloned().collect();
        if i == 0 {
            lead.extend(entry_lead.iter().cloned());
        }
        let mut text = format!("{head}{})", o.text);
        if i > 0 {
            let above: String = pending
                .iter()
                .chain(entry_lead.iter())
                .map(|c| format!("{}\n", comment(c, cont)))
                .collect();
            text = format!("\n{above}{}{text}", " ".repeat(cont));
        }
        parts.push(text);
        pending = vec![];
        if let Some(first) = o.after.first() {
            if i < entries.len() - 1 {
                parts.push(trailing(Some(first)));
                pending = o.after[1..].to_vec();
            } else {
                after = o.after.iter().chain(after.iter()).cloned().collect();
            }
        }
    }
    Ok(Out {
        text: parts.concat(),
        lead,
        after,
    })
}

/// A whole file: the root map's entries as one statement per line, with the file's own top and bottom comments.
///
/// # Errors
/// A root that is not a map, or a number with no k7x spelling.
pub fn print_file(root: &PNode, top: &[String], bottom: &[String]) -> Res<String> {
    let PKind::Map(entries) = &root.kind else {
        return fail("a k7x file is a map at its root");
    };
    let mut lines: Vec<String> = top.iter().map(|c| comment(c, 0)).collect();
    for e in entries {
        if (e.blank || e.value.blank) && lines.last().is_some_and(|l| !l.is_empty()) {
            lines.push(String::new());
        }
        let head = format!("{}(", print_key(&e.key));
        let o = print(&e.value, 0, Ctx::Entry, u16len(&head))?;
        for c in e.lead.iter().chain(o.lead.iter()) {
            lines.push(comment(c, 0));
        }
        lines.push(format!("{head}{}){}", o.text, trailing(o.after.first())));
        for c in o.after.iter().skip(1) {
            lines.push(comment(c, 0));
        }
    }
    for c in root.end.iter().chain(bottom.iter()) {
        lines.push(comment(c, 0));
    }
    Ok(format!("{}\n", lines.join("\n")))
}

/// The `//` lines of a file's header, one per line given (a line holding a newline is several), each ending its line.
#[must_use]
pub fn header_text(lines: &[String]) -> String {
    lines
        .iter()
        .flat_map(|l| split_lines(l))
        .map(|l| {
            if l.is_empty() {
                "//\n".to_owned()
            } else {
                format!("{}\n", js_trim_end(&format!("// {l}")))
            }
        })
        .collect()
}

/// `split(/\r?\n|\r/)`.
fn split_lines(s: &str) -> Vec<&str> {
    let b = s.as_bytes();
    let (mut out, mut from, mut i) = (Vec::new(), 0, 0);
    while i < b.len() {
        if b[i] == b'\r' || b[i] == b'\n' {
            out.push(&s[from..i]);
            i += if b[i] == b'\r' && b.get(i + 1) == Some(&b'\n') {
                2
            } else {
                1
            };
            from = i;
        } else {
            i += 1;
        }
    }
    out.push(&s[from..]);
    out
}

/// Canonical text for a plain value: no comments but the header, and the `inline` nodes on one line.
///
/// # Errors
/// A value that is not a map, or a number with no k7x spelling.
pub fn stringify(value: &Value, opts: &StringifyOptions) -> Res<String> {
    let body = print_file(&from_js(value, opts.inline, &mut Vec::new()), &[], &[])?;
    if opts.header.is_empty() {
        return Ok(body);
    }
    Ok(header_text(opts.header) + if body == "\n" { "" } else { &body })
}
