// scan.rs — one string field read out of a file's text by a scan, half-typed files included.
// A reader would get wrong that this is no parse: `key("v")` or `key: "v"` is found anywhere, so a text that does not read still answers.

use crate::edit::{Value, is_js_space, parse};

/// The characters a key must not follow: the scan's `(?:^|[^A-Za-z0-9_$-])`.
fn word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '$' || c == '-'
}

/// The end (past the closing quote) of the string literal opening at `at`, as `"(?:[^"\\\n]|\\.)*"` reads it.
fn literal_end(t: &[char], at: usize) -> Option<usize> {
    let q = *t.get(at)?;
    let mut i = at + 1;
    loop {
        match *t.get(i)? {
            c if c == q => return Some(i + 1),
            '\n' => return None,
            // `.` matches anything but a line end.
            '\\' => match t.get(i + 1)? {
                '\n' | '\r' | '\u{2028}' | '\u{2029}' => return None,
                _ => i += 2,
            },
            _ => i += 1,
        }
    }
}

/// The first `key("v")` or `key: "v"` of `text`, its literal unquoted by the reader; None for none, an empty or an unreadable one.
#[must_use]
pub fn scalar_field(text: &str, key: &str) -> Option<String> {
    let t: Vec<char> = text.chars().collect();
    let k: Vec<char> = key.chars().collect();
    let skip = |mut i: usize| {
        while t.get(i).copied().is_some_and(is_js_space) {
            i += 1;
        }
        i
    };
    for at in 0..t.len() {
        if at > 0 && word_char(t[at - 1]) {
            continue;
        }
        if t.get(at..at + k.len()) != Some(k.as_slice()) {
            continue;
        }
        let i = skip(at + k.len());
        if !matches!(t.get(i), Some('(' | ':')) {
            continue;
        }
        let start = skip(i + 1);
        if !matches!(t.get(start), Some('"' | '\'')) {
            continue;
        }
        let Some(end) = literal_end(&t, start) else {
            continue;
        };
        let literal: String = t[start..end].iter().collect();
        return match parse(&format!("v({literal})")) {
            Ok(Value::Map(entries)) => match entries.into_iter().find(|(k, _)| k == "v") {
                Some((_, Value::Str(s))) if !s.is_empty() => Some(s),
                _ => None,
            },
            _ => None,
        };
    }
    None
}

#[cfg(test)]
mod tests {
    use super::scalar_field;

    #[test]
    fn a_call_or_an_object_key_gives_its_string() {
        assert_eq!(
            scalar_field("source(\"kick.wav\")\n", "source").as_deref(),
            Some("kick.wav")
        );
        assert_eq!(
            scalar_field("x({ source: 'a b' })", "source").as_deref(),
            Some("a b")
        );
        assert_eq!(
            scalar_field("source ( \"k\" )", "source").as_deref(),
            Some("k")
        );
    }

    #[test]
    fn the_key_must_stand_alone() {
        assert_eq!(scalar_field("my_source(\"k\")", "source"), None);
        assert_eq!(scalar_field("a-source(\"k\")", "source"), None);
        assert_eq!(
            scalar_field("x.source(\"k\")", "source").as_deref(),
            Some("k")
        );
    }

    #[test]
    fn a_half_typed_file_still_answers() {
        assert_eq!(
            scalar_field("source(\"k\")\nnotes([\n", "source").as_deref(),
            Some("k")
        );
        assert_eq!(scalar_field("source(\"unclosed", "source"), None);
    }

    #[test]
    fn an_empty_value_is_none_and_the_first_candidate_decides() {
        assert_eq!(scalar_field("source(\"\")", "source"), None);
        assert_eq!(
            scalar_field("source(1) source(\"b\")", "source").as_deref(),
            Some("b")
        );
        assert_eq!(scalar_field("source(\"\") source(\"b\")", "source"), None);
    }

    #[test]
    fn escapes_are_read_by_the_reader() {
        assert_eq!(
            scalar_field("source(\"a\\\"b\")", "source").as_deref(),
            Some("a\"b")
        );
        assert_eq!(
            scalar_field("source(\"a\\nb\")", "source").as_deref(),
            Some("a\nb")
        );
    }
}
