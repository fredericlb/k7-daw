// fill.rs — the pure half of the command bar's parameter form: what a line still owes, and where a gesture's values land.
// A reader would get wrong that the form is a queue: `values` has one slot per param, "" meaning unanswered, so any field can be redone.

use serde::{Deserialize, Serialize};
use song_edit::js_text::js_trim;
use std::collections::HashMap;

/// What the form reads of a param: its name, its class, and whether the line runs without it.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FormArg {
    pub name: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub optional: bool,
}

fn blank(v: Option<&String>) -> bool {
    v.is_none_or(|s| js_trim(s).is_empty())
}

/// How many params must be filled before the line runs: everything up to the last non-optional one, since skipping shifts the rest.
#[must_use]
pub fn required_count(args: &[FormArg]) -> usize {
    let mut n = args.len();
    while n > 0 && args[n - 1].optional {
        n -= 1;
    }
    n
}

/// A fresh answer sheet: one empty slot per param, so a field can be filled out of order.
#[must_use]
pub fn empty_values(args: &[FormArg]) -> Vec<String> {
    args.iter().map(|_| String::new()).collect()
}

/// Whether the line can be dispatched as it stands: a trailing optional left blank is the answer 'not that'.
#[must_use]
pub fn can_run(args: &[FormArg], values: &[String]) -> bool {
    (0..required_count(args)).all(|i| !blank(values.get(i)))
}

/// The line's arguments, trailing blanks dropped; a blank in the middle is kept, or the params after it would shift left.
#[must_use]
pub fn line_args(values: &[String]) -> Vec<String> {
    let mut out = values.to_vec();
    while out.last().is_some_and(|v| js_trim(v).is_empty()) {
        out.pop();
    }
    out
}

/// The next field after `from`: the last one stays put, so Tab cannot walk off the form.
#[must_use]
pub fn next_field(args: &[FormArg], from: usize) -> usize {
    (from + 1).min(args.len().saturating_sub(1))
}

/// The first field still unanswered, or -1 when the form is complete: what Enter uses when the line is not runnable yet.
#[must_use]
pub fn first_missing(args: &[FormArg], values: &[String]) -> i64 {
    (0..required_count(args))
        .find(|&i| blank(values.get(i)))
        .map_or(-1, |i| i as i64)
}

/// A gesture's values placed into the form, and the field that now has the keyboard.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Offered {
    pub values: Vec<String>,
    pub active: usize,
}

/// Where a gesture's values land, keyed by param name, filling forward only from the field with the keyboard.
#[must_use]
pub fn apply_offer(
    args: &[FormArg],
    offered: &HashMap<String, String>,
    values: &[String],
    active: i64,
) -> Option<Offered> {
    let mut out = values.to_vec();
    let mut hi = None;
    for (i, a) in args.iter().enumerate().skip(active.max(0) as usize) {
        let Some(v) = offered.get(&a.name) else { continue };
        if out.len() <= i {
            out.resize(i + 1, String::new());
        }
        out[i] = v.clone();
        hi = Some(i);
    }
    hi.map(|active| Offered { values: out, active })
}

/// What the timeline caret is looking at, in the vocabulary a command line speaks: timecodes rather than beats.
#[derive(Clone, Debug, Default, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct CaretAim {
    pub track: Option<String>,
    pub at: Option<String>,
    pub event_start: Option<String>,
    pub sel_tracks: Option<String>,
    pub sel_from: Option<String>,
    pub sel_to: Option<String>,
}

impl CaretAim {
    fn fact(&self, name: &str) -> Option<&str> {
        let v = match name {
            "track" => &self.track,
            "at" => &self.at,
            "eventStart" => &self.event_start,
            "selTracks" => &self.sel_tracks,
            "selFrom" => &self.sel_from,
            "selTo" => &self.sel_to,
            _ => &None,
        };
        v.as_deref().filter(|s| !s.is_empty())
    }
}

/// Which caret facts answer which of a command's params: an explicit list, or a caret would pre-answer every destructive verb.
const CARET_ANSWERS: [(&str, &[(&str, &str)]); 5] = [
    // The event is addressed by its start, and the cut is its own argument, which is what the caret gives.
    (
        "event split",
        &[("track", "track"), ("timecode", "eventStart"), ("at", "at")],
    ),
    // Split's inverse, answered by the rectangle: `tracks` carries every lane it covered.
    (
        "selection consolidate",
        &[("tracks", "selTracks"), ("from", "selFrom"), ("to", "selTo")],
    ),
    // The timeline a range is turned into, the name left to the form.
    (
        "selection timeline",
        &[("tracks", "selTracks"), ("from", "selFrom"), ("to", "selTo")],
    ),
    // Only `at`: these two are project-wide, so making the length the hole keeps the gesture deliberate.
    ("project add", &[("at", "at")]),
    ("project remove", &[("at", "at")]),
];

/// The caret's answers to one command, ready for `apply_offer`: a fact it cannot supply is absent, so the form still opens on it.
#[must_use]
pub fn caret_answers(canonical: &str, caret: Option<&CaretAim>) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let (Some(caret), Some((_, wanted))) = (caret, CARET_ANSWERS.iter().find(|(c, _)| *c == canonical)) else {
        return out;
    };
    for (param, fact) in *wanted {
        if let Some(v) = caret.fact(fact) {
            out.insert((*param).to_owned(), v.to_owned());
        }
    }
    out
}

/// Whether the field with the keyboard is asking for `kind`: the inlet for a gesture that knows a value and not a command.
#[must_use]
pub fn wants_kind(args: &[FormArg], active: i64, kind: &str) -> bool {
    usize::try_from(active)
        .ok()
        .and_then(|i| args.get(i))
        .is_some_and(|a| a.kind == kind)
}

/// What Enter does in the parameter form.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum EnterAction {
    Take { value: String },
    Run,
    Goto { index: usize },
    None,
}

/// What Enter does: a highlighted suggestion first, but only when it would change the field, or Enter is eaten for ever.
#[must_use]
pub fn enter_action(args: &[FormArg], values: &[String], active: i64, suggestion: Option<&str>) -> EnterAction {
    let current = usize::try_from(active)
        .ok()
        .and_then(|i| values.get(i))
        .map_or("", String::as_str);
    if let Some(s) = suggestion
        && s != current
    {
        return EnterAction::Take { value: s.to_owned() };
    }
    if can_run(args, values) {
        return EnterAction::Run;
    }
    match first_missing(args, values) {
        i if i >= 0 => EnterAction::Goto { index: i as usize },
        _ => EnterAction::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arg(name: &str, kind: &str, optional: bool) -> FormArg {
        FormArg {
            name: name.into(),
            kind: kind.into(),
            optional,
        }
    }

    fn text(name: &str) -> FormArg {
        arg(name, "text", false)
    }

    fn move_args() -> Vec<FormArg> {
        vec![
            arg("track", "track", false),
            arg("timecode", "timecode", false),
            arg("newTimecode", "timecode", false),
            arg("toTrack", "track", true),
        ]
    }

    fn vals(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

    fn offer(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect()
    }

    #[test]
    fn required_count_stops_at_the_last_non_optional_param() {
        assert_eq!(required_count(&move_args()), 3);
        assert_eq!(required_count(&[text("a"), text("b")]), 2);
        assert_eq!(required_count(&[arg("a", "text", true), arg("b", "text", true)]), 0);
        assert_eq!(required_count(&[text("a"), arg("b", "text", true), text("c")]), 3);
        assert_eq!(required_count(&[]), 0);
    }

    #[test]
    fn can_run_waits_for_every_required_answer_and_reads_whitespace_as_blank() {
        let m = move_args();
        assert!(!can_run(&m, &vals(&["gtr", "", "", ""])));
        assert!(!can_run(&m, &vals(&["gtr", "1:1:1", "", ""])));
        assert!(can_run(&m, &vals(&["gtr", "1:1:1", "5:1:1", ""])));
        assert!(!can_run(&m, &vals(&["gtr", "1:1:1", "   ", ""])));
        assert!(can_run(&[], &[]));
    }

    #[test]
    fn line_args_drops_trailing_blanks_and_keeps_a_middle_one() {
        assert_eq!(
            line_args(&vals(&["gtr", "1:1:1", "5:1:1", ""])),
            vals(&["gtr", "1:1:1", "5:1:1"])
        );
        assert_eq!(line_args(&vals(&["gtr", "", "5:1:1"])), vals(&["gtr", "", "5:1:1"]));
    }

    #[test]
    fn tab_stops_at_the_last_field_and_the_first_missing_one_is_named() {
        let m = move_args();
        assert_eq!(next_field(&m, 0), 1);
        assert_eq!(next_field(&m, 3), 3);
        assert_eq!(first_missing(&m, &empty_values(&m)), 0);
        assert_eq!(first_missing(&m, &vals(&["gtr", "", "", ""])), 1);
        assert_eq!(first_missing(&m, &vals(&["gtr", "1:1:1", "5:1:1", ""])), -1);
    }

    #[test]
    fn apply_offer_fills_what_the_gesture_knows_and_the_keys_follow_the_last() {
        let m = move_args();
        let got = apply_offer(
            &m,
            &offer(&[("track", "gtr"), ("timecode", "1:1:1"), ("newTimecode", "5:1:1")]),
            &empty_values(&m),
            0,
        );
        assert_eq!(
            got,
            Some(Offered {
                values: vals(&["gtr", "1:1:1", "5:1:1", ""]),
                active: 2
            })
        );
        let got = apply_offer(&m, &offer(&[("toTrack", "bass")]), &empty_values(&m), 0);
        assert_eq!(got.map(|o| o.active), Some(3));
    }

    #[test]
    fn apply_offer_fills_forward_only_and_answers_none_when_nothing_matches() {
        let m = move_args();
        let from_two = apply_offer(
            &m,
            &offer(&[("track", "gtr"), ("newTimecode", "5:1:1")]),
            &vals(&["keep", "", "", ""]),
            2,
        );
        assert_eq!(from_two.map(|o| o.values), Some(vals(&["keep", "", "5:1:1", ""])));
        assert_eq!(apply_offer(&m, &offer(&[("nope", "x")]), &empty_values(&m), 0), None);
    }

    fn caret(f: impl FnOnce(&mut CaretAim)) -> CaretAim {
        let mut c = CaretAim::default();
        f(&mut c);
        c
    }

    #[test]
    fn the_caret_answers_only_the_commands_listed_and_only_the_facts_it_has() {
        let c = caret(|c| {
            c.track = Some("gtr".into());
            c.at = Some("3:1:1".into());
            c.event_start = Some("2:1:1".into());
            c.sel_tracks = Some("gtr,bass".into());
            c.sel_from = Some("1:1:1".into());
            c.sel_to = Some("5:1:1".into());
        });
        assert_eq!(
            caret_answers("event split", Some(&c)),
            offer(&[("track", "gtr"), ("timecode", "2:1:1"), ("at", "3:1:1")])
        );
        assert_eq!(
            caret_answers("selection consolidate", Some(&c)),
            offer(&[("tracks", "gtr,bass"), ("from", "1:1:1"), ("to", "5:1:1")])
        );
        assert_eq!(caret_answers("project add", Some(&c)), offer(&[("at", "3:1:1")]));
        assert!(caret_answers("track mute", Some(&c)).is_empty());
        assert!(caret_answers("event split", None).is_empty());
        let bare = caret(|c| c.at = Some("3:1:1".into()));
        assert_eq!(caret_answers("event split", Some(&bare)), offer(&[("at", "3:1:1")]));
    }

    #[test]
    fn a_field_asks_for_a_kind_by_its_class() {
        let m = move_args();
        assert!(wants_kind(&m, 0, "track"));
        assert!(!wants_kind(&m, 0, "timecode"));
        assert!(!wants_kind(&m, 9, "track"));
        assert!(!wants_kind(&m, -1, "track"));
    }

    #[test]
    fn enter_takes_a_suggestion_that_changes_the_field_then_runs_then_goes_to_the_gap() {
        let m = move_args();
        let full = vals(&["gtr", "1:1:1", "5:1:1", ""]);
        assert_eq!(
            enter_action(&m, &full, 0, Some("bass")),
            EnterAction::Take { value: "bass".into() }
        );
        // The suggestion already in the field would eat every Enter.
        assert_eq!(enter_action(&m, &full, 0, Some("gtr")), EnterAction::Run);
        assert_eq!(enter_action(&m, &full, 0, None), EnterAction::Run);
        assert_eq!(
            enter_action(&m, &vals(&["gtr", "", "", ""]), 0, None),
            EnterAction::Goto { index: 1 }
        );
        assert_eq!(enter_action(&[text("a")], &vals(&["x"]), 0, None), EnterAction::Run);
    }
}
