// ai_script.rs — a widgets.js read for its SYNTAX alone: strings, comments, templates, regexes and brackets must close; nothing in it runs.
// A reader would get wrong that this is no JavaScript parser: it catches what a cut-off or a stray bracket does, and the page compiles the script for real where it lands.

use crate::files::scalar::js_space;

/// What may precede a `/` that starts a regex rather than divides.
const REGEX_AFTER: &str = "(,=:[!&|?{};+-*%<>~^";
const REGEX_KEYWORDS: [&str; 8] = ["return", "typeof", "case", "in", "of", "delete", "void", "throw"];

fn export_default(s: &[char], start: usize) -> Option<usize> {
    let skip = |mut i: usize| {
        while s.get(i).is_some_and(|c| js_space(*c)) {
            i += 1;
        }
        i
    };
    let word = |i: usize, w: &str| {
        let end = i + w.chars().count();
        (s.get(i..end)? == w.chars().collect::<Vec<_>>().as_slice()).then_some(end)
    };
    let i = word(skip(start), "export")?;
    let j = skip(i);
    (j > i).then_some(())?;
    let k = word(j, "default")?;
    let end = skip(k);
    (end > k).then_some(end)
}

/// `^\s*export\s+default\s+` as a multi-line replacement by `return `, once.
fn without_export(script: &str) -> Vec<char> {
    let s: Vec<char> = script.chars().collect();
    let line_start = |i: usize| i == 0 || matches!(s[i - 1], '\n' | '\r' | '\u{2028}' | '\u{2029}');
    for p in 0..s.len() {
        if line_start(p)
            && let Some(end) = export_default(&s, p)
        {
            let mut out: Vec<char> = s[..p].to_vec();
            out.extend("return ".chars());
            out.extend(&s[end..]);
            return out;
        }
    }
    s
}

/// Reads template text from `i` (after a backtick or a `}`): Ok(true) when it ended at a `${`, Ok(false) at its closing backtick.
fn template(s: &[char], i: &mut usize) -> Result<bool, &'static str> {
    while *i < s.len() {
        match s[*i] {
            '\\' => *i += 2,
            '`' => {
                *i += 1;
                return Ok(false);
            }
            '$' if s.get(*i + 1) == Some(&'{') => {
                *i += 2;
                return Ok(true);
            }
            _ => *i += 1,
        }
    }
    Err("Unterminated template literal")
}

fn quoted(s: &[char], i: &mut usize, q: char) -> Result<(), &'static str> {
    *i += 1;
    while *i < s.len() {
        match s[*i] {
            '\\' => *i += 2,
            c if c == q => {
                *i += 1;
                return Ok(());
            }
            '\n' => break,
            _ => *i += 1,
        }
    }
    Err("Invalid or unexpected token")
}

fn regex(s: &[char], i: &mut usize) -> Result<(), &'static str> {
    *i += 1;
    let mut class = false;
    while *i < s.len() {
        match s[*i] {
            '\\' => *i += 1,
            '[' => class = true,
            ']' => class = false,
            '/' if !class => {
                *i += 1;
                while s.get(*i).is_some_and(|c| c.is_ascii_alphabetic()) {
                    *i += 1;
                }
                return Ok(());
            }
            '\n' => break,
            _ => {}
        }
        *i += 1;
    }
    Err("Invalid regular expression: missing /")
}

fn is_id(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}

fn check(s: &[char]) -> Result<(), String> {
    let mut stack: Vec<char> = Vec::new();
    let (mut i, mut prev, mut word) = (0, '\0', String::new());
    while i < s.len() {
        let c = s[i];
        let here = i;
        match c {
            '"' | '\'' => quoted(s, &mut i, c)?,
            '`' => {
                i += 1;
                if template(s, &mut i)? {
                    stack.push('$');
                }
            }
            '/' if s.get(i + 1) == Some(&'/') => {
                while i < s.len() && s[i] != '\n' {
                    i += 1;
                }
                continue;
            }
            '/' if s.get(i + 1) == Some(&'*') => {
                let close = (i + 2..s.len().saturating_sub(1)).find(|&k| s[k] == '*' && s[k + 1] == '/');
                i = close.ok_or("Invalid or unexpected token")? + 2;
                continue;
            }
            '/' => {
                let after_word = REGEX_KEYWORDS.contains(&word.as_str());
                if prev == '\0' || REGEX_AFTER.contains(prev) || after_word {
                    regex(s, &mut i)?;
                } else {
                    i += 1;
                }
            }
            '(' | '[' | '{' => {
                stack.push(c);
                i += 1;
            }
            ')' | ']' | '}' => {
                let want = match c {
                    ')' => '(',
                    ']' => '[',
                    _ => '{',
                };
                match stack.pop() {
                    Some(open) if open == want => i += 1,
                    Some('$') if c == '}' => {
                        i += 1;
                        if template(s, &mut i)? {
                            stack.push('$');
                        }
                    }
                    _ => return Err(format!("Unexpected token '{c}'")),
                }
            }
            _ => i += 1,
        }
        // A statement `export` is a syntax error in a function body; a property or a key by that name is not.
        if c == 'e' && !(here > 0 && is_id(s[here - 1])) && s[here..].starts_with(&['e', 'x', 'p', 'o', 'r', 't']) {
            let after = s.get(here + 6).copied().unwrap_or(' ');
            let named = s[(here + 6).min(s.len())..].iter().find(|c| !js_space(**c)) == Some(&':');
            if !is_id(after) && prev != '.' && !named {
                return Err("Unexpected token 'export'".into());
            }
        }
        if is_id(c) {
            if here > 0 && is_id(s[here - 1]) {
                word.push(c);
            } else {
                word = c.to_string();
            }
        } else if !js_space(c) {
            word.clear();
        }
        if !js_space(c) {
            prev = if c.is_alphanumeric() || c == '_' || c == '$' || matches!(c, '"' | '\'' | '`') {
                'a'
            } else {
                c
            };
        }
    }
    if stack.is_empty() {
        Ok(())
    } else {
        Err("Unexpected end of input".to_owned())
    }
}

/// The script's syntax problem as a sentence, or None when its strings, comments and brackets close.
pub fn script_syntax_error(script: &str) -> Option<String> {
    check(&without_export(script)).err()
}

#[cfg(test)]
mod tests {
    use super::script_syntax_error as err;

    #[test]
    fn a_script_that_closes_is_fine() {
        assert_eq!(
            err("export default {\n  osc(lcd) { return `a${1 + 2}b`; },\n  'x': (l) => /re[/]g/.test(\"s\"),\n};"),
            None
        );
        assert_eq!(err("// just a comment"), None);
        assert_eq!(err("const a = 4 / 2; /* c */ const b = a / 2;"), None);
    }

    #[test]
    fn a_cut_off_or_stray_script_is_named() {
        assert_eq!(
            err("export default { osc(lcd) {").as_deref(),
            Some("Unexpected end of input")
        );
        assert_eq!(err("export default { a() } }").as_deref(), Some("Unexpected token '}'"));
        assert_eq!(err("const s = 'open").as_deref(), Some("Invalid or unexpected token"));
        assert_eq!(
            err("const t = `open ${1}").as_deref(),
            Some("Unterminated template literal")
        );
        assert_eq!(err("f(]").as_deref(), Some("Unexpected token ']'"));
        assert_eq!(
            err("const t = 1; export default {}").as_deref(),
            Some("Unexpected token 'export'")
        );
        assert_eq!(err("return { export: 1, b: a.export, exports: 2 }"), None);
    }
}
