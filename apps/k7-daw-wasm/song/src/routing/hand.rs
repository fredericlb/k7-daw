// routing/hand.rs — the helpers the routing's hand decoders share: a located failure and the scalars a file is made of.
// A reader would get wrong where an error lands: on the key of an unknown or repeated key, else on the value read.

use crate::files::Finding;
use k7x::{Entry, MapNode, Node, Scalar};

/// A failure and the byte offset it is about.
pub(crate) struct Fail {
    pub at: usize,
    pub message: String,
}

pub(crate) type R<T> = Result<T, Fail>;

pub(crate) fn fail<T>(at: usize, message: impl Into<String>) -> R<T> {
    Err(Fail {
        at,
        message: message.into(),
    })
}

impl Fail {
    pub fn finding(self, file: &str, text: &str) -> Finding {
        Finding::at(file, text, self.at, self.message)
    }
}

pub(crate) fn text<'a>(node: &Node<'a>) -> R<&'a str> {
    match node {
        Node::Scalar {
            value: Scalar::Str(s), ..
        } => Ok(s),
        other => fail(other.start(), "expected text"),
    }
}

pub(crate) fn non_empty<'a>(node: &Node<'a>) -> R<&'a str> {
    let s = text(node)?;
    if s.is_empty() {
        return fail(node.start(), "expected a non-empty string");
    }
    Ok(s)
}

pub(crate) fn boolean(node: &Node) -> R<bool> {
    match node {
        Node::Scalar {
            value: Scalar::Bool(b), ..
        } => Ok(*b),
        other => fail(other.start(), "expected true or false"),
    }
}

pub(crate) fn finite(node: &Node) -> R<f64> {
    match node {
        Node::Scalar {
            value: Scalar::Num(n), ..
        } if n.is_finite() => Ok(*n),
        other => fail(other.start(), "expected a finite number"),
    }
}

pub(crate) fn object<'a>(node: &Node<'a>) -> R<MapNode<'a>> {
    match node {
        Node::Map(m) => Ok(*m),
        other => fail(other.start(), "expected an object"),
    }
}

pub(crate) fn list<'a>(node: &Node<'a>) -> R<&'a [Node<'a>]> {
    match node {
        Node::List { items, .. } => Ok(items),
        other => fail(other.start(), "expected a list"),
    }
}

/// A list of non-empty strings.
pub(crate) fn names(node: &Node) -> R<Vec<String>> {
    list(node)?.iter().map(|n| non_empty(n).map(str::to_owned)).collect()
}

/// A map's entries, refused when a key is not in `allowed` or is written twice.
pub(crate) fn strict<'a>(map: &MapNode<'a>, allowed: &[&str]) -> R<&'a [Entry<'a>]> {
    for (i, e) in map.entries.iter().enumerate() {
        if !allowed.contains(&e.key) {
            return fail(e.key_start, format!("unknown key `{}`", e.key));
        }
        if map.entries[..i].iter().any(|p| p.key == e.key) {
            return fail(e.key_start, format!("key `{}` is written twice", e.key));
        }
    }
    Ok(map.entries)
}

pub(crate) fn get<'a>(map: &MapNode<'a>, key: &str) -> Option<&'a Entry<'a>> {
    map.entries.iter().find(|e| e.key == key)
}

pub(crate) fn require<'a>(map: &MapNode<'a>, key: &str) -> R<&'a Entry<'a>> {
    get(map, key).map_or_else(|| fail(map.start, format!("missing key `{key}`")), Ok)
}

/// A node id or lane name: non-empty, and neither starts nor ends with `#`, the port separator.
pub(crate) fn node_name<'a>(node: &Node<'a>) -> R<&'a str> {
    let s = non_empty(node)?;
    if s.starts_with('#') || s.ends_with('#') {
        return fail(
            node.start(),
            "a node name cannot start or end with `#`, the port separator",
        );
    }
    Ok(s)
}
