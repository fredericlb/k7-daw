// warnings.rs — what an engine warning is about, read off the C++ engine's prose, and which warnings mean the project plays without something it names.
// A reader would get wrong that the engine sends no code: each kind is recognised by the stable start of its text, as the regexes it replaces were.

use serde::{Deserialize, Serialize};

/// What a list of duplicated ids held.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum Duplicated {
    Clips,
    Tracks,
    Modules,
    Entries,
}

/// What a warning is about; `unknown` keeps the raw words with the page.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum WarningKind {
    DuplicateId { what: Duplicated },
    TimeSignature { signature: String },
    Unreadable { file: String },
    ClipMissing { file: String },
    RecordFolder,
    PluginFailed { name: String },
    AutomationParam { param: String },
    Hung,
    Internal,
    Unknown,
}

/// Why the engine plays without a thing: refused by its folders, not there, or not installed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum Why {
    Refused,
    NotFound,
    NotInstalled,
}

/// One thing the project names that the engine is playing without.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct MissingItem {
    /// What it is, in the engine's words: "rack sample", "clip", "plugin".
    pub what: String,
    /// The file's name or the plugin's, never a whole path.
    pub name: String,
    pub why: Why,
}

/// A JavaScript line terminator, which `.` does not match.
fn is_eol(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

/// `(.+)` taken to the end of the text: non-empty, one line.
fn rest_of_line(s: &str) -> Option<&str> {
    (!s.is_empty() && !s.contains(is_eol)).then_some(s)
}

/// The last segment of a path, either slash; the whole path when that is empty.
fn file_name(path: &str) -> String {
    match path.rsplit(['/', '\\']).next() {
        Some(last) if !last.is_empty() => last.to_owned(),
        _ => path.to_owned(),
    }
}

/// Where `.` stops matching in `s`: the first line terminator, or its end.
fn line_end(s: &str) -> usize {
    s.find(is_eol).unwrap_or(s.len())
}

/// `^(.+?) '(.+)' is outside the folders this engine reads`: what it was, and the path.
fn outside(message: &str) -> Option<(&str, &str)> {
    const TAIL: &str = "' is outside the folders this engine reads";
    let eol = line_end(message);
    let start = message.chars().next()?.len_utf8();
    // The lazy head grows to each " '" in turn; the greedy path ends at the last tail on its line.
    for (i, _) in message.get(start..)?.match_indices(" '") {
        let at = start + i;
        if at > eol {
            return None;
        }
        let body = &message[at + 2..];
        let limit = line_end(body);
        let hit = body
            .match_indices(TAIL)
            .map(|(j, _)| j)
            .filter(|&j| j >= 1 && j <= limit)
            .last();
        if let Some(j) = hit {
            return Some((&message[..at], &body[..j]));
        }
    }
    None
}

/// `^could not create plugin '(.+?)'`: the name up to the first quote one char in.
fn plugin_name(message: &str) -> Option<&str> {
    let body = message.strip_prefix("could not create plugin '")?;
    let first = body.chars().next()?;
    let at = first.len_utf8() + body.get(first.len_utf8()..)?.find('\'')?;
    let name = &body[..at];
    (!name.contains(is_eol)).then_some(name)
}

pub fn warning_kind(message: &str) -> WarningKind {
    if let Some(rest) = message.strip_prefix("two ") {
        for (word, what) in [
            ("clips", Duplicated::Clips),
            ("tracks", Duplicated::Tracks),
            ("modules", Duplicated::Modules),
            ("entries", Duplicated::Entries),
        ] {
            if rest.strip_prefix(word).is_some_and(|r| r.starts_with(" share the id ")) {
                return WarningKind::DuplicateId { what };
            }
        }
    }
    if let Some(rest) = message.strip_prefix("time signature ") {
        let token = rest.split(char::is_whitespace).next().unwrap_or("");
        if !token.is_empty() && rest[token.len()..].starts_with(" cannot be played") {
            return WarningKind::TimeSignature {
                signature: token.to_owned(),
            };
        }
    }
    if let Some(path) = message
        .strip_prefix("clip source not found/invalid: ")
        .and_then(rest_of_line)
    {
        return WarningKind::ClipMissing { file: file_name(path) };
    }
    if let Some(rest) = message.strip_prefix("record folder '")
        && rest
            .find("' is outside the folders")
            .is_some_and(|i| i <= line_end(rest))
    {
        return WarningKind::RecordFolder;
    }
    if let Some((_, path)) = outside(message) {
        return WarningKind::Unreadable { file: file_name(path) };
    }
    if let Some(name) = plugin_name(message) {
        return WarningKind::PluginFailed { name: name.to_owned() };
    }
    if let Some(param) = message
        .strip_prefix("automation param not found: ")
        .and_then(rest_of_line)
    {
        return WarningKind::AutomationParam {
            param: param.to_owned(),
        };
    }
    if message.starts_with("the audio engine is hung") {
        return WarningKind::Hung;
    }
    for head in ["background work", "an engine job", "the state tick"] {
        if message.strip_prefix(head).is_some_and(|r| r.starts_with(" failed")) {
            return WarningKind::Internal;
        }
    }
    WarningKind::Unknown
}

// An external plugin's type is only its format, so its own name says which one is missing.
fn plugin_label(name: &str, kind: &str) -> String {
    let format = ["vst3", "vst", "au", "clap", "lv2"].contains(&kind.to_lowercase().as_str());
    if format && !name.is_empty() { name } else { kind }.to_owned()
}

/// `(.*)` then `sep` then `(.+)` then `tail`, greedy both: the latest separator whose body ends at the tail's last occurrence.
fn split_greedy<'a>(body: &'a str, sep: &str, tail: &str) -> Option<(&'a str, &'a str)> {
    for (at, _) in body.match_indices(sep).collect::<Vec<_>>().into_iter().rev() {
        let (head, rest) = (&body[..at], &body[at + sep.len()..]);
        if head.contains(is_eol) {
            continue;
        }
        let limit = line_end(rest);
        let hit = rest
            .match_indices(tail)
            .map(|(i, _)| i)
            .filter(|&i| i >= 1 && i <= limit)
            .last();
        if let Some(i) = hit {
            return Some((head, &rest[..i]));
        }
    }
    None
}

/// The missing thing an engine warning reports, or none when the warning is about something else.
pub fn missing_of(message: &str) -> Option<MissingItem> {
    if let Some((what, path)) = outside(message) {
        return Some(MissingItem {
            what: what.to_owned(),
            name: file_name(path),
            why: Why::Refused,
        });
    }
    if let Some(path) = message
        .strip_prefix("clip source not found/invalid: ")
        .and_then(rest_of_line)
    {
        return Some(MissingItem {
            what: "clip".into(),
            name: file_name(path),
            why: Why::NotFound,
        });
    }
    if let Some(body) = message.strip_prefix("rack sound '")
        && let Some((_, path)) = split_greedy(body, "' -> '", "' is missing or unreadable")
    {
        return Some(MissingItem {
            what: "rack sample".into(),
            name: file_name(path),
            why: Why::NotFound,
        });
    }
    if let Some(body) = message.strip_prefix("plugin '")
        && let Some((name, kind)) = split_greedy(body, "' (", ") is not in the cache")
    {
        return Some(MissingItem {
            what: "plugin".into(),
            name: plugin_label(name, kind),
            why: Why::NotInstalled,
        });
    }
    if let Some(body) = message.strip_prefix("could not create plugin '") {
        for (at, _) in body.match_indices("' (").collect::<Vec<_>>().into_iter().rev() {
            let (name, rest) = (&body[..at], &body[at + 3..]);
            if let Some(kind) = rest.strip_suffix(')').and_then(rest_of_line)
                && !name.contains(is_eol)
            {
                return Some(MissingItem {
                    what: "plugin".into(),
                    name: plugin_label(name, kind),
                    why: Why::NotInstalled,
                });
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_engines_own_texts() {
        let k = warning_kind;
        assert_eq!(
            k("two clips share the id 'track-1_1.1@16' — the second is 'track-1_1.1@16#2'"),
            WarningKind::DuplicateId {
                what: Duplicated::Clips
            }
        );
        assert_eq!(
            k("two modules share the id 'eq' — the second is 'eq#2'"),
            WarningKind::DuplicateId {
                what: Duplicated::Modules
            }
        );
        assert_eq!(
            k("time signature 5/6 cannot be played — using 4/4"),
            WarningKind::TimeSignature {
                signature: "5/6".into()
            }
        );
        assert_eq!(
            k("clip source not found/invalid: C:\\songs\\a\\take.wav"),
            WarningKind::ClipMissing {
                file: "take.wav".into()
            }
        );
        assert_eq!(
            k("clip source '/home/x/b.wav' is outside the folders this engine reads — ignored"),
            WarningKind::Unreadable { file: "b.wav".into() }
        );
        assert_eq!(
            k("record folder '/tmp/r' is outside the folders this engine writes — recording is off"),
            WarningKind::RecordFolder
        );
        assert_eq!(
            k("could not create plugin 'Diva' (vst3): no"),
            WarningKind::PluginFailed { name: "Diva".into() }
        );
        assert_eq!(
            k("the audio engine is hung inside a plugin — restart the app"),
            WarningKind::Hung
        );
        assert_eq!(k("an engine job failed: boom"), WarningKind::Internal);
        assert_eq!(
            k("automation param not found: cutoff"),
            WarningKind::AutomationParam { param: "cutoff".into() }
        );
        assert_eq!(k("something new"), WarningKind::Unknown);
    }

    #[test]
    fn leaves_a_clip_source_with_a_second_line_unrecognised() {
        assert_eq!(
            warning_kind("clip source not found/invalid: a\nb"),
            WarningKind::Unknown
        );
    }

    fn item(what: &str, name: &str, why: Why) -> Option<MissingItem> {
        Some(MissingItem {
            what: what.into(),
            name: name.into(),
            why,
        })
    }

    #[test]
    fn reads_a_file_the_engine_refused_by_its_name_alone() {
        assert_eq!(
            missing_of(
                "rack sample 'C:\\Users\\me\\libraries\\k7-core\\1\\samples\\Leviathan4\\Kick_C#_03.wav' is outside the folders this engine reads — ignored"
            ),
            item("rack sample", "Kick_C#_03.wav", Why::Refused)
        );
        assert_eq!(
            missing_of(
                "surge patch '/srv/libs/surge-xt/base/Basses/E-Bass.fxp' is outside the folders this engine reads — ignored"
            ),
            item("surge patch", "E-Bass.fxp", Why::Refused)
        );
    }

    #[test]
    fn reads_a_file_that_is_not_there() {
        assert_eq!(
            missing_of("clip source not found/invalid: /a/b/take.wav"),
            item("clip", "take.wav", Why::NotFound)
        );
        assert_eq!(
            missing_of("rack sound 'KICK' -> '/a/BD.wav' is missing or unreadable; skipped"),
            item("rack sample", "BD.wav", Why::NotFound)
        );
    }

    #[test]
    fn reads_a_plugin_the_machine_does_not_have() {
        assert_eq!(
            missing_of("plugin 'abc' (Surge XT) is not in the cache — run 'k7 engine scan'"),
            item("plugin", "Surge XT", Why::NotInstalled)
        );
        assert_eq!(
            missing_of("could not create plugin 'Diva' (vst3)"),
            item("plugin", "Diva", Why::NotInstalled)
        );
    }

    #[test]
    fn ignores_every_other_warning_preset_saves_included() {
        assert_eq!(
            missing_of(
                "preset save path '/a/x.vstpreset' is not a .vstpreset under the folders this engine writes — not saved"
            ),
            None
        );
        assert_eq!(missing_of("audio device not found: Focusrite"), None);
    }
}
