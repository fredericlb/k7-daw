// tools.rs — what a PRESS on a clip or a shot means under a tool: one verb's line to run, or a refusal to say out loud.
// A tool stands on ONE verb, so a gesture is one command and one Ctrl+Z. The note is a message id and its parameters: the page turns it into words.

use crate::js;
use serde::{Deserialize, Serialize};

/// The pointer is the tool with no verb: it means the grab, the marquee and the caret.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum Tool {
    Pointer,
    Cut,
    Erase,
    Mute,
    Crossfade,
}

/// What a press on a clip knows about it: the beats for the decisions, the timecodes for the line.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ClipTarget {
    pub track: String,
    pub name: String,
    pub start: f64,
    /// Beats, 0 meaning the material's natural length: a tail is not always knowable here.
    pub length: f64,
    /// Where the pointer landed, in beats.
    pub at: f64,
    /// `start` and `at` as timecodes: how every `event` verb addresses a placement.
    pub tc: String,
    pub at_tc: String,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub muted: Option<bool>,
}

/// What a press on a SHOT knows about it, in seconds.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ShotTarget {
    pub lane: String,
    pub name: String,
    pub start: f64,
    /// Where the shot ends by the picture's rule; absent for an open-ended last one.
    #[cfg_attr(feature = "ts", ts(optional))]
    #[serde(default)]
    pub end: Option<f64>,
    pub at: f64,
}

/// The sentence a press answers with, as the message's id and what fills it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "id", rename_all = "camelCase")]
pub enum Note {
    CutAtHead { name: String },
    CutAtTail { name: String },
    CutDone { name: String, at: String },
    Erased { name: String, track: String },
    Muted { name: String },
    Unmuted { name: String },
    ShotNoMute,
}

/// What a press produces: a line to run, if any, and what to say.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ToolAct {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub line: Option<String>,
    pub note: Note,
}

/// Two beats that are the same one: the writers' own, since these decide whether a verb is worth sending.
const EPS: f64 = 1e-6;

fn say(note: Note) -> Option<ToolAct> {
    Some(ToolAct { line: None, note })
}

/// A press on a clip; None is the POINTER (or a crossfade, which the timeline answers itself).
pub fn tool_line(tool: Tool, c: &ClipTarget) -> Option<ToolAct> {
    match tool {
        Tool::Pointer | Tool::Crossfade => None,
        Tool::Cut => {
            // On the head there is nothing to cut; the TAIL is left to the verb, a natural length being unknowable here.
            if c.at <= c.start + EPS {
                return say(Note::CutAtHead { name: c.name.clone() });
            }
            if c.length > 0.0 && c.at >= c.start + c.length - EPS {
                return say(Note::CutAtTail { name: c.name.clone() });
            }
            Some(ToolAct {
                line: Some(format!("event split {} {} {}", c.track, c.tc, c.at_tc)),
                note: Note::CutDone {
                    name: c.name.clone(),
                    at: c.at_tc.clone(),
                },
            })
        }
        Tool::Erase => Some(ToolAct {
            line: Some(format!("event remove {} {}", c.track, c.tc)),
            note: Note::Erased {
                name: c.name.clone(),
                track: c.track.clone(),
            },
        }),
        // Bare, so the VERB toggles: two surfaces reading the state and deciding would eventually disagree about it.
        Tool::Mute => Some(ToolAct {
            line: Some(format!("event status mute {} {}", c.track, c.tc)),
            note: if c.muted == Some(true) {
                Note::Unmuted { name: c.name.clone() }
            } else {
                Note::Muted { name: c.name.clone() }
            },
        }),
    }
}

/// The same tools on a picture lane, each on its `picture` verb; None leaves the drag underneath to run.
pub fn picture_tool_line(tool: Tool, s: &ShotTarget) -> Option<ToolAct> {
    let sec = |v: f64| js::number_string(js::round(v * 1000.0) / 1000.0);
    match tool {
        Tool::Pointer | Tool::Crossfade => None,
        Tool::Cut => {
            if s.at <= s.start + 1e-3 {
                return say(Note::CutAtHead { name: s.name.clone() });
            }
            if s.end.is_some_and(|end| s.at >= end - 1e-3) {
                return say(Note::CutAtTail { name: s.name.clone() });
            }
            Some(ToolAct {
                line: Some(format!("picture split {} {} {}", s.lane, sec(s.start), sec(s.at))),
                note: Note::CutDone {
                    name: s.name.clone(),
                    at: format!("{}s", sec(s.at)),
                },
            })
        }
        Tool::Erase => Some(ToolAct {
            line: Some(format!("picture delete {} {}", s.lane, sec(s.start))),
            note: Note::Erased {
                name: s.name.clone(),
                track: s.lane.clone(),
            },
        }),
        Tool::Mute => say(Note::ShotNoMute),
    }
}

/// A clip as the crossfade rule reads it.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct FadeClip {
    pub id: String,
    pub start: f64,
    pub length: f64,
    /// `audio` takes part in a crossfade; anything else does not.
    #[serde(rename = "type")]
    #[cfg_attr(feature = "ts", ts(rename = "type"))]
    pub kind: String,
}

/// The neighbour a crossfade press means, as its index in `clips`, and whether it lies AFTER the pressed clip.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct FadePair {
    pub other: usize,
    pub after: bool,
}

/// The two neighbouring events a crossfade press means: the one after on the clip's right half, the one before on its left.
pub fn crossfade_pair(clips: &[FadeClip], c: &FadeClip, at: f64) -> Option<FadePair> {
    let audio = clips
        .iter()
        .enumerate()
        .filter(|(_, x)| x.kind == "audio" && x.id != c.id);
    if at >= c.start + c.length / 2.0 {
        let mut best: Option<(usize, f64)> = None;
        for (i, x) in audio.filter(|(_, x)| x.start > c.start) {
            if best.is_none_or(|(_, s)| x.start < s) {
                best = Some((i, x.start));
            }
        }
        return best.map(|(other, _)| FadePair { other, after: true });
    }
    let mut best: Option<(usize, f64)> = None;
    for (i, x) in audio.filter(|(_, x)| x.start < c.start) {
        let end = x.start + x.length;
        if best.is_none_or(|(_, e)| end > e) {
            best = Some((i, end));
        }
    }
    best.map(|(other, _)| FadePair { other, after: false })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(at: f64, tc_at: &str) -> ClipTarget {
        ClipTarget {
            track: "drums".into(),
            name: "break".into(),
            start: 4.0,
            length: 8.0,
            at,
            tc: "2:1:1".into(),
            at_tc: tc_at.into(),
            muted: None,
        }
    }
    fn line(a: Option<ToolAct>) -> Option<String> {
        a.and_then(|a| a.line)
    }

    #[test]
    fn the_pointer_produces_nothing_so_the_gesture_underneath_runs() {
        assert_eq!(tool_line(Tool::Pointer, &clip(6.0, "2:3:1")), None);
        assert_eq!(tool_line(Tool::Crossfade, &clip(6.0, "2:3:1")), None);
    }

    #[test]
    fn cut_splits_the_event_at_the_pointer() {
        let act = tool_line(Tool::Cut, &clip(6.0, "2:3:1")).expect("an act");
        assert_eq!(act.line.as_deref(), Some("event split drums 2:1:1 2:3:1"));
        assert_eq!(
            act.note,
            Note::CutDone {
                name: "break".into(),
                at: "2:3:1".into()
            }
        );
    }

    #[test]
    fn cut_refuses_on_the_head_by_name_and_on_the_tail_of_a_stated_length() {
        let head = tool_line(Tool::Cut, &clip(4.0, "2:1:1")).expect("an act");
        assert_eq!((head.line, head.note), (None, Note::CutAtHead { name: "break".into() }));
        assert_eq!(line(tool_line(Tool::Cut, &clip(12.0, "3:1:1"))), None);
    }

    #[test]
    fn cut_leaves_the_tail_of_a_natural_length_event_to_the_verb() {
        let c = ClipTarget {
            length: 0.0,
            ..clip(400.0, "2:3:1")
        };
        assert_eq!(
            line(tool_line(Tool::Cut, &c)).as_deref(),
            Some("event split drums 2:1:1 2:3:1")
        );
    }

    #[test]
    fn erase_removes_the_placement_addressed_where_it_starts() {
        let act = tool_line(Tool::Erase, &clip(11.0, "3:4:1")).expect("an act");
        assert_eq!(act.line.as_deref(), Some("event remove drums 2:1:1"));
        assert_eq!(
            act.note,
            Note::Erased {
                name: "break".into(),
                track: "drums".into()
            }
        );
    }

    #[test]
    fn mute_sends_the_verb_bare_and_reads_the_state_only_to_speak() {
        let on = tool_line(Tool::Mute, &clip(6.0, "2:3:1")).expect("an act");
        let off = tool_line(
            Tool::Mute,
            &ClipTarget {
                muted: Some(true),
                ..clip(6.0, "2:3:1")
            },
        )
        .expect("an act");
        assert_eq!(on.line.as_deref(), Some("event status mute drums 2:1:1"));
        assert_eq!(on.line, off.line);
        assert_eq!(on.note, Note::Muted { name: "break".into() });
        assert_eq!(off.note, Note::Unmuted { name: "break".into() });
    }

    fn shot(at: f64) -> ShotTarget {
        ShotTarget {
            lane: "main".into(),
            name: "shot_a".into(),
            start: 4.0,
            end: Some(10.0),
            at,
        }
    }

    #[test]
    fn a_shot_is_cut_with_picture_split_addressed_in_seconds() {
        assert_eq!(
            line(picture_tool_line(Tool::Cut, &shot(6.5))).as_deref(),
            Some("picture split main 4 6.5")
        );
    }

    #[test]
    fn a_shot_refuses_a_cut_on_either_edge_and_an_open_ended_one_has_no_tail() {
        assert_eq!(line(picture_tool_line(Tool::Cut, &shot(4.0))), None);
        assert_eq!(line(picture_tool_line(Tool::Cut, &shot(10.0))), None);
        let open = ShotTarget {
            end: None,
            ..shot(500.0)
        };
        assert_eq!(
            line(picture_tool_line(Tool::Cut, &open)).as_deref(),
            Some("picture split main 4 500")
        );
    }

    #[test]
    fn a_shot_is_erased_with_picture_delete_and_has_no_mute() {
        assert_eq!(
            line(picture_tool_line(Tool::Erase, &shot(6.5))).as_deref(),
            Some("picture delete main 4")
        );
        assert_eq!(
            picture_tool_line(Tool::Mute, &shot(6.5)).map(|a| a.note),
            Some(Note::ShotNoMute)
        );
        assert_eq!(picture_tool_line(Tool::Pointer, &shot(6.5)), None);
    }

    fn fade(id: &str, start: f64, length: f64, kind: &str) -> FadeClip {
        FadeClip {
            id: id.into(),
            start,
            length,
            kind: kind.into(),
        }
    }

    #[test]
    fn a_crossfade_is_the_event_after_on_the_right_half_and_the_one_before_on_the_left() {
        let clips = [
            fade("a@0", 0.0, 8.0, "audio"),
            fade("b@8", 8.0, 8.0, "audio"),
            fade("m@16", 16.0, 4.0, "midi"),
        ];
        assert_eq!(
            crossfade_pair(&clips, &clips[0], 7.0),
            Some(FadePair { other: 1, after: true })
        );
        assert_eq!(
            crossfade_pair(&clips, &clips[1], 9.0),
            Some(FadePair { other: 0, after: false })
        );
    }

    #[test]
    fn a_crossfade_is_nothing_beside_a_lanes_end_and_never_a_midi_event() {
        let clips = [
            fade("a@0", 0.0, 8.0, "audio"),
            fade("b@8", 8.0, 8.0, "audio"),
            fade("m@16", 16.0, 4.0, "midi"),
        ];
        assert_eq!(crossfade_pair(&clips, &clips[0], 1.0), None);
        assert_eq!(crossfade_pair(&clips, &clips[1], 15.0), None);
    }

    #[test]
    fn the_notes_serialize_as_an_id_and_its_parameters() {
        let json = serde_json::to_value(tool_line(Tool::Cut, &clip(6.0, "2:3:1"))).expect("json");
        assert_eq!(
            json["note"],
            serde_json::json!({"id": "cutDone", "name": "break", "at": "2:3:1"})
        );
        let none = serde_json::to_value(picture_tool_line(Tool::Mute, &shot(1.0))).expect("json");
        assert_eq!(none, serde_json::json!({"note": {"id": "shotNoMute"}}));
    }
}
