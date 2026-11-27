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

    fn root(remote: &str, name: &str) -> Root {
        Root {
            remote: remote.into(),
            name: name.into(),
            writable: true,
            per_child: None,
        }
    }

    #[test]
    fn joins_below_a_root_with_a_slash_the_root_kept_as_the_server_names_it() {
        let ws = r"C:\k7\workspaces\me\workspace";
        let renders = r"C:\k7\workspaces\me\.renders";
        let roots = [root(ws, "ws-me"), root(renders, "renders-me")];
        let spell = |p: &str| {
            let out = tunnel_spelling(&serde_json::to_string(p).unwrap(), &roots, '\\');
            serde_json::from_str::<String>(&out).unwrap()
        };
        assert_eq!(
            spell(&format!(r"{renders}\ab\cd.wav")),
            format!("{renders}/ab/cd.wav")
        );
        // Outside every root, the root itself and an already-spelt path stay as they are.
        assert_eq!(spell(r"C:\elsewhere\x.wav"), r"C:\elsewhere\x.wav");
        assert_eq!(spell(ws), ws);
        assert_eq!(spell(&format!("{ws}/a/b.wav")), format!("{ws}/a/b.wav"));
    }

    #[test]
    fn a_posix_server_changes_nothing() {
        let roots = [root("/srv/ws", "ws")];
        let job = r#""/srv/ws/a/b.wav""#;
        assert_eq!(tunnel_spelling(job, &roots, '/'), job);
    }

    #[test]
    fn rewrites_every_path_of_a_render_job_nested_ones_included() {
        let ws = r"C:\k7\workspaces\me\workspace";
        let renders = r"C:\k7\workspaces\me\.renders";
        let roots = [root(ws, "ws-me"), root(renders, "renders-me")];
        let job = serde_json::json!({
            "type": "render",
            "out": format!(r"{renders}\f.wav"),
            "doc": { "tracks": [{ "clips": [{ "source": format!(r"{ws}\song\clips\a.wav") }] }], "bpm": 110 },
        });
        let out: serde_json::Value =
            serde_json::from_str(&tunnel_spelling(&job.to_string(), &roots, '\\')).unwrap();
        assert_eq!(
            out,
            serde_json::json!({
                "type": "render",
                "out": format!("{renders}/f.wav"),
                "doc": { "tracks": [{ "clips": [{ "source": format!("{ws}/song/clips/a.wav") }] }], "bpm": 110 },
            })
        );
    }

    #[test]
    fn names_a_path_by_the_nearer_of_two_nested_roots() {
        let roots = [root(r"C:\a", "outer"), root(r"C:\a\b", "inner")];
        assert_eq!(
            tunnel_spelling(r#""C:\\a\\b\\c.wav""#, &roots, '\\'),
            r#""C:\\a\\b/c.wav""#
        );
    }
}
