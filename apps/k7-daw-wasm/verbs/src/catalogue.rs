// catalogue.rs — commands.yaml as the crate reads it (`commands.json`, generated): the words, the SID, each argument's shape and completion.
// A reader would get wrong that a timecode argument is checked in the song's meter: it is checked at 4/4, as the TypeScript does.

use serde::{Deserialize, Serialize};
use song_edit::js_text::{is_js_space, str_to_number};
use song_schema::parse_position_on;
use std::sync::OnceLock;

#[derive(Clone, Debug, Deserialize)]
pub struct ArgSpec {
    pub name: String,
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
    /// The named provider that offers this argument's values (`tracks`, `nodes`…).
    #[serde(default)]
    pub complete: Option<String>,
    /// …or the closed set the command itself fixes.
    #[serde(default)]
    pub values: Option<Vec<String>>,
    /// A clue about what the argument accepts, when its class's own does not say enough.
    #[serde(default)]
    pub hint: Option<String>,
    /// Trailing arguments only: the line runs without it.
    #[serde(default)]
    pub optional: bool,
}

/// The palette's sentence, in the two languages a surface can be asked for.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Readable {
    pub en: String,
    pub fr: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Spec {
    pub command: String,
    pub sid: String,
    #[serde(default)]
    pub help: Option<String>,
    #[serde(default)]
    pub readable: Option<Readable>,
    /// Absent: the entry runs everywhere, which is `cli`.
    #[serde(default)]
    pub surfaces: Option<Vec<String>>,
    /// The right-click menus that may offer it, as written; `palette.rs` checks the words.
    #[serde(default)]
    pub menu: Option<Vec<String>>,
    #[serde(default)]
    pub confirm: bool,
    #[serde(default)]
    pub args: Vec<ArgSpec>,
}

const TEXT: &str = include_str!("../commands.json");

/// Every command of the catalogue; an unreadable file is an empty one, as the TypeScript's `cached = []`.
pub fn specs() -> &'static [Spec] {
    static SPECS: OnceLock<Vec<Spec>> = OnceLock::new();
    SPECS.get_or_init(|| serde_json::from_str(TEXT).unwrap_or_default())
}

/// The command a line runs: the LONGEST one it starts with, a word boundary after it.
pub fn spec_of(line: &str) -> Option<&'static Spec> {
    specs()
        .iter()
        .filter(|c| {
            line == c.command
                || line
                    .strip_prefix(c.command.as_str())
                    .is_some_and(|r| r.starts_with(' '))
        })
        .max_by_key(|c| c.command.len())
}

/// `parseMidiNote`: a MIDI number 0..127 or a note name (C4, F#3).
pub fn parse_midi_note(s: &str) -> Option<i64> {
    let t = s.trim_matches(is_js_space);
    if !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit()) {
        let k = str_to_number(t);
        return (0.0..=127.0).contains(&k).then_some(k as i64);
    }
    let mut chars = t.chars();
    let letter = chars.next()?.to_ascii_lowercase();
    let pc = match letter {
        'c' => 0,
        'd' => 2,
        'e' => 4,
        'f' => 5,
        'g' => 7,
        'a' => 9,
        'b' => 11,
        _ => return None,
    };
    let rest = chars.as_str();
    let (accidental, octave) = match rest.chars().next() {
        Some('#') => (1, &rest[1..]),
        Some('b') => (-1, &rest[1..]),
        _ => (0, rest),
    };
    let digits = octave.strip_prefix('-').unwrap_or(octave);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let k = (str_to_number(octave) as i64 + 1) * 12 + pc + accidental;
    (0..=127).contains(&k).then_some(k)
}

fn arg_error(kind: &str, name: &str, token: &str) -> Option<String> {
    match kind {
        "timecode" => parse_position_on(token, 4.0, 4.0)
            .err()
            .map(|_| format!("<{name}> expects a bar:beat:sixteenth position (5, 5:1, 5:1:1) — got \"{token}\"")),
        "number" => (!str_to_number(token).is_finite()).then(|| format!("<{name}> expects a number — got \"{token}\"")),
        "note" => parse_midi_note(token)
            .is_none()
            .then(|| format!("<{name}> expects a note name (C4, F#3) or a MIDI number 0..127 — got \"{token}\"")),
        _ => None,
    }
}

/// The first declared argument whose token does not fit its shape, walked by position.
pub fn validate_args(spec: &Spec, user_args: &str) -> Option<String> {
    if spec.args.is_empty() || user_args.is_empty() {
        return None;
    }
    let tokens: Vec<&str> = user_args.split(is_js_space).filter(|t| !t.is_empty()).collect();
    spec.args
        .iter()
        .zip(tokens)
        .find_map(|(a, t)| a.kind.as_deref().and_then(|k| arg_error(k, &a.name, t)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_longest_command_wins() {
        assert_eq!(
            spec_of("event loop points a 1 2 3").map(|s| s.sid.as_str()),
            Some("event.loopPoints")
        );
        assert_eq!(spec_of("event loop a 1").map(|s| s.sid.as_str()), Some("event.loop"));
        assert!(spec_of("eventx").is_none());
        assert_eq!(specs().len(), 192);
    }

    #[test]
    fn a_note_reads_as_midi() {
        assert_eq!(parse_midi_note("C4"), Some(60));
        assert_eq!(parse_midi_note("f#3"), Some(54));
        assert_eq!(parse_midi_note("128"), None);
        assert_eq!(parse_midi_note("H2"), None);
    }
}
