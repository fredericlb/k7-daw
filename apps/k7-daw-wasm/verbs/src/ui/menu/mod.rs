// menu/mod.rs — a right-click's rows: every entry is a command line, derived from the catalogue by what the clicked thing can answer.
// A reader would get wrong that a row's words are here: a hand-named row carries a message key, which the page turns into words in its language.

mod cases;
mod ctx;
mod node;
mod tabs;
mod title;

pub use ctx::{Cc, LoopMode, MenuCtx, NoteRegion, Sel};
pub use node::{NodeMenuTarget, node_menu};
pub use tabs::{TabGroup, TabRow, danger_split, is_destructive, menu_tab, menu_tabs, move_focus};
pub use title::{MenuTarget, NameSpec, menu_target};

use crate::palette::CommandInfo;
use crate::ui::prefs::{Prefs, in_menu};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Words the page resolves: a literal, or a message of its catalogue with its parameters, after an optional literal prefix.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Text {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub prefix: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(type = "Record<string, string | number>", optional))]
    pub params: Option<Map<String, Value>>,
}

impl Text {
    #[must_use]
    pub fn plain(s: impl Into<String>) -> Self {
        Text {
            text: Some(s.into()),
            ..Text::default()
        }
    }

    /// A message of the page's catalogue.
    #[must_use]
    pub fn msg(key: &str) -> Self {
        Text {
            key: Some(key.to_owned()),
            ..Text::default()
        }
    }

    /// A message with one parameter.
    #[must_use]
    pub fn msg_with(key: &str, name: &str, value: impl Into<Value>) -> Self {
        let mut params = Map::new();
        params.insert(name.to_owned(), value.into());
        Text {
            key: Some(key.to_owned()),
            params: Some(params),
            ..Text::default()
        }
    }

    #[must_use]
    pub fn with_prefix(mut self, prefix: &str) -> Self {
        self.prefix = prefix.to_owned();
        self
    }

    /// What tells two texts apart when the page has not resolved them.
    fn identity(&self) -> String {
        format!(
            "{}|{}|{}|{}",
            self.prefix,
            self.text.as_deref().unwrap_or(""),
            self.key.as_deref().unwrap_or(""),
            self.params
                .as_ref()
                .map_or_else(String::new, |p| Value::Object(p.clone()).to_string())
        )
    }
}

/// A displayable menu item: what the command is called, what it does, and the line it stands for.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Item {
    /// The command's name minus the word that names what was clicked, or the row's own name.
    pub label: Text,
    /// What it does, as one sentence in the language being read.
    pub description: Text,
    pub command: String,
    /// The host runs it as it stands, rather than sending it to the bar for an argument.
    pub complete: bool,
    /// Ask before running it.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub confirm: Option<bool>,
    /// The verb this entry stands for, untranslated: what the prefs are keyed by.
    pub verb: String,
    /// The catalogue's sentence in the language being read, when the row has no name of its own.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub sentence: Option<String>,
}

/// A canonical entry before it becomes a row.
#[derive(Clone, Debug, Default)]
pub(crate) struct Entry {
    pub verb: String,
    pub arg: Option<String>,
    pub needs_more: bool,
    pub help: Option<Text>,
    pub name: Option<Text>,
}

impl Entry {
    pub(crate) fn new(verb: &str) -> Self {
        Entry {
            verb: verb.to_owned(),
            ..Entry::default()
        }
    }

    pub(crate) fn arg(mut self, arg: impl Into<String>) -> Self {
        self.arg = Some(arg.into());
        self
    }

    pub(crate) fn more(mut self) -> Self {
        self.needs_more = true;
        self
    }

    pub(crate) fn named(mut self, name: Text) -> Self {
        self.name = Some(name);
        self
    }

    pub(crate) fn help(mut self, help: Text) -> Self {
        self.help = Some(help);
        self
    }
}

/// An argument that must survive as one token: the dispatcher splits on whitespace, so anything with a space is quoted and nothing else is.
pub(crate) fn quote_arg(s: &str) -> String {
    if s.chars().any(song_edit::js_text::is_js_space) {
        serde_json::to_string(s).unwrap_or_else(|_| s.to_owned())
    } else {
        s.to_owned()
    }
}

/// The catalogue's help line as the sentence a row reports: the `(e.g. ...)` tail dropped, every other parenthetical kept.
fn readable_help(help: &str) -> String {
    let mut out = String::new();
    let mut rest = help;
    while let Some(at) = rest.find("(e.g.") {
        match rest[at..].find(')') {
            Some(close) => {
                out.push_str(rest[..at].trim_end_matches(song_edit::js_text::is_js_space));
                rest = &rest[at + close + 1..];
            }
            None => break,
        }
    }
    out.push_str(rest);
    song_edit::js_text::js_trim(&out).to_owned()
}

fn short_name(name: &str, head: &str) -> String {
    match name.strip_prefix(head).and_then(|r| r.strip_prefix(' ')) {
        Some(rest) if !head.is_empty() => rest.to_owned(),
        _ => name.to_owned(),
    }
}

/// The one place every entry passes through, so "not on the right-click" is applied once and cannot be forgotten by a new case.
pub(crate) fn build(
    commands: &[CommandInfo],
    entries: Vec<Entry>,
    prefs: &Prefs,
    head: &str,
    locale: &str,
    entity: Option<&str>,
    also: &[&str],
) -> Vec<Item> {
    let find = |verb: &str| commands.iter().find(|c| c.command == verb);
    let kept: Vec<Entry> = entries
        .into_iter()
        .filter(|e| {
            let placed = find(&e.verb).and_then(|c| c.menu.as_deref());
            std::iter::once(entity)
                .chain(also.iter().map(|a| Some(*a)))
                .any(|en| in_menu(&e.verb, prefs, en, placed))
        })
        .collect();
    let full: Vec<Text> = kept
        .iter()
        .map(|e| e.name.clone().unwrap_or_else(|| Text::plain(e.verb.clone())))
        .collect();
    let short: Vec<Text> = kept
        .iter()
        .zip(&full)
        .map(|(e, n)| {
            if e.name.is_some() {
                n.clone()
            } else {
                Text::plain(short_name(&e.verb, head))
            }
        })
        .collect();
    // All or nothing: when shortening would make a pair indistinguishable, nothing is dropped.
    let ids: std::collections::HashSet<String> = short.iter().map(Text::identity).collect();
    let distinct = ids.len() == short.len();
    kept.iter()
        .enumerate()
        .map(|(i, e)| {
            let line = match &e.arg {
                Some(a) if !a.is_empty() => format!("{} {a}", e.verb),
                _ => e.verb.clone(),
            };
            let c = find(&e.verb);
            let readable = c.and_then(|c| c.readable.as_ref()).map(|r| match locale {
                "en" => r.en.as_str(),
                "fr" => r.fr.as_str(),
                _ => "",
            });
            let sentence = (e.name.is_none() && e.help.is_none())
                .then(|| readable.filter(|s| !s.is_empty()))
                .flatten()
                .map(str::to_owned);
            let description = match &e.help {
                Some(h) => h.clone(),
                None => Text::plain(match readable.filter(|s| !s.is_empty()) {
                    Some(r) => r.to_owned(),
                    None => readable_help(c.and_then(|c| c.help.as_deref()).unwrap_or("")),
                }),
            };
            let mut command = song_edit::js_text::js_trim(&line).to_owned();
            if e.needs_more {
                command.push(' ');
            }
            Item {
                label: if distinct { short[i].clone() } else { full[i].clone() },
                description,
                command,
                complete: !e.needs_more,
                confirm: c.and_then(|c| c.confirm).filter(|x| *x),
                verb: e.verb.clone(),
                sentence,
            }
        })
        .collect()
}

/// The menu for a context; empty means "do not open a menu here".
#[must_use]
pub fn context_menu(ctx: &MenuCtx, commands: &[CommandInfo], prefs: &Prefs, locale: &str) -> Vec<Item> {
    cases::menu(ctx, commands, prefs, locale)
}

/// A module the engine provides has no file anywhere, and the `@engine/` prefix is exactly what says so.
#[must_use]
pub fn with_ambient(ctx: MenuCtx) -> MenuCtx {
    match ctx {
        MenuCtx::Module {
            id,
            missing,
            model,
            native,
            rack,
            ambient: _,
        } if id.starts_with("@engine/") => MenuCtx::Module {
            id,
            missing,
            ambient: Some(true),
            model,
            native,
            rack,
        },
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_example_tail_goes_and_other_parentheses_stay() {
        assert_eq!(
            readable_help("set a track's gain in dB on its mixer strip (e.g. track gain melody -6)"),
            "set a track's gain in dB on its mixer strip"
        );
        assert_eq!(
            readable_help("toggle a track's (or group's) mute"),
            "toggle a track's (or group's) mute"
        );
        assert_eq!(readable_help("a (e.g. x) b (e.g. y)"), "a b");
        assert_eq!(readable_help("open (e.g. never closed"), "open (e.g. never closed");
        assert_eq!(readable_help(""), "");
    }

    #[test]
    fn an_argument_with_a_space_is_one_quoted_token() {
        assert_eq!(quote_arg("Filter Freq"), "\"Filter Freq\"");
        assert_eq!(quote_arg("Cutoff"), "Cutoff");
    }
}
