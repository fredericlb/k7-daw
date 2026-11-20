//! fold.rs — a scratch folder named `<DIR>` in every spelling an answer gives it, so two answers compare by shape.
//! What a reader gets wrong: the path after the folder is made of slashes, whatever separator the answer used.

/// The length of one separator at the start of `s`: `/`, `\` or the doubled `\\` of a JSON string.
fn sep_len(s: &str) -> usize {
    if s.starts_with("\\\\") {
        2
    } else {
        usize::from(s.starts_with(['/', '\\']))
    }
}

/// Where `dir`'s parts end at the start of `s`, whatever the separators between them; `None` if `s` does not open with it.
fn match_dir(s: &str, parts: &[&str], lead: bool) -> Option<usize> {
    let mut i = 0;
    if lead {
        i += Some(sep_len(s)).filter(|n| *n > 0)?;
    }
    for (k, part) in parts.iter().enumerate() {
        if k > 0 {
            i += Some(sep_len(&s[i..])).filter(|n| *n > 0)?;
        }
        i += s[i..].starts_with(part).then_some(part.len())?;
    }
    Some(i)
}

/// Replaces `dir`, in whatever mix of separators `s` spells it, by `<DIR>`.
fn name_dir(s: &str, dir: &str) -> String {
    let parts: Vec<&str> = dir.split(['/', '\\']).filter(|p| !p.is_empty()).collect();
    if parts.is_empty() {
        return s.to_string();
    }
    let lead = dir.starts_with('/');
    let mut out = String::new();
    let mut at = 0;
    while at < s.len() {
        if let Some(n) = match_dir(&s[at..], &parts, lead) {
            out.push_str("<DIR>");
            at += n;
        } else {
            let step = s[at..].chars().next().map_or(1, char::len_utf8);
            out.push_str(&s[at..at + step]);
            at += step;
        }
    }
    out
}

/// A folder named `<DIR>` in every spelling an answer gives it, the path after it made of slashes.
pub fn fold_dir(s: &str, dir: &str) -> String {
    let named = name_dir(s, dir);
    let mut out = String::new();
    let mut rest = named.as_str();
    while let Some(at) = rest.find("<DIR>") {
        let (head, tail) = rest.split_at(at + "<DIR>".len());
        out.push_str(head);
        let mut chars = tail.char_indices().peekable();
        let mut end = tail.len();
        while let Some((i, c)) = chars.next() {
            match c {
                '"' | '\'' | ' ' | ',' | ')' | '}' | ']' => {
                    end = i;
                    break;
                }
                '\\' if matches!(chars.peek(), Some((_, '"'))) => {
                    end = i;
                    break;
                }
                '\\' => {
                    if matches!(chars.peek(), Some((_, '\\'))) {
                        chars.next();
                    }
                    out.push('/');
                }
                c => out.push(c),
            }
        }
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod fold_tests {
    use super::fold_dir;

    #[test]
    fn folds_every_spelling() {
        let dir = r"C:\t\x";
        assert_eq!(fold_dir(r"C:\t\x\a\b", dir), "<DIR>/a/b");
        assert_eq!(
            fold_dir(r#"{"p":"C:\\t\\x\\a"}"#, dir),
            r#"{"p":"<DIR>/a"}"#
        );
        assert_eq!(fold_dir("C:/t/x/a b", dir), "<DIR>/a b");
        assert_eq!(fold_dir(r"C:/t\x/a\b", "C:/t/x"), "<DIR>/a/b");
        assert_eq!(fold_dir(r"C:\\t\\x\\a", "C:/t/x"), "<DIR>/a");
        assert_eq!(fold_dir("/home/x/a", "/home/x"), "<DIR>/a");
    }
}
