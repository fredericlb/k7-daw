// tasks.rs — the workspace's `TODOS.md` read: a Markdown checklist, each `- [ ] text` line a task, done when `[x]`.
// A reader would get wrong that a task may end with `` `project @ timecode` ``, where in which song it is about: it is not part of the text.

use crate::js_text::is_js_space;

/// Where the tasks live: one file at the workspace root, beside the projects it talks about.
pub const TASKS_FILE: &str = "TODOS.md";

/// One task line as read: its index among the file's lines.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Task {
    pub line: usize,
    pub text: String,
    pub done: bool,
    pub at: Option<(String, String)>,
}

/// `TASK`: `- [ ] rest` with any leading space; a `\r` in the rest fails the match, as JS's `.` does.
fn task_of(raw: &str) -> Option<(bool, &str)> {
    let s = raw.trim_start_matches(is_js_space);
    let s = s.strip_prefix("- [").or_else(|| s.strip_prefix("* ["))?;
    let mut it = s.chars();
    let mark = it.next()?;
    let rest = it.as_str().strip_prefix("] ")?;
    if !matches!(mark, ' ' | 'x' | 'X') || rest.contains(['\r', '\u{2028}', '\u{2029}']) {
        return None;
    }
    Some((mark != ' ', rest))
}

/// `ANCHOR`: a trailing `` `project @ timecode` ``, the text before it.
fn anchor_of(rest: &str) -> Option<(&str, String, String)> {
    let s = rest.trim_end_matches(is_js_space);
    let body = s.strip_suffix('`')?;
    let open = body.rfind('`')?;
    let inner = &body[open + 1..];
    let at = inner.find('@')?;
    let (project, timecode) = (inner[..at].strip_suffix(' ')?, inner[at + 1..].strip_prefix(' ')?);
    if project.is_empty() || timecode.is_empty() {
        return None;
    }
    Some((&body[..open], project.trim().to_owned(), timecode.trim().to_owned()))
}

/// Every task line of the file, in order.
#[must_use]
pub fn parse_tasks(text: &str) -> Vec<Task> {
    text.split('\n')
        .enumerate()
        .filter_map(|(line, raw)| {
            let (done, rest) = task_of(raw)?;
            Some(match anchor_of(rest) {
                Some((before, p, t)) => Task {
                    line,
                    text: before.trim().to_owned(),
                    done,
                    at: Some((p, t)),
                },
                None => Task {
                    line,
                    text: rest.trim().to_owned(),
                    done,
                    at: None,
                },
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_task_line_reads_its_anchor() {
        let t = parse_tasks("# T\n- [ ] fix `song @ 49:1`\n  * [x] done\n- [y] no\n- [ ] cr\r");
        assert_eq!(t.len(), 2);
        assert_eq!((t[0].line, t[0].text.as_str()), (1, "fix"));
        assert_eq!(t[0].at, Some(("song".to_owned(), "49:1".to_owned())));
        assert!(t[1].done && t[1].at.is_none());
        assert!(anchor_of("x ` @ 1`").is_none());
    }
}
