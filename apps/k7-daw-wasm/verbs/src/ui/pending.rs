// pending.rs — what a one-shot timeline command DOES, read off its line, so the lanes show it the moment it is sent.
// A reader would get wrong that this predicts the document: it is a guess until the next document makes it true or a refusal takes it back.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use song_edit::js_text::is_js_space;
use song_schema::{parse_duration_on, parse_position_on};

/// One event, as the lines name it: its lane and the beat it starts on.
#[must_use]
pub fn event_key(track: &str, start_beats: f64) -> String {
    // `Math.round`: halves go up, and a rounded zero has no sign.
    let micro = (start_beats * 1e6 + 0.5).floor();
    format!("{track}@{}", if micro == 0.0 { 0 } else { micro as i64 })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum Fade {
    FadeIn,
    FadeOut,
}

/// What a sent line is expected to do to the lanes, until the document says so itself.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum PendingEdit {
    Hide {
        key: String,
    },
    Mute {
        key: String,
        on: Option<bool>,
    },
    Split {
        key: String,
        at: f64,
    },
    Fade {
        key: String,
        which: Fade,
        length: f64,
    },
    Warp {
        key: String,
        on: Option<bool>,
    },
    /// A range move: the lanes it moves (none for all of them), the span of starts it takes, and the shift.
    Shift {
        tracks: Option<Vec<String>>,
        from: f64,
        to: f64,
        by: f64,
    },
}

/// The tokens of `s` after `prefix`, which must be exactly `n` non-space words one space apart (`n + 1` when `extra` allows a trailing one).
fn words<'a>(s: &'a str, prefix: &str, n: usize, extra: bool) -> Option<Vec<&'a str>> {
    let rest = s.strip_prefix(prefix)?;
    let parts: Vec<&str> = rest.split(' ').collect();
    let fits = parts.len() == n || (extra && parts.len() == n + 1);
    (fits && parts.iter().all(|p| !p.is_empty() && !p.chars().any(is_js_space))).then_some(parts)
}

fn state(word: Option<&&str>) -> Option<Option<bool>> {
    match word.copied() {
        None => Some(None),
        Some("on") => Some(Some(true)),
        Some("off") => Some(Some(false)),
        Some("toggle") => Some(None),
        Some(_) => None,
    }
}

type Parsed = Result<Option<Vec<PendingEdit>>, song_schema::SchemaError>;

fn read(s: &str, num: f64, den: f64) -> Parsed {
    let at = |tc: &str| parse_position_on(tc, num, den);
    let dur = |tc: &str| parse_duration_on(tc, num, den);
    if let Some(w) = words(s, "event remove ", 2, false) {
        return Ok(Some(vec![PendingEdit::Hide {
            key: event_key(w[0], at(w[1])?),
        }]));
    }
    if let Some(list) = s
        .strip_prefix("events remove ")
        .filter(|l| l.starts_with('[') && l.ends_with(']') && !l.contains(['\n', '\r']))
    {
        let Ok(Value::Array(items)) = serde_json::from_str::<Value>(list) else {
            return Ok(Some(vec![]));
        };
        let mut out = Vec::new();
        for e in &items {
            match e {
                // A null element throws on `.track`, which discards the whole line's edits.
                Value::Null => return Ok(Some(vec![])),
                Value::Object(o) => {
                    if let (Some(Value::String(t)), Some(Value::String(st))) = (o.get("track"), o.get("start")) {
                        out.push(PendingEdit::Hide {
                            key: event_key(t, at(st)?),
                        });
                    }
                }
                _ => {}
            }
        }
        return Ok(Some(out));
    }
    for (prefix, mute) in [("event status mute ", true), ("event status warp ", false)] {
        if let Some(w) = words(s, prefix, 2, true)
            && let Some(on) = state(w.get(2))
        {
            let key = event_key(w[0], at(w[1])?);
            return Ok(Some(vec![if mute {
                PendingEdit::Mute { key, on }
            } else {
                PendingEdit::Warp { key, on }
            }]));
        }
    }
    // The crossfade's two fades: its overlap is the server's to find, and arrives with the document.
    if let Some(w) = words(s, "event crossfade ", 4, false) {
        let length = if w[3] == "0" { 0.0 } else { dur(w[3])? };
        return Ok(Some(vec![
            PendingEdit::Fade {
                key: event_key(w[0], at(w[1])?),
                which: Fade::FadeOut,
                length,
            },
            PendingEdit::Fade {
                key: event_key(w[0], at(w[2])?),
                which: Fade::FadeIn,
                length,
            },
        ]));
    }
    if let Some(w) = words(s, "event split ", 3, false) {
        return Ok(Some(vec![PendingEdit::Split {
            key: event_key(w[0], at(w[1])?),
            at: at(w[2])?,
        }]));
    }
    if let Some(w) = words(s, "events move ", 4, false) {
        let (sign, rest) = match w[3].as_bytes() {
            [b'-', _, ..] => (-1.0, &w[3][1..]),
            [b'+', _, ..] => (1.0, &w[3][1..]),
            _ => (1.0, w[3]),
        };
        return Ok(Some(vec![PendingEdit::Shift {
            tracks: (w[0] != "all").then(|| w[0].split(',').map(str::to_owned).collect()),
            from: at(w[1])?,
            to: at(w[2])?,
            by: sign * dur(rest)?,
        }]));
    }
    Ok(None)
}

/// The edits `line` makes: none for any other verb, or for a line that does not parse.
#[must_use]
pub fn pending_of(line: &str, num: f64, den: f64) -> Vec<PendingEdit> {
    let s = song_edit::js_text::js_trim(line);
    // A line that does not parse changes nothing here: the command will say why.
    read(s, num, den).ok().flatten().unwrap_or_default()
}

/// The fields of a clip the edits read.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ClipFields {
    pub start: f64,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub disabled: Option<bool>,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub fade_in: Option<f64>,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub fade_out: Option<f64>,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub warp_off: Option<bool>,
}

/// A lane's clips as the edits leave them: a hidden one is none, a changed one the fields that changed (null takes a field away).
#[must_use]
pub fn pending_clips(track: &str, clips: &[ClipFields], edits: &[PendingEdit]) -> Vec<Option<Map<String, Value>>> {
    clips
        .iter()
        .map(|c| {
            let key = event_key(track, c.start);
            if edits
                .iter()
                .any(|e| matches!(e, PendingEdit::Hide { key: k } if *k == key))
            {
                return None;
            }
            let mut disabled = c.disabled;
            let mut fades = [c.fade_in, c.fade_out];
            let mut warp_off = c.warp_off;
            let mut start = c.start;
            let mut touched = Map::new();
            for e in edits {
                match e {
                    PendingEdit::Mute { key: k, on } if *k == key => {
                        let v = on.unwrap_or(!disabled.unwrap_or(false));
                        disabled = Some(v);
                        touched.insert("disabled".to_owned(), Value::Bool(v));
                    }
                    PendingEdit::Fade { key: k, which, length } if *k == key => {
                        let slot = usize::from(*which == Fade::FadeOut);
                        // `length || undefined`: a zero fade is no fade.
                        fades[slot] = (*length != 0.0).then_some(*length);
                        let name = if slot == 0 { "fadeIn" } else { "fadeOut" };
                        touched.insert(name.to_owned(), fades[slot].map_or(Value::Null, Value::from));
                    }
                    // `on` follows the map; the clip's flag is the opposite one, that it is declined.
                    PendingEdit::Warp { key: k, on } if *k == key => {
                        let v = on.map_or(!warp_off.unwrap_or(false), |o| !o);
                        warp_off = Some(v);
                        touched.insert("warpOff".to_owned(), Value::Bool(v));
                    }
                    PendingEdit::Shift { tracks, from, to, by }
                        if tracks.as_ref().is_none_or(|t| t.iter().any(|x| x == track))
                            && c.start >= from - 1e-6
                            && c.start <= to + 1e-6 =>
                    {
                        start += by;
                        touched.insert("start".to_owned(), Value::from(start));
                    }
                    _ => {}
                }
            }
            Some(touched)
        })
        .collect()
}

/// Where a pending split cuts the event starting at `start` on `track`, if one does.
#[must_use]
pub fn pending_split_at(track: &str, start: f64, edits: &[PendingEdit]) -> Option<f64> {
    let key = event_key(track, start);
    edits.iter().find_map(|e| match e {
        PendingEdit::Split { key: k, at } if *k == key => Some(*at),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn of(line: &str) -> Vec<PendingEdit> {
        pending_of(line, 4.0, 4.0)
    }

    #[test]
    fn it_reads_removals_a_mute_a_split_and_a_range_move() {
        assert_eq!(
            of("event remove gtr 2:1"),
            vec![PendingEdit::Hide {
                key: event_key("gtr", 4.0)
            }]
        );
        assert_eq!(
            of(r#"events remove [{"track":"gtr","start":"1:1"}]"#),
            vec![PendingEdit::Hide {
                key: event_key("gtr", 0.0)
            }]
        );
        assert_eq!(
            of("event status mute gtr 3:1"),
            vec![PendingEdit::Mute {
                key: event_key("gtr", 8.0),
                on: None
            }]
        );
        assert_eq!(
            of("event split gtr 1:1 1:3"),
            vec![PendingEdit::Split {
                key: event_key("gtr", 0.0),
                at: 2.0
            }]
        );
        assert_eq!(
            of("events move gtr,bass 1:1 4:4 -0:1"),
            vec![PendingEdit::Shift {
                tracks: Some(vec!["gtr".into(), "bass".into()]),
                from: 0.0,
                to: 15.0,
                by: -1.0
            }]
        );
    }

    #[test]
    fn it_reads_nothing_from_another_verb_or_a_line_that_does_not_parse() {
        assert_eq!(of("event move gtr 2:1 3:1"), vec![]);
        assert_eq!(of("events remove [oops"), vec![]);
        assert_eq!(of("event remove gtr nonsense"), vec![]);
        // One bad start discards the whole list, as the throw did.
        assert_eq!(
            of(r#"events remove [{"track":"a","start":"1:1"},{"track":"b","start":"zz"}]"#),
            vec![]
        );
        assert_eq!(of("event status mute gtr 3:1 maybe"), vec![]);
    }

    fn clips() -> Vec<ClipFields> {
        let c = |start: f64, disabled: Option<bool>| ClipFields {
            start,
            disabled,
            fade_in: None,
            fade_out: None,
            warp_off: None,
        };
        vec![c(0.0, None), c(8.0, Some(true)), c(16.0, None)]
    }

    #[test]
    fn it_hides_flips_a_mute_and_shifts_a_range_and_leaves_the_rest() {
        let mut edits = of("event remove gtr 1:1");
        edits.extend(of("event status mute gtr 3:1"));
        edits.extend(of("events move all 5:1 5:1 +1:0"));
        let shown = pending_clips("gtr", &clips(), &edits);
        assert_eq!(shown[0], None);
        assert_eq!(
            shown[1],
            Some(json!({ "disabled": false }).as_object().unwrap().clone())
        );
        assert_eq!(shown[2], Some(json!({ "start": 20.0 }).as_object().unwrap().clone()));
        assert!(
            pending_clips("gtr", &clips(), &[])
                .iter()
                .all(|c| c.as_ref().is_some_and(Map::is_empty))
        );
    }

    #[test]
    fn it_knows_where_a_pending_split_cuts() {
        let edits = of("event split gtr 1:1 1:3");
        assert_eq!(pending_split_at("gtr", 0.0, &edits), Some(2.0));
        assert_eq!(pending_split_at("gtr", 8.0, &edits), None);
    }

    #[test]
    fn a_crossfade_draws_two_fades_and_a_warp_flips_at_once() {
        let lane = vec![
            ClipFields {
                start: 0.0,
                disabled: None,
                fade_in: None,
                fade_out: None,
                warp_off: None,
            },
            ClipFields {
                start: 8.0,
                disabled: None,
                fade_in: None,
                fade_out: None,
                warp_off: Some(false),
            },
        ];
        let mut edits = of("event crossfade gtr 1:1 3:1 0:1");
        edits.extend(of("event status warp gtr 3:1"));
        let shown = pending_clips("gtr", &lane, &edits);
        assert_eq!(shown[0].as_ref().unwrap().get("fadeOut"), Some(&json!(1.0)));
        let second = shown[1].as_ref().unwrap();
        assert_eq!(second.get("fadeIn"), Some(&json!(1.0)));
        assert_eq!(second.get("warpOff"), Some(&json!(true)));
    }

    #[test]
    fn a_zero_length_crossfade_takes_the_fade_away() {
        let edits = of("event crossfade gtr 1:1 3:1 0");
        let shown = pending_clips(
            "gtr",
            &[ClipFields {
                start: 0.0,
                disabled: None,
                fade_in: Some(2.0),
                fade_out: Some(2.0),
                warp_off: None,
            }],
            &edits,
        );
        assert_eq!(shown[0].as_ref().unwrap().get("fadeOut"), Some(&Value::Null));
    }
}
