// reads/shell.rs — what the shell projects out of one document (the track order, the recorder's fields, the strips, the first sequence of each lane), and the one mute/solo rule.
// A reader would get wrong that group and solo reach a lane from two places: a group's mute silences its members, and any solo silences every lane not soloed.

use super::clip::mixer_modules;
use super::model::{RGroup, RTrack, RVideoTrack};
use indexmap::IndexMap;
use serde::Serialize;

/// One lane's mixer state as the footer and the strips read it; `pan` is 0..1, `mixer` is empty when the lane has no strip.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DocTrack {
    pub id: String,
    pub volume: f64,
    pub pan: f64,
    pub mute: bool,
    pub solo: bool,
    pub highlight: bool,
    pub mixer: String,
}

/// What the recorder and the footer print of the song.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", ts(rename_all = "camelCase"))]
pub struct RecInfo {
    pub tempo: f64,
    pub beats_per_bar: f64,
    pub beat_unit: f64,
    pub count_in: f64,
    #[cfg_attr(feature = "ts", ts(type = "Record<string, string>"))]
    pub names: IndexMap<String, String>,
}

/// An instrument lane's sequence: its first MIDI clip and where it starts.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", ts(rename_all = "camelCase"))]
pub struct SeqRef {
    pub seq_id: String,
    pub start: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", ts(rename_all = "camelCase"))]
pub struct ShellView {
    pub track_order: Vec<String>,
    pub rec_info: RecInfo,
    #[cfg_attr(feature = "ts", ts(as = "std::collections::HashMap<String, SeqRef>"))]
    pub seq_by_track: IndexMap<String, SeqRef>,
    pub mixer_strips: Vec<String>,
    pub doc_tracks: Vec<DocTrack>,
}

/// A zero or missing meter half reads as four.
fn or_four(n: Option<f64>) -> f64 {
    n.filter(|n| *n != 0.0 && !n.is_nan()).unwrap_or(4.0)
}

/// Everything the shell projects out of one document.
pub fn shell_view(tempo: f64, num: Option<f64>, den: Option<f64>, count_in: f64, tracks: &[RTrack]) -> ShellView {
    let mut seq_by_track = IndexMap::new();
    for t in tracks {
        if let Some(seq) = t.clips.iter().find(|c| c.kind == "midi") {
            seq_by_track.insert(
                t.id.clone(),
                SeqRef {
                    seq_id: seq.id.clone(),
                    start: seq.start,
                },
            );
        }
    }
    ShellView {
        track_order: tracks.iter().map(|t| t.id.clone()).collect(),
        rec_info: RecInfo {
            tempo,
            beats_per_bar: or_four(num),
            beat_unit: or_four(den),
            count_in,
            names: tracks.iter().map(|t| (t.id.clone(), t.name.clone())).collect(),
        },
        seq_by_track,
        mixer_strips: mixer_modules(tracks),
        doc_tracks: tracks
            .iter()
            .map(|t| {
                let mixer = t.mixer.as_ref();
                DocTrack {
                    id: t.id.clone(),
                    volume: mixer.and_then(|m| m.volume).unwrap_or(0.0),
                    pan: (mixer.and_then(|m| m.pan).unwrap_or(0.0) + 1.0) / 2.0,
                    mute: t.mute == Some(true),
                    solo: t.solo == Some(true),
                    highlight: t.highlight == Some(true),
                    mixer: mixer.and_then(|m| m.module.clone()).unwrap_or_default(),
                }
            })
            .collect(),
    }
}

/// How a lane reads: muted by itself or its group, soloed by itself or its group, and so silent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LaneMix {
    pub mute: bool,
    pub solo: bool,
    pub silent: bool,
}

/// Each lane's mix, and whether each picture lane is off: solo is mixer-wide, so one soloed lane silences every other.
pub fn lane_mix(tracks: &[RTrack], groups: &[RGroup], video: &[RVideoTrack]) -> (Vec<LaneMix>, Vec<bool>) {
    let group_of = |id: &str| groups.iter().rev().find(|g| g.id == id);
    let eff: Vec<(bool, bool)> = tracks
        .iter()
        .map(|t| {
            let g = group_of(&t.group);
            (
                t.mute == Some(true) || g.is_some_and(|g| g.mute == Some(true)),
                t.solo == Some(true) || g.is_some_and(|g| g.solo == Some(true)),
            )
        })
        .collect();
    let any_solo = eff.iter().any(|e| e.1);
    let lanes = eff
        .iter()
        .map(|&(mute, solo)| LaneMix {
            mute,
            solo,
            silent: mute || (any_solo && !solo),
        })
        .collect();
    let any_video = video.iter().any(|v| v.solo == Some(true));
    let pictures = video
        .iter()
        .map(|v| v.mute == Some(true) || (any_video && v.solo != Some(true)))
        .collect();
    (lanes, pictures)
}
