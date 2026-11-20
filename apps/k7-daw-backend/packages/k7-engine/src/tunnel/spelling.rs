//! spelling.rs — a job frame's paths put in the tunnel's spelling before it reaches the machine, as
//! `jobSpelling.ts` does it: a root as the server names it, then `/` below it. The one thing a reader
//! would get wrong: on a `/` server this changes nothing, the frame is not even re-serialized.

use serde_json::Value;

use super::frames::Root;
use crate::js;

/// `path` in the tunnel's spelling when it lies under one of `roots` in the server's, else as it was.
fn tunnel_path(path: &str, roots: &[&Root], sep: char) -> String {
    for r in roots {
        let head = format!("{}{sep}", r.remote);
        if let Some(rest) = path.strip_prefix(&head) {
            return format!(
                "{}/{}",
                r.remote,
                rest.split(sep).collect::<Vec<_>>().join("/")
            );
        }
    }
    path.to_string()
}

/// Every string of a JSON frame through `tunnel_path`; a frame that is not JSON passes untouched.
pub fn tunnel_spelling(text: &str, roots: &[Root], sep: char) -> String {
    if sep == '/' || roots.is_empty() {
        return text.to_string();
    }
    let Ok(frame) = serde_json::from_str::<Value>(text) else {
        return text.to_string();
    };
    let mut by_length: Vec<&Root> = roots.iter().collect();
    // The longest root first, so a root inside another names the path by the nearer one.
    by_length.sort_by_key(|r| std::cmp::Reverse(js::utf16_len(&r.remote)));
    fn walk(v: &Value, roots: &[&Root], sep: char) -> Value {
        match v {
            Value::String(s) => Value::String(tunnel_path(s, roots, sep)),
            Value::Array(a) => Value::Array(a.iter().map(|x| walk(x, roots, sep)).collect()),
            Value::Object(o) => Value::Object(
                o.iter()
                    .map(|(k, x)| (k.clone(), walk(x, roots, sep)))
                    .collect(),
            ),
            other => other.clone(),
        }
    }
    js::stringify(&walk(&frame, &by_length, sep))
}

/// This server's separator.
pub const SEP: char = if cfg!(windows) { '\\' } else { '/' };

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn respells_a_windows_path_under_a_root_and_nothing_else() {
        let roots = [
            Root {
                remote: r"C:\srv".into(),
                name: "a".into(),
                writable: true,
                per_child: None,
            },
            Root {
                remote: r"C:\srv\lib".into(),
                name: "b".into(),
                writable: true,
                per_child: None,
            },
        ];
        let out = tunnel_spelling(
            r#"{"src":"C:\\srv\\lib\\x\\y.wav","n":1.0,"o":"D:\\x"}"#,
            &roots,
            '\\',
        );
        assert_eq!(out, r#"{"src":"C:\\srv\\lib/x/y.wav","n":1,"o":"D:\\x"}"#);
        assert_eq!(tunnel_spelling("{\"a\":1.0}", &roots, '/'), "{\"a\":1.0}");
        assert_eq!(tunnel_spelling("not json", &roots, '\\'), "not json");
    }
}
