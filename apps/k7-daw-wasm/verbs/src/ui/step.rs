// step.rs — an undo journal step as a person reads it: the catalogue's sentence for the command it ran, and its subject.
// A reader would get wrong that the subject is named here: the page's namers turn a lane id or a clip ref into its shown name.

use serde::{Deserialize, Serialize};
use song_edit::js_text::is_js_space;
use std::collections::HashMap;

/// What the sentence needs of a command: its words, its sentences by language, and its first argument's provider.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct StepCommand {
    pub command: String,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub readable: Option<HashMap<String, String>>,
    #[serde(default)]
    pub args: Vec<StepArg>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct StepArg {
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub provider: Option<String>,
}

/// What a step's subject is, so the page can name it: a lane, a clip, or a plain word.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum SubjectKind {
    Lane,
    Clip,
    Plain,
}

/// The sentence of a step, and its subject when the line carried one.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Step {
    /// The catalogue's sentence; absent when the label names no command with one, and the label stands as written.
    pub sentence: Option<String>,
    pub subject: Option<Subject>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Subject {
    pub kind: SubjectKind,
    pub value: String,
}

/// The lexer of a journal label: a JSON-quoted token, or a bare run of non-space.
fn tokens(label: &str) -> Vec<String> {
    let chars: Vec<char> = label.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if is_js_space(chars[i]) {
            i += 1;
            continue;
        }
        let end = if chars[i] == '"' { quoted_end(&chars, i) } else { None };
        let end = end.unwrap_or_else(|| {
            let mut j = i;
            while j < chars.len() && !is_js_space(chars[j]) {
                j += 1;
            }
            j
        });
        out.push(chars[i..end].iter().collect());
        i = end;
    }
    out
}

/// Where a quoted token starting at `at` ends, or none when it never closes.
fn quoted_end(chars: &[char], at: usize) -> Option<usize> {
    let mut i = at + 1;
    while i < chars.len() {
        match chars[i] {
            '"' => return Some(i + 1),
            '\\' => {
                // An escape takes one char that is not a line terminator, as the page's pattern did.
                let next = *chars.get(i + 1)?;
                if matches!(next, '\n' | '\r' | '\u{2028}' | '\u{2029}') {
                    return None;
                }
                i += 2;
            }
            _ => i += 1,
        }
    }
    None
}

fn unquote(t: &str) -> String {
    if !t.starts_with('"') {
        return t.to_owned();
    }
    serde_json::from_str::<String>(t).unwrap_or_else(|_| t.to_owned())
}

/// The catalogue command a line runs: the one whose words begin it, the longest winning.
fn command_of<'a>(tokens: &[String], commands: &'a [StepCommand]) -> Option<&'a StepCommand> {
    let mut best = None;
    let mut best_len = 0;
    for c in commands {
        let words: Vec<&str> = c.command.split(' ').collect();
        if words.len() <= best_len || words.len() > tokens.len() {
            continue;
        }
        if words.iter().enumerate().all(|(i, w)| tokens[i] == *w) {
            best = Some(c);
            best_len = words.len();
        }
    }
    best
}

/// A journal step's label as a sentence and its subject: the label as written when it names no command with a sentence.
#[must_use]
pub fn step_sentence(label: &str, commands: &[StepCommand], locale: &str) -> Step {
    let toks: Vec<String> = tokens(label).iter().map(|t| unquote(t)).collect();
    let none = Step {
        sentence: None,
        subject: None,
    };
    let Some(c) = command_of(&toks, commands) else {
        return none;
    };
    let sentence = c
        .readable
        .as_ref()
        .and_then(|r| r.get(locale).filter(|s| !s.is_empty()).or_else(|| r.get("en")))
        .filter(|s| !s.is_empty());
    let Some(sentence) = sentence else { return none };
    let value = toks.get(c.command.split(' ').count()).filter(|v| !v.is_empty());
    let subject = value.map(|v| {
        let kind = match c.args.first().and_then(|a| a.provider.as_deref()) {
            Some("tracks" | "pictureLanes") => SubjectKind::Lane,
            Some("sequenceFiles" | "clipTargets") => SubjectKind::Clip,
            _ => SubjectKind::Plain,
        };
        Subject { kind, value: v.clone() }
    });
    Step {
        sentence: Some(sentence.clone()),
        subject,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(command: &str, en: Option<&str>, fr: Option<&str>, provider: Option<&str>) -> StepCommand {
        let mut r = HashMap::new();
        if let Some(e) = en {
            r.insert("en".to_owned(), e.to_owned());
        }
        if let Some(f) = fr {
            r.insert("fr".to_owned(), f.to_owned());
        }
        StepCommand {
            command: command.into(),
            readable: (!r.is_empty()).then_some(r),
            args: vec![StepArg {
                provider: provider.map(Into::into),
            }],
        }
    }

    fn catalogue() -> Vec<StepCommand> {
        vec![
            cmd(
                "track mute",
                Some("Mute or unmute a track"),
                Some("Couper une piste"),
                Some("tracks"),
            ),
            cmd("track", Some("Any track"), None, None),
            cmd("clip process", Some("Process a clip"), None, Some("clipTargets")),
            cmd("project tempo", Some("Set the tempo"), None, None),
            cmd("silent verb", None, None, None),
        ]
    }

    fn sentence(label: &str, locale: &str) -> Step {
        step_sentence(label, &catalogue(), locale)
    }

    #[test]
    fn a_step_reads_as_the_catalogues_sentence_then_its_subject() {
        let s = sentence("track mute gtr", "en");
        assert_eq!(s.sentence.as_deref(), Some("Mute or unmute a track"));
        assert_eq!(
            s.subject,
            Some(Subject {
                kind: SubjectKind::Lane,
                value: "gtr".into()
            })
        );
        assert_eq!(
            sentence("clip process kick.k7x chain", "en").subject.map(|s| s.kind),
            Some(SubjectKind::Clip)
        );
        assert_eq!(
            sentence("project tempo 96", "en").subject.map(|s| s.kind),
            Some(SubjectKind::Plain)
        );
    }

    #[test]
    fn it_reads_in_the_locale_and_falls_back_to_english() {
        assert_eq!(
            sentence("track mute gtr", "fr").sentence.as_deref(),
            Some("Couper une piste")
        );
        assert_eq!(
            sentence("clip process a b", "fr").sentence.as_deref(),
            Some("Process a clip")
        );
    }

    #[test]
    fn the_longest_command_wins() {
        assert_eq!(
            sentence("track mute gtr", "en").sentence.as_deref(),
            Some("Mute or unmute a track")
        );
        assert_eq!(sentence("track other", "en").sentence.as_deref(), Some("Any track"));
    }

    #[test]
    fn a_quoted_subject_is_one_token_unquoted() {
        let s = sentence("track mute \"my lead\"", "en");
        assert_eq!(s.subject.map(|s| s.value), Some("my lead".to_owned()));
        // A broken quote stays as written.
        let s = sentence("track mute \"my lead", "en");
        assert_eq!(s.subject.map(|s| s.value), Some("\"my".to_owned()));
    }

    #[test]
    fn a_line_with_no_subject_is_its_sentence_alone() {
        assert_eq!(sentence("project tempo", "en").subject, None);
    }

    #[test]
    fn a_label_naming_no_command_or_one_with_no_sentence_stands_as_written() {
        assert_eq!(sentence("something else", "en").sentence, None);
        assert_eq!(sentence("silent verb x", "en").sentence, None);
        assert_eq!(sentence("", "en").sentence, None);
    }
}
