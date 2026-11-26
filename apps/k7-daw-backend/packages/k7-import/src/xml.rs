// xml.rs — a foreign project's XML as a plain tree, and the readings both mappers share: numbers, booleans, children by tag.
// What a reader gets wrong: `val` is Live's alone, it walks the <X><Manual Value="…"/></X> nesting a .als wraps every scalar in.

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

/// How deep a document may nest: Live's racks reach tens, a bomb reaches thousands.
const MAX_DEPTH: usize = 400;

/// How many elements a document may hold: a tree costs far more than the text it came from.
const MAX_NODES: usize = 12_000_000;

#[derive(Clone, Debug, Default)]
pub struct Node {
    pub tag: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
    /// The element's own text, trimmed.
    pub text: String,
}

impl Node {
    #[must_use]
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    /// The first child named `tag`.
    #[must_use]
    pub fn child(&self, tag: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.tag == tag)
    }

    /// Every child named `tag`, in document order.
    pub fn kids<'a>(&'a self, tag: &'a str) -> impl Iterator<Item = &'a Node> {
        self.children.iter().filter(move |c| c.tag == tag)
    }

    /// The first child at the end of `path`.
    #[must_use]
    pub fn at(&self, path: &[&str]) -> Option<&Node> {
        let mut cur = self;
        for key in path {
            cur = cur.child(key)?;
        }
        Some(cur)
    }

    /// Whether the element says anything: an empty element with no attribute is what a JS reading calls falsy.
    #[must_use]
    pub fn present(&self) -> bool {
        !(self.attrs.is_empty() && self.children.is_empty() && self.text.is_empty())
    }

    /// The children grouped by tag in the order each tag first appears, as an object keyed by tag lists them.
    #[must_use]
    pub fn grouped(&self) -> Vec<(&str, Vec<&Node>)> {
        let mut out: Vec<(&str, Vec<&Node>)> = Vec::new();
        for c in &self.children {
            match out.iter_mut().find(|(t, _)| *t == c.tag) {
                Some((_, v)) => v.push(c),
                None => out.push((c.tag.as_str(), vec![c])),
            }
        }
        out
    }

    /// Whether `tag` names an element anywhere below this one.
    #[must_use]
    pub fn holds(&self, tag: &str) -> bool {
        self.children
            .iter()
            .any(|c| c.tag == tag || c.holds(tag))
    }
}

/// Live's `Value` of the element at the end of `path`.
#[must_use]
pub fn val<'a>(node: Option<&'a Node>, path: &[&str]) -> Option<&'a str> {
    node?.at(path)?.attr("Value")
}

/// JS `parseFloat` over a string: the longest numeric prefix, none when there is not one.
#[must_use]
pub fn parse_float(s: &str) -> Option<f64> {
    let t = s.trim_start_matches(|c: char| c.is_whitespace() || c == '\u{feff}');
    let b = t.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    for word in ["Infinity"] {
        if t[i..].starts_with(word) {
            let v = f64::INFINITY;
            return Some(if b[0] == b'-' { -v } else { v });
        }
    }
    let digits = |from: usize| b[from..].iter().take_while(|c| c.is_ascii_digit()).count();
    let int = digits(i);
    let mut end = i + int;
    let mut frac = 0;
    if end < b.len() && b[end] == b'.' {
        frac = digits(end + 1);
        if int > 0 || frac > 0 {
            end += 1 + frac;
        }
    }
    if int == 0 && frac == 0 {
        return None;
    }
    if end < b.len() && (b[end] == b'e' || b[end] == b'E') {
        let mut e = end + 1;
        if e < b.len() && (b[e] == b'+' || b[e] == b'-') {
            e += 1;
        }
        let d = digits(e);
        if d > 0 {
            end = e + d;
        }
    }
    t[..end].parse::<f64>().ok()
}

/// A number from an optional attribute, `dflt` when it is absent or not finite.
#[must_use]
pub fn num(v: Option<&str>, dflt: f64) -> f64 {
    v.and_then(parse_float).filter(|n| n.is_finite()).unwrap_or(dflt)
}

#[must_use]
pub fn boolean(v: Option<&str>) -> bool {
    v == Some("true")
}

/// Float noise trimmed (both formats write about 17 digits) without lying about beats.
#[must_use]
pub fn round(n: f64, places: i32) -> f64 {
    let f = 10f64.powi(places);
    song_edit::start_time::js_round(n * f) / f
}

#[must_use]
pub fn round6(n: f64) -> f64 {
    round(n, 6)
}

/// An XML file that declares a doctype or an entity: neither format writes one and an entity can expand without bound.
pub fn refuse_declarations(xml: &str) -> Result<(), String> {
    let lower = xml.to_ascii_lowercase();
    if lower.contains("<!doctype") || lower.contains("<!entity") {
        return Err("this file declares XML entities, which no project holds — refused".into());
    }
    Ok(())
}

/// The five standard entities and the numeric references, the only ones a name may carry.
fn unescape(raw: &str) -> String {
    if !raw.contains('&') {
        return raw.to_owned();
    }
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let Some(end) = rest.find(';') else { break };
        match reference(&rest[1..end]) {
            Some(c) => out.push(c),
            None => out.push_str(&rest[..=end]),
        }
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    out
}

fn reference(name: &str) -> Option<char> {
    match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        _ => {
            let digits = name.strip_prefix('#')?;
            let code = match digits.strip_prefix(['x', 'X']) {
                Some(h) => u32::from_str_radix(h, 16).ok()?,
                None => digits.parse().ok()?,
            };
            char::from_u32(code)
        }
    }
}

fn open(e: &BytesStart<'_>) -> Result<Node, String> {
    let mut node = Node {
        tag: e.name().as_ref().to_owned(),
        ..Node::default()
    };
    for a in e.attributes().with_checks(false) {
        let a = a.map_err(|e| e.to_string())?;
        let key = a.key.as_ref().to_owned();
        let value = unescape(&a.value);
        node.attrs.push((key, value));
    }
    Ok(node)
}

/// The document's root element; an error is a sentence for the person, never the parser's position.
pub fn parse(xml: &str) -> Result<Node, String> {
    refuse_declarations(xml)?;
    let mut reader = Reader::from_str(xml);
    let mut stack: Vec<Node> = Vec::new();
    let mut root: Option<Node> = None;
    let mut count = 0usize;
    loop {
        let event = reader
            .read_event()
            .map_err(|e| format!("the XML does not read: {e}"))?;
        match event {
            Event::Start(e) => {
                count += 1;
                if stack.len() >= MAX_DEPTH || count > MAX_NODES {
                    return Err("this file nests or grows past what an import reads — refused".into());
                }
                stack.push(open(&e)?);
            }
            Event::Empty(e) => {
                count += 1;
                if count > MAX_NODES {
                    return Err("this file nests or grows past what an import reads — refused".into());
                }
                let node = open(&e)?;
                match stack.last_mut() {
                    Some(parent) => parent.children.push(node),
                    None => root = root.or(Some(node)),
                }
            }
            Event::End(_) => {
                let Some(mut node) = stack.pop() else { continue };
                node.text = k7x::edit::js_trim(&node.text).to_owned();
                match stack.last_mut() {
                    Some(parent) => parent.children.push(node),
                    None => root = root.or(Some(node)),
                }
            }
            Event::Text(t) => {
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(t.as_ref());
                }
            }
            Event::CData(t) => {
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(t.as_ref());
                }
            }
            Event::GeneralRef(r) => {
                if let Some(top) = stack.last_mut() {
                    let c = reference(r.as_ref());
                    top.text.extend(c);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    root.ok_or_else(|| "the XML holds no element".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_float_reads_as_parse_float_reads_it() {
        assert_eq!(parse_float("12.0_12402"), Some(12.0));
        assert_eq!(parse_float("  -3.5e2x"), Some(-350.0));
        assert_eq!(parse_float(".5"), Some(0.5));
        assert_eq!(parse_float("abc"), None);
        assert_eq!(parse_float("1e"), Some(1.0));
    }

    #[test]
    fn the_tree_keeps_attributes_text_and_the_standard_entities() {
        let n = parse("<a x=\"1&amp;2\"><b/><b y=\"z\"> hi &lt;there&gt; </b></a>").unwrap();
        assert_eq!(n.attr("x"), Some("1&2"));
        assert_eq!(n.kids("b").count(), 2);
        assert_eq!(n.children[1].text, "hi <there>");
        assert!(!n.children[0].present());
    }

    #[test]
    fn a_declared_entity_is_refused() {
        assert!(parse("<!DOCTYPE a [<!ENTITY x \"y\">]><a/>").is_err());
    }
}
