// complete — the command line's completion: what a line may continue with, after `completionItems` and its providers.
// A reader would get wrong that an item is a whole LINE: the typed prefix, then the value, so a caller replaces its field with `line`.

mod ctx;
mod files;
pub mod provider_info;
mod providers;

use crate::catalogue::{ArgSpec, Spec, specs};
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use song::Cache;
use song_edit::Fs;
use song_edit::confine::Env;
use song_edit::facade::env_from_json;
use song_edit::js_text::is_js_space;

pub use ctx::Ctx;

/// One completion: the line it completes to, and what a list shows for its value when that is not the value.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    pub line: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CompleteRequest {
    /// The command line as typed.
    pub line: String,
    /// The caret, in UTF-16 units as a browser's `selectionStart` counts; what follows it is not read. Absent: the line's end.
    #[serde(default)]
    pub cursor: Option<usize>,
    /// The project folder, in the `Fs`'s spelling.
    pub project: String,
    /// `{studioHome, metaRoot, ...}`, as `verb.run` takes it.
    #[serde(default)]
    pub env: Json,
}

/// The line up to `cursor` UTF-16 units.
fn head_of(line: &str, cursor: Option<usize>) -> &str {
    let Some(mut left) = cursor else { return line };
    for (at, c) in line.char_indices() {
        if left < c.len_utf16() {
            return &line[..at];
        }
        left -= c.len_utf16();
    }
    line
}

/// `s.split(/\s+/)`: a run of space is one cut, and a leading or trailing one leaves an empty word at that end.
fn split_runs(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut from = 0;
    let mut chars = s.char_indices().peekable();
    while let Some((at, c)) = chars.next() {
        if !is_js_space(c) {
            continue;
        }
        out.push(&s[from..at]);
        let mut end = at + c.len_utf8();
        while let Some(&(next, d)) = chars.peek() {
            if !is_js_space(d) {
                break;
            }
            end = next + d.len_utf8();
            chars.next();
        }
        from = end;
    }
    out.push(&s[from..]);
    out
}

/// The command a typed line runs, by the longest one it starts with: case never decides, and a space must follow.
fn command_of(sl: &str) -> Option<&'static Spec> {
    specs()
        .iter()
        .filter(|c| sl.strip_prefix(c.command.as_str()).is_some_and(|r| r.starts_with(' ')))
        .max_by_key(|c| c.command.len())
}

/// The candidate values of one argument: its literal set, else its provider asked, else none.
fn values_of(arg: &ArgSpec, ctx: &mut Ctx) -> (Vec<String>, Vec<(String, String)>) {
    if let Some(v) = &arg.values {
        return (v.clone(), Vec::new());
    }
    match arg.complete.as_deref() {
        Some(name) => providers::ask(name, ctx),
        None => (Vec::new(), Vec::new()),
    }
}

/// What `line` may continue with: the command words while one is being typed, then the values of the argument under the caret.
pub fn complete(line: &str, ctx: &mut Ctx) -> Vec<Item> {
    let s = line.trim_start_matches(is_js_space);
    let sl = s.to_lowercase();
    let Some(spec) = command_of(&sl) else {
        return specs()
            .iter()
            .filter(|c| c.command.starts_with(&sl) && c.command != sl)
            .map(|c| Item {
                line: c.command.clone(),
                label: None,
            })
            .collect();
    };
    let words = split_runs(s.get(spec.command.len() + 1..).unwrap_or(""));
    let at = words.len() - 1;
    let Some(arg) = spec.args.get(at) else {
        return Vec::new();
    };
    let (values, labels) = values_of(arg, ctx);
    let partial = words[at].to_lowercase();
    let mut prefix = vec![spec.command.as_str()];
    prefix.extend(&words[..at]);
    let prefix = prefix.join(" ");
    values
        .into_iter()
        .filter(|v| v.to_lowercase().starts_with(&partial))
        .map(|v| Item {
            label: labels.iter().find(|(k, _)| *k == v).map(|(_, l)| l.clone()),
            line: format!("{prefix} {v}"),
        })
        .collect()
}

/// `complete` over `fs` with the roots `env`, for a caller that holds an `Env` and a compile cache from key to key.
pub fn complete_in_env(fs: &dyn Fs, cache: &mut Cache, env: Env, project: &str, line: &str) -> Vec<Item> {
    let mut ctx = Ctx::new(fs, cache, env, project);
    complete(line, &mut ctx)
}

/// `complete_in_env` as a request: the line cut at the caret, the roots read off the JSON.
pub fn complete_request(fs: &dyn Fs, cache: &mut Cache, req: &CompleteRequest) -> Vec<Item> {
    let head = head_of(&req.line, req.cursor);
    complete_in_env(fs, cache, env_from_json(&req.env), &req.project, head)
}
