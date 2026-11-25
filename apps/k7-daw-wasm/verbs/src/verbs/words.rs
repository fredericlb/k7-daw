// words.rs — `splitArgs`: a line's arguments, a double-quoted run being one argument with its quotes dropped.
// A reader would get wrong an unclosed quote: it is no quote, and the run of non-spaces it starts is the argument.

use song_edit::js_text::is_js_space;

/// `/"([^"]*)"|(\S+)/g`, as the TypeScript matches it.
pub fn split_args(a: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = a;
    while let Some(c) = rest.chars().next() {
        if is_js_space(c) {
            rest = &rest[c.len_utf8()..];
            continue;
        }
        if c == '"'
            && let Some(end) = rest[1..].find('"')
        {
            out.push(rest[1..=end].to_owned());
            rest = &rest[end + 2..];
            continue;
        }
        let end = rest.find(is_js_space).unwrap_or(rest.len());
        out.push(rest[..end].to_owned());
        rest = &rest[end..];
    }
    out
}

/// `/^[A-Za-z0-9_.-]+$/`: a name a file, a lane or a timeline may take.
pub fn is_name(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_quoted_run_is_one_argument() {
        assert_eq!(split_args(r#" a "b c" "" d"#), ["a", "b c", "", "d"]);
        assert_eq!(split_args(r#"x"y z"#), [r#"x"y"#, "z"]);
        assert_eq!(split_args(r#""open"#), [r#""open"#]);
    }
}
