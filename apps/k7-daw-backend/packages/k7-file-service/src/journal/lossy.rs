//! lossy — a journal read as the shim reads it: bad UTF-8 and a lone-surrogate escape become U+FFFD, never a torn file (1796).
//! What a reader gets wrong: only a `\u` escape inside a string is scanned; JSON has no escape anywhere else.

use std::borrow::Cow;

/// Which kinds of damage reading a journal repaired; each is counted in the runtime's stats.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Lossy {
    pub utf8: bool,
    pub surrogate: bool,
}

impl Lossy {
    pub fn any(self) -> bool {
        self.utf8 || self.surrogate
    }
}

fn hex4(b: &[u8]) -> Option<u32> {
    let s = std::str::from_utf8(b.get(..4)?).ok()?;
    if !s.bytes().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    u32::from_str_radix(s, 16).ok()
}

/// The text of `bytes`, with each invalid sequence a U+FFFD and each lone-surrogate escape the escape of U+FFFD.
pub fn read_lossy(bytes: &[u8]) -> (String, Lossy) {
    let text = String::from_utf8_lossy(bytes);
    let utf8 = matches!(text, Cow::Owned(_));
    let (text, surrogate) = scrub_surrogates(&text);
    (text.into_owned(), Lossy { utf8, surrogate })
}

fn scrub_surrogates(text: &str) -> (Cow<'_, str>, bool) {
    let b = text.as_bytes();
    let mut out: Option<String> = None;
    let (mut i, mut copied, mut in_str) = (0, 0, false);
    while i < b.len() {
        match b[i] {
            b'"' => in_str = !in_str,
            b'\\' if in_str => {
                if b.get(i + 1) == Some(&b'u')
                    && let Some(u) = hex4(&b[i + 2..])
                    && (0xd800..0xe000).contains(&u)
                {
                    let low = |at: usize| {
                        b.get(at) == Some(&b'\\')
                            && b.get(at + 1) == Some(&b'u')
                            && hex4(&b[(at + 2).min(b.len())..])
                                .is_some_and(|l| (0xdc00..0xe000).contains(&l))
                    };
                    if u < 0xdc00 && low(i + 6) {
                        i += 12;
                        continue;
                    }
                    let s = out.get_or_insert_with(String::new);
                    s.push_str(&text[copied..i]);
                    s.push_str("\\ufffd");
                    copied = i + 6;
                    i += 6;
                    continue;
                }
                i += 2;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    match out {
        Some(mut s) => {
            s.push_str(&text[copied..]);
            (Cow::Owned(s), true)
        }
        None => (Cow::Borrowed(text), false),
    }
}
