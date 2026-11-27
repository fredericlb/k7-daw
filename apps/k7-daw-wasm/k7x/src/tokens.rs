// tokens.rs — what the editor reads off a parsed buffer: every string value with its field, every key with its path, the ▶ lines, the slice symbols.
// A reader would get wrong that the spans are the TREE's: the page maps them through the edits made since the tree was built (apps/k7-daw-frontend/packages/ui-editor/docs/k7xTree.md).

use crate::ast::{Entry, MapForm, MapNode, Node, Scalar};
use crate::json::utf16_offsets;

/// A string value of the buffer, its span in UTF-16 units with the quotes still in it.
#[derive(Clone, Debug, PartialEq)]
pub struct ValueToken {
    pub from: usize,
    pub to: usize,
    /// The span is a quoted source: the page takes one character off each end once it has mapped it.
    pub quoted: bool,
    /// The key whose value this is, inherited through lists; None at the root.
    pub field: Option<String>,
    /// Every key above it, outermost first, the field included.
    pub key_path: Vec<String>,
    pub value: String,
}

/// A map key of the buffer: what a graph's sections are made of.
#[derive(Clone, Debug, PartialEq)]
pub struct KeyToken {
    pub from: usize,
    pub to: usize,
    pub quoted: bool,
    /// The keys above it, outermost first; its own text is `key`.
    pub key_path: Vec<String>,
    pub key: String,
}

/// One step of the way from the root to a node.
#[derive(Clone, Copy)]
enum Step<'a> {
    Key(&'a str),
    Index,
}

fn keys_of(path: &[Step]) -> Vec<String> {
    path.iter()
        .filter_map(|s| match s {
            Step::Key(k) => Some((*k).to_owned()),
            Step::Index => None,
        })
        .collect()
}

/// Visits every node depth-first, parents first, with its path and the entry it is the value of.
fn walk<'a>(
    node: &Node<'a>,
    path: &mut Vec<Step<'a>>,
    entry: Option<&Entry<'a>>,
    visit: &mut impl FnMut(&Node<'a>, &[Step<'a>], Option<&Entry<'a>>),
) {
    visit(node, path, entry);
    match node {
        Node::List { items, .. } => {
            for item in *items {
                path.push(Step::Index);
                walk(item, path, None, visit);
                path.pop();
            }
        }
        Node::Map(m) => {
            for e in m.entries {
                path.push(Step::Key(e.key));
                walk(&e.value, path, Some(e), visit);
                path.pop();
            }
        }
        Node::Scalar { .. } => {}
    }
}

/// Whether the source at `[from, to)` (UTF-16 units) opens with a quote and is more than one character.
fn quoted(text: &str, from: usize, to: usize, byte: usize) -> bool {
    to - from >= 2 && matches!(text[byte..].chars().next(), Some('"' | '\'' | '`'))
}

/// Every non-empty string value of the tree, with its field and the keys above it.
#[must_use]
pub fn value_tokens(root: &MapNode, text: &str) -> Vec<ValueToken> {
    let u = utf16_offsets(text);
    let mut out = Vec::new();
    let root = Node::Map(*root);
    walk(&root, &mut Vec::new(), None, &mut |node, path, _| {
        let Node::Scalar {
            value: Scalar::Str(s),
            start,
            end,
        } = node
        else {
            return;
        };
        if s.is_empty() {
            return;
        }
        let keys = keys_of(path);
        let (from, to) = (u[*start], u[*end]);
        out.push(ValueToken {
            from,
            to,
            quoted: quoted(text, from, to, *start),
            field: keys.last().cloned(),
            key_path: keys,
            value: (*s).to_owned(),
        });
    });
    out
}

/// Every non-empty map key of the tree with the keys above it, in the order of the text.
#[must_use]
pub fn key_tokens(root: &MapNode, text: &str) -> Vec<KeyToken> {
    let u = utf16_offsets(text);
    let mut out = Vec::new();
    let root = Node::Map(*root);
    walk(&root, &mut Vec::new(), None, &mut |_, path, entry| {
        let Some(e) = entry.filter(|e| !e.key.is_empty()) else {
            return;
        };
        let (from, to) = (u[e.key_start], u[e.key_end]);
        let mut parents = keys_of(path);
        parents.pop();
        out.push(KeyToken {
            from,
            to,
            quoted: quoted(text, from, to, e.key_start),
            key_path: parents,
            key: e.key.to_owned(),
        });
    });
    // The page sorts by the span it keeps, quotes off.
    out.sort_by_key(|t| t.from + usize::from(t.quoted));
    out
}

/// What a ▶ plays: a file, a clip, or a window of a source in seconds.
#[derive(Clone, Debug, PartialEq)]
pub enum PreviewReq {
    Source { source: String },
    Clip { clip: String },
    Slice { source: String, from: f64, to: f64 },
}

/// A line that earns a ▶: the span the page anchors it on (UTF-16 units of the tree's text) and what it plays.
#[derive(Clone, Debug, PartialEq)]
pub struct PreviewTrigger {
    pub from: usize,
    pub to: usize,
    pub req: PreviewReq,
}

/// A map's string or number scalar fields by key; the last of a duplicate wins.
fn fields_of<'a>(map: &MapNode<'a>) -> Vec<(&'a str, Scalar<'a>)> {
    let mut out: Vec<(&str, Scalar)> = Vec::new();
    for e in map.entries {
        if let Node::Scalar { value, .. } = e.value
            && matches!(value, Scalar::Str(_) | Scalar::Num(_))
        {
            match out.iter_mut().find(|(k, _)| *k == e.key) {
                Some(slot) => slot.1 = value,
                None => out.push((e.key, value)),
            }
        }
    }
    out
}

fn field<'a>(fields: &[(&str, Scalar<'a>)], key: &str) -> Option<Scalar<'a>> {
    fields.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
}

/// Every ▶ the tree earns; a slice symbol is resolved after the walk, since the source it belongs to may sit below it.
#[must_use]
pub fn preview_triggers(root: &MapNode, text: &str) -> Vec<PreviewTrigger> {
    let u = utf16_offsets(text);
    let mut out = Vec::new();
    let mut slices: Vec<(usize, usize, f64, f64)> = Vec::new();
    let mut shared: Option<String> = None;
    let root = Node::Map(*root);
    walk(&root, &mut Vec::new(), None, &mut |node, _, entry| {
        if let (Some(e), Node::Scalar { value, .. }) = (entry, node)
            && e.key == "source"
        {
            let Scalar::Str(source) = value else { return };
            if source.is_empty() {
                return;
            }
            shared.get_or_insert_with(|| (*source).to_owned());
            out.push(PreviewTrigger {
                from: u[e.start],
                to: u[e.end],
                req: PreviewReq::Source {
                    source: (*source).to_owned(),
                },
            });
            return;
        }
        let Node::Map(m) = node else { return };
        if m.form == MapForm::Root {
            return;
        }
        let fields = fields_of(m);
        // A rack symbol answers to a note and plays its own clip or a window of the source; without `note` it is not one.
        if field(&fields, "note").is_none() {
            return;
        }
        let (from, to) = (u[m.start], u[m.end]);
        if let Some(Scalar::Str(clip)) = field(&fields, "clip")
            && !clip.is_empty()
        {
            out.push(PreviewTrigger {
                from,
                to,
                req: PreviewReq::Clip {
                    clip: clip.to_owned(),
                },
            });
            return;
        }
        let num = |k: &str| match field(&fields, k) {
            Some(Scalar::Num(n)) if n.is_finite() => Some(n),
            _ => None,
        };
        if let (Some(a), Some(b)) = (num("from"), num("to")) {
            slices.push((from, to, a, b));
        }
    });
    if let Some(source) = shared {
        for (from, to, a, b) in slices {
            out.push(PreviewTrigger {
                from,
                to,
                req: PreviewReq::Slice {
                    source: source.clone(),
                    from: a,
                    to: b,
                },
            });
        }
    }
    out
}

/// A slice rack's symbol: its window in seconds and where its numbers and name sit, in UTF-16 units of the tree's text.
#[derive(Clone, Debug, PartialEq)]
pub struct SliceSym {
    pub symbol: String,
    pub from: f64,
    pub to: f64,
    pub from_range: (usize, usize),
    pub to_range: (usize, usize),
    /// The symbol's NAME, quotes excluded.
    pub key_range: (usize, usize),
}

fn entry_of<'a, 'b>(map: &'b MapNode<'a>, key: &str) -> Option<&'b Entry<'a>> {
    map.entries.iter().find(|e| e.key == key)
}

fn map_of<'a>(map: &MapNode<'a>, key: &str) -> Option<MapNode<'a>> {
    match entry_of(map, key)?.value {
        Node::Map(m) => Some(m),
        _ => None,
    }
}

/// The symbols of a parsed file: each one's window and the spans of its two numbers and its name; None for no slice rack.
#[must_use]
pub fn slices_of(root: &MapNode, text: &str) -> Option<Vec<SliceSym>> {
    let rack = map_of(root, "rack")?;
    match entry_of(&rack, "rack_type")?.value {
        Node::Scalar {
            value: Scalar::Str("slice"),
            ..
        } => {}
        _ => return None,
    }
    let symbols = map_of(&rack, "symbols")?;
    let u = utf16_offsets(text);
    let mut out = Vec::new();
    for e in symbols.entries {
        let Node::Map(m) = e.value else { continue };
        let num = |k: &str| match entry_of(&m, k).map(|e| e.value) {
            Some(Node::Scalar {
                value: Scalar::Num(n),
                start,
                end,
            }) => Some((n, (u[start], u[end]))),
            _ => None,
        };
        let (Some((from, from_range)), Some((to, to_range))) = (num("from"), num("to")) else {
            continue;
        };
        let q = matches!(text[e.key_start..].chars().next(), Some('"' | '\''));
        let (ks, ke) = (u[e.key_start], u[e.key_end]);
        out.push(SliceSym {
            symbol: e.key.to_owned(),
            from,
            to,
            from_range,
            to_range,
            key_range: if q { (ks + 1, ke - 1) } else { (ks, ke) },
        });
    }
    (!out.is_empty()).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Bump, parse_tree};

    fn triggers(text: &str) -> Vec<(usize, PreviewReq)> {
        let bump = Bump::new();
        let root = parse_tree(text, &bump).unwrap();
        let line = |from: usize| {
            text[..text.char_indices().nth(from).map_or(text.len(), |(i, _)| i)]
                .matches('\n')
                .count()
                + 1
        };
        let mut v: Vec<_> = preview_triggers(&root, text)
            .into_iter()
            .map(|t| (line(t.from), t.req))
            .collect();
        v.sort_by_key(|(l, _)| *l);
        v
    }

    fn src(s: &str) -> PreviewReq {
        PreviewReq::Source { source: s.into() }
    }

    #[test]
    fn a_clips_own_source_earns_a_play_button() {
        assert_eq!(
            triggers("source(\"kick.wav\")\n"),
            vec![(1, src("kick.wav"))]
        );
        assert!(triggers("source(\"\")\n").is_empty());
    }

    #[test]
    fn a_samples_rack_symbols_each_play_their_clip() {
        let text = "rack({\n  rack_type: \"samples\",\n  symbols: {\n    BD: { note: 36, clip: \"kick\" },\n    SN: { note: 38, clip: \"snare\" },\n  },\n})\n";
        assert_eq!(
            triggers(text),
            vec![
                (
                    4,
                    PreviewReq::Clip {
                        clip: "kick".into()
                    }
                ),
                (
                    5,
                    PreviewReq::Clip {
                        clip: "snare".into()
                    }
                )
            ]
        );
    }

    #[test]
    fn a_slice_symbol_resolves_against_the_source_declared_after_it() {
        let text = "rack({\n  rack_type: \"slice\",\n  symbols: {\n    BD: { note: 36, from: 0, to: 0.5 },\n  },\n  source: \"break.wav\",\n})\n";
        let got = triggers(text);
        assert!(got.contains(&(
            4,
            PreviewReq::Slice {
                source: "break.wav".into(),
                from: 0.0,
                to: 0.5
            }
        )));
        assert!(got.contains(&(6, src("break.wav"))));
    }

    #[test]
    fn a_symbol_without_a_note_or_a_source_earns_nothing() {
        assert!(triggers("rack({ symbols: { BD: { clip: \"kick\" } } })\n").is_empty());
        assert!(triggers("rack({ symbols: { BD: { note: 36, from: 0, to: 1 } } })\n").is_empty());
    }

    #[test]
    fn values_inherit_the_key_above_a_list() {
        let text = "mix({ in: [\"melody\", \"pad\"], track: \"x\" })\n";
        let bump = Bump::new();
        let root = parse_tree(text, &bump).unwrap();
        let v = value_tokens(&root, text);
        let fields: Vec<_> = v
            .iter()
            .map(|t| (t.value.as_str(), t.field.as_deref()))
            .collect();
        assert!(fields.contains(&("melody", Some("in"))));
        assert!(fields.contains(&("pad", Some("in"))));
        assert!(fields.contains(&("x", Some("track"))));
        assert!(v.iter().all(|t| t.quoted));
    }

    #[test]
    fn keys_carry_the_keys_above_them() {
        let text = "graph({ \"a b\": { c: 1 } })\n";
        let bump = Bump::new();
        let root = parse_tree(text, &bump).unwrap();
        let k = key_tokens(&root, text);
        let c = k.iter().find(|t| t.key == "c").unwrap();
        assert_eq!(c.key_path, vec!["graph".to_owned(), "a b".to_owned()]);
        assert!(k.iter().find(|t| t.key == "a b").unwrap().quoted);
    }
}
