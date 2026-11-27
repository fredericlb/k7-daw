// cursor.rs — where the cursor sits in key terms, read from the text BEFORE it by a tolerant scan, not a parse.
// A reader would get wrong why this is no parse: completion runs on half-written text, which the reader refuses (apps/k7-daw-frontend/packages/ui-editor/docs/completion.md).

/// What the cursor is filling: the same four answers the yaml mode gives, so one vocabulary serves both formats.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cursor {
    /// The keys of the mapping the cursor is in (or whose value it fills), outermost first; list items add no key.
    pub path: Vec<String>,
    /// Is the cursor where a key goes: a statement's start, after `.` in a chain, or an object's key?
    pub in_key: bool,
    /// The key whose value the cursor is filling, when it is filling one.
    pub value_of: Option<String>,
    /// Is the cursor at an item of a list, which may be a value or a chain of keys?
    pub in_seq_item: bool,
    /// Is the cursor inside a string it is typing?
    pub in_string: bool,
}

enum Frame {
    /// A mapping written as a chain, the file's root included: keys come first, after `.` or at a statement's start.
    Chain { path: Vec<String>, root: bool, expect_key: bool },
    /// A call's arguments: the value of `key` in the mapping at `path`.
    Call { path: Vec<String>, key: String },
    /// `{ … }`: keys until `:`, then the value of that key.
    Object { path: Vec<String>, key: Option<String>, after_colon: bool },
    /// `[ … ]`, the value of `key` in the mapping at `path`.
    List { path: Vec<String>, key: String },
}

impl Frame {
    fn path(&self) -> &[String] {
        match self {
            Frame::Chain { path, .. } | Frame::Call { path, .. } | Frame::Object { path, .. } | Frame::List { path, .. } => path,
        }
    }
}

/// The path and key a value at the top of the stack belongs to, or None when the top is no value position.
fn value_slot(top: &Frame) -> Option<(Vec<String>, String)> {
    match top {
        Frame::Call { path, key } | Frame::List { path, key } => Some((path.clone(), key.clone())),
        Frame::Object {
            path,
            key: Some(key),
            after_colon: true,
        } => Some((path.clone(), key.clone())),
        _ => None,
    }
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || c == '$'
}

fn is_ident_part(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '$' || c == '-'
}

/// The length of the number at `i` (`-?\d+(\.\d+)?([eE][+-]?\d+)?`), or 0.
fn number_len(t: &[char], i: usize) -> usize {
    let digits = |mut j: usize| {
        let s = j;
        while j < t.len() && t[j].is_ascii_digit() {
            j += 1;
        }
        (j, j > s)
    };
    let mut j = i;
    if t.get(j) == Some(&'-') {
        j += 1;
    }
    let (after, any) = digits(j);
    if !any {
        return 0;
    }
    j = after;
    if t.get(j) == Some(&'.') {
        let (a, any) = digits(j + 1);
        if any {
            j = a;
        }
    }
    if matches!(t.get(j), Some('e' | 'E')) {
        let mut k = j + 1;
        if matches!(t.get(k), Some('+' | '-')) {
            k += 1;
        }
        let (a, any) = digits(k);
        if any {
            j = a;
        }
    }
    j - i
}

fn starts_with(t: &[char], i: usize, s: &str) -> bool {
    s.chars().enumerate().all(|(k, c)| t.get(i + k) == Some(&c))
}

fn find(t: &[char], from: usize, s: &str) -> Option<usize> {
    (from..t.len()).find(|&i| starts_with(t, i, s))
}

/// Scan `text` (everything before the cursor) and say what the cursor is filling.
#[must_use]
pub fn cursor(text: &str) -> Cursor {
    let t: Vec<char> = text.chars().collect();
    let mut stack = vec![Frame::Chain {
        path: vec![],
        root: true,
        expect_key: true,
    }];
    let mut pending: Option<String> = None;
    let mut i = 0;
    let mut in_string = false;
    while i < t.len() {
        let c = t[i];
        if matches!(c, ' ' | '\t' | '\n' | '\r') {
            i += 1;
            continue;
        }
        if starts_with(&t, i, "//") {
            i = find(&t, i, "\n").unwrap_or(t.len());
            continue;
        }
        if starts_with(&t, i, "/*") {
            i = find(&t, i + 2, "*/").map_or(t.len(), |close| close + 2);
            continue;
        }
        if matches!(c, '"' | '\'' | '`') {
            let mut j = i + 1;
            while j < t.len() && t[j] != c {
                j += if t[j] == '\\' { 2 } else { 1 };
            }
            if j >= t.len() {
                in_string = true;
                if let Some(Frame::Object { after_colon: false, .. }) = stack.last() {
                    pending = None;
                }
                break;
            }
            let key: String = t[i + 1..j].iter().collect();
            let is_key = matches!(
                stack.last(),
                Some(Frame::Chain { expect_key: true, .. } | Frame::Object { after_colon: false, .. })
            );
            pending = is_key.then_some(key);
            i = j + 1;
            continue;
        }
        let n = number_len(&t, i);
        if n > 0 {
            i += n;
            pending = None;
            continue;
        }
        if is_ident_start(c) {
            let mut j = i + 1;
            while j < t.len() && is_ident_part(t[j]) {
                j += 1;
            }
            let word: String = t[i..j].iter().collect();
            i = j;
            let top = stack.last().expect("the root frame stays");
            let slot = value_slot(top);
            // At a value position, a word is the head of a chain: the value is a mapping, its keys under the slot's path.
            if let Some((path, key)) = slot {
                let mut p = path;
                p.push(key);
                stack.push(Frame::Chain {
                    path: p,
                    root: false,
                    expect_key: false,
                });
            } else if let Some(Frame::Chain { expect_key, .. }) = stack.last_mut() {
                *expect_key = false;
            }
            pending = Some(word);
            continue;
        }
        i += 1;
        match c {
            '(' => {
                let key = pending.take();
                let owner_path = stack.last().map(|f| (f.path().to_vec(), matches!(f, Frame::Chain { .. } | Frame::Object { .. })));
                match (key, owner_path) {
                    (Some(key), Some((path, true))) => stack.push(Frame::Call { path, key }),
                    _ => stack.push(Frame::Call {
                        path: vec![],
                        key: String::new(),
                    }),
                }
            }
            ')' => {
                if matches!(stack.last(), Some(Frame::Chain { root: false, .. })) {
                    stack.pop();
                }
                if matches!(stack.last(), Some(Frame::Call { .. })) {
                    stack.pop();
                }
                reset_owner(&mut stack);
            }
            '.' => {
                if let Some(Frame::Chain { expect_key, .. }) = stack.last_mut() {
                    *expect_key = true;
                }
            }
            ';' => {
                if let Some(Frame::Chain { root: true, expect_key, .. }) = stack.last_mut() {
                    *expect_key = true;
                }
            }
            '[' | '{' => {
                let top = stack.last().expect("the root frame stays");
                let (path, key) = match value_slot(top) {
                    Some((p, k)) => (p, k),
                    None => (top.path().to_vec(), String::new()),
                };
                if c == '[' {
                    stack.push(Frame::List { path, key });
                } else {
                    // `filter(Boolean)`: an empty key, quoted or not, is dropped from the whole path.
                    let mut p: Vec<String> = path.into_iter().filter(|s| !s.is_empty()).collect();
                    if !key.is_empty() {
                        p.push(key);
                    }
                    stack.push(Frame::Object {
                        path: p,
                        key: None,
                        after_colon: false,
                    });
                }
                pending = None;
            }
            ']' | '}' => {
                if matches!(stack.last(), Some(Frame::Chain { root: false, .. })) {
                    stack.pop();
                }
                if stack.len() > 1 {
                    stack.pop();
                }
                reset_owner(&mut stack);
            }
            ':' => {
                if let Some(Frame::Object { key, after_colon, .. }) = stack.last_mut() {
                    *key = pending.clone();
                    *after_colon = true;
                }
                pending = None;
            }
            ',' => {
                if matches!(stack.last(), Some(Frame::Chain { root: false, .. })) {
                    stack.pop();
                }
                if let Some(Frame::Object { key, after_colon, .. }) = stack.last_mut() {
                    *after_colon = false;
                    *key = None;
                }
                pending = None;
            }
            _ => {}
        }
    }
    answer(&stack, text, in_string, pending.is_some())
}

/// After a closing bracket, the frame below is a chain again: it expects a key only when it is the root.
fn reset_owner(stack: &mut [Frame]) {
    if let Some(Frame::Chain { root, expect_key, .. }) = stack.last_mut() {
        *expect_key = *root;
    }
}

fn answer(stack: &[Frame], text: &str, in_string: bool, has_pending: bool) -> Cursor {
    let top = stack.last().expect("the root frame stays");
    // A word being typed sits on the stack as a pending key: the position is the one before it.
    let typing = !in_string && text.chars().next_back().is_some_and(is_ident_part);
    let plain = |path: Vec<String>, in_key: bool, value_of: Option<String>, in_seq_item: bool| Cursor {
        path,
        in_key,
        value_of,
        in_seq_item,
        in_string,
    };
    match top {
        Frame::Chain { path, expect_key, .. } => plain(path.clone(), *expect_key || (typing && has_pending), None, false),
        Frame::Object {
            path,
            key,
            after_colon,
        } => {
            if *after_colon {
                plain(path.clone(), false, key.clone(), false)
            } else {
                plain(path.clone(), true, None, false)
            }
        }
        Frame::List { path, key } => {
            let mut p = path.clone();
            p.push(key.clone());
            plain(p, true, None, true)
        }
        Frame::Call { path, key } => plain(path.clone(), false, (!key.is_empty()).then(|| key.clone()), false),
    }
}

#[cfg(test)]
mod tests {
    use super::{Cursor, cursor};

    fn key(path: &[&str]) -> Cursor {
        Cursor {
            path: path.iter().map(|s| (*s).to_owned()).collect(),
            in_key: true,
            value_of: None,
            in_seq_item: false,
            in_string: false,
        }
    }

    fn value(path: &[&str], of: &str, in_string: bool) -> Cursor {
        Cursor {
            in_key: false,
            value_of: Some(of.to_owned()),
            in_string,
            ..key(path)
        }
    }

    #[test]
    fn a_top_level_key_at_the_start_while_typing_and_after_a_statement() {
        for text in ["", "ext", "extends(\"x\")\npar", "extends(\"x\")\n", "// a comment\n"] {
            assert_eq!(cursor(text), key(&[]), "{text:?}");
        }
    }

    #[test]
    fn a_call_value_inside_its_quotes_or_not() {
        assert_eq!(cursor("extends("), value(&[], "extends", false));
        assert_eq!(cursor("extends(\"@wor"), value(&[], "extends", true));
    }

    #[test]
    fn an_objects_keys_and_values_under_the_call_that_holds_it() {
        assert_eq!(cursor("project({ "), key(&["project"]));
        assert_eq!(cursor("project({ name: \"x\", tem"), key(&["project"]));
        assert_eq!(cursor("project({ video: "), value(&["project"], "video", false));
        assert_eq!(cursor("engines({ tracktion: { pa"), key(&["engines", "tracktion"]));
    }

    #[test]
    fn a_list_item_as_a_value_or_a_chain() {
        let item = Cursor {
            in_seq_item: true,
            ..key(&["steps"])
        };
        assert_eq!(cursor("steps([\n  "), item);
        assert_eq!(cursor("steps([\n  ext"), key(&["steps"]));
        assert_eq!(cursor("steps([\n  extends(\"x\")\n    .eng"), key(&["steps"]));
        assert_eq!(cursor("steps([\n  extends(\"x\").engine("), value(&["steps"], "engine", false));
        assert_eq!(cursor("steps([\n  id(1),\n  extends("), value(&["steps"], "extends", false));
    }

    #[test]
    fn what_was_closed_hands_the_cursor_back() {
        assert_eq!(cursor("project({ name: \"x\", tempo: 120 })\nmar"), key(&[]));
        assert_eq!(cursor("steps([id(1), to([chain(\"@out\")])])\n"), key(&[]));
        assert_eq!(cursor("params({ Level: -12, Gain: 1e5, "), key(&["params"]));
    }

    #[test]
    fn a_chain_written_as_a_calls_value_sits_under_that_calls_key() {
        assert_eq!(cursor("engines(tracktion("), value(&["engines"], "tracktion", false));
        assert_eq!(cursor("engines(tracktion(params({ Ra"), key(&["engines", "tracktion", "params"]));
    }
}
