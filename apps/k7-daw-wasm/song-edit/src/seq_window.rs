// seq_window.rs — the window a sequence states, read from its own k7x TEXT rather than from a compiled clip: the note grids are handed the document.
// A reader would get wrong that a number is no edge: `from: 1.5` is a seconds spelling a sequence does not have, and a half-typed timecode reads as absent.

use crate::js_text::js_trim;
use k7x::edit::{Value, parse};
use song_schema::parse_position_on;

/// The window a sequence states: where it opens in beats, and where it closes (None: wherever the notes end).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SeqWindow {
    pub from: f64,
    pub to: Option<f64>,
}

/// A window resolved against the notes it windows: both edges, in beats.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowSpan {
    pub from: f64,
    pub to: f64,
}

fn edge(data: &Value, field: &str, num: f64, den: f64) -> Option<f64> {
    let Some(Value::Str(raw)) = data.get(field) else {
        return None;
    };
    let raw = js_trim(raw);
    if raw.is_empty() {
        return None;
    }
    parse_position_on(raw, num, den).ok().filter(|b| b.is_finite() && *b >= 0.0)
}

/// The window `text` states, or None when it states none or does not parse: a half-typed edge reads as absent, not as 0.
#[must_use]
pub fn seq_window_from_text(text: &str, num: f64, den: f64) -> Option<SeqWindow> {
    let data = parse(text).ok()?;
    let from = edge(&data, "from", num, den);
    let to = edge(&data, "to", num, den);
    if from.is_none() && to.is_none() {
        return None;
    }
    Some(SeqWindow {
        from: from.unwrap_or(0.0),
        to,
    })
}

/// Both edges as beats against the notes' extent; None when the span does not run forwards.
#[must_use]
pub fn window_span(win: SeqWindow, extent: f64) -> Option<WindowSpan> {
    let to = win.to.unwrap_or(extent);
    (to > win.from).then_some(WindowSpan { from: win.from, to })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seq(head: &str) -> String {
        format!("{head}\nnotes([{{ t: 0, d: 1, k: 60 }}])\n")
    }

    fn window(head: &str) -> Option<SeqWindow> {
        seq_window_from_text(&seq(head), 4.0, 4.0)
    }

    #[test]
    fn it_reads_both_edges_as_beats() {
        assert_eq!(
            window("from(\"2:1\")\nto(\"4:1\")"),
            Some(SeqWindow {
                from: 4.0,
                to: Some(12.0)
            })
        );
    }

    #[test]
    fn a_lone_from_leaves_to_open_and_a_lone_to_starts_at_the_first_beat() {
        assert_eq!(
            window("from(\"2:1\")"),
            Some(SeqWindow {
                from: 4.0,
                to: None
            })
        );
        assert_eq!(
            window("to(\"3:1\")"),
            Some(SeqWindow {
                from: 0.0,
                to: Some(8.0)
            })
        );
    }

    #[test]
    fn a_file_stating_nothing_says_nothing() {
        assert_eq!(window("name(\"drums\")"), None);
    }

    #[test]
    fn the_window_follows_the_meter() {
        let w = seq_window_from_text(&seq("from(\"2:1\")"), 3.0, 4.0);
        assert_eq!(w.map(|w| w.from), Some(3.0));
    }

    #[test]
    fn it_refuses_to_guess() {
        assert_eq!(window("from(\"3:\")"), None);
        assert_eq!(window("from(1.5)"), None);
        assert_eq!(window("from(\"\")"), None);
        assert_eq!(seq_window_from_text("from(\"2:1\")\nnotes([", 4.0, 4.0), None);
    }

    #[test]
    fn a_window_resolves_against_the_notes_it_windows() {
        let at = |from, to| window_span(SeqWindow { from, to }, 16.0);
        assert_eq!(at(4.0, None), Some(WindowSpan { from: 4.0, to: 16.0 }));
        assert_eq!(at(0.0, Some(32.0)), Some(WindowSpan { from: 0.0, to: 32.0 }));
        assert_eq!(at(8.0, Some(4.0)), None);
        assert_eq!(at(16.0, None), None);
    }
}
