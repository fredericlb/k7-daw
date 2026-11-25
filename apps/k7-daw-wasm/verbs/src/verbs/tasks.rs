// tasks.rs — the workspace's `TODOS.md`: `task add`, `task mark`, `task done`, `task remove`, after verbs/tasks.ts.
// A reader would get wrong where the file is: the workspace root, never the song's folder; an edit rewrites one line.

use super::shared::{Done, join, position, split_words};
use crate::host::Host;
use song_edit::confine::workspace_dir_of;
use song_edit::js_text::str_to_number;
use song_edit::paths::relative;
use song_edit::workspace_file::{ws_exists, ws_read, ws_write};

pub const TASKS_FILE: &str = "TODOS.md";

/// One task line as read: its index among the file's lines.
struct Task {
    line: usize,
    text: String,
    done: bool,
    at: Option<(String, String)>,
}

/// `TASK`: `- [ ] rest` with any leading space; a `\r` in the rest fails the match, as JS's `.` does.
fn task_of(raw: &str) -> Option<(bool, &str)> {
    let s = raw.trim_start_matches(|c: char| song_edit::js_text::is_js_space(c));
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
    let s = rest.trim_end_matches(|c: char| song_edit::js_text::is_js_space(c));
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

fn parse_tasks(text: &str) -> Vec<Task> {
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

fn task_line(text: &str, done: bool, at: Option<&(String, String)>) -> String {
    let text = split_words(text).join(" ");
    let at = at.map_or(String::new(), |(p, t)| format!(" `{p} @ {t}`"));
    format!("- [{}] {text}{at}", if done { "x" } else { " " })
}

/// `addTask`: the new open task appended, a header first when the file is new.
fn add_task(text: &str, task: &str, at: Option<&(String, String)>) -> String {
    let body = text.trim_end();
    let line = task_line(task, false, at);
    if body.is_empty() {
        format!("# Tasks\n\n{line}\n")
    } else {
        format!("{body}\n{line}\n")
    }
}

/// Task `n` (1-based) ticked or unticked, or dropped; None when there is no such task.
fn edit_task(text: &str, n: usize, remove: bool) -> Option<String> {
    let t = parse_tasks(text).into_iter().nth(n.checked_sub(1)?)?;
    let mut lines: Vec<String> = text.split('\n').map(str::to_owned).collect();
    if remove {
        lines.remove(t.line);
    } else {
        lines[t.line] = task_line(&t.text, !t.done, t.at.as_ref());
    }
    Some(lines.join("\n"))
}

fn tasks_file(h: &Host) -> String {
    join(&workspace_dir_of(&h.env, &h.song_path), TASKS_FILE)
}

fn read(h: &Host, abs: &str) -> Result<String, song_edit::Error> {
    if ws_exists(&*h.fs, abs) {
        ws_read(&*h.fs, abs)
    } else {
        Ok(String::new())
    }
}

fn add(a: &str, h: &mut Host, mark: bool) -> Done {
    let abs = tasks_file(h);
    let mut words = split_words(a);
    let timecode = if mark && !words.is_empty() {
        Some(words.remove(0))
    } else {
        None
    };
    let text = words.join(" ");
    if text.is_empty() {
        h.refuse([if mark {
            "usage: task mark <timecode> <text>  (e.g. task mark 49:1 fix the snare)".to_owned()
        } else {
            "usage: task add <text>  (e.g. task add bounce the bass)".to_owned()
        }]);
        return Ok(());
    }
    let mut at = None;
    if let Some(tc) = timecode {
        let p = h.project()?;
        if let Err(m) = position(tc, &p) {
            h.refuse([format!("bad timecode: {m}")]);
            return Ok(());
        }
        let rel = relative(&workspace_dir_of(&h.env, &h.song_path), &h.song_path).replace('\\', "/");
        at = Some((if rel.is_empty() { ".".to_owned() } else { rel }, tc.to_owned()));
    }
    let before = read(h, &abs)?;
    ws_write(h.fs, &abs, add_task(&before, &text, at.as_ref()).as_bytes())?;
    let place = at.as_ref().map_or(String::new(), |(p, t)| format!(" at {p} {t}"));
    h.log([format!("task added{place}: {text}")]);
    Ok(())
}

fn edit(a: &str, h: &mut Host, remove: bool) -> Done {
    let abs = tasks_file(h);
    let op = if remove { "remove" } else { "done" };
    let n = str_to_number(split_words(a).first().copied().unwrap_or(""));
    if n.fract() != 0.0 || !n.is_finite() || n < 1.0 || split_words(a).is_empty() {
        h.refuse([format!(
            "usage: task {op} <number>  (the task's place in the list, from 1)"
        )]);
        return Ok(());
    }
    let before = read(h, &abs)?;
    let Some(after) = edit_task(&before, n as usize, remove) else {
        h.refuse([format!("no task {} in {TASKS_FILE}", k7x::edit::js_number(n))]);
        return Ok(());
    };
    ws_write(h.fs, &abs, after.as_bytes())?;
    let n = k7x::edit::js_number(n);
    h.log([if remove {
        format!("task {n} removed")
    } else {
        format!("task {n} ticked or unticked")
    }]);
    Ok(())
}

pub fn task_add(a: &str, h: &mut Host) -> Done {
    add(a, h, false)
}

pub fn task_mark(a: &str, h: &mut Host) -> Done {
    add(a, h, true)
}

pub fn task_done(a: &str, h: &mut Host) -> Done {
    edit(a, h, false)
}

pub fn task_remove(a: &str, h: &mut Host) -> Done {
    edit(a, h, true)
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
