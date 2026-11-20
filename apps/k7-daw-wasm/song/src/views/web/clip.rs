// views/web/clip.rs — one ClipDoc as the browser reads it, after `toWebClip` and `naturalBeats`.
// A reader would get wrong `length`: 0 means "unstated", so the file's own length (in beats) stands in for it.

use super::FROZEN_CLIP_SUFFIX;
use crate::clips::{ClipDoc, ClipKind, CompMark, CrossfadeMark, EventDoc};
use crate::source::SourceError;
use crate::views::lengths::Measure;
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct WebNote {
    pub time: f64,
    pub duration: f64,
    pub key: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub vel: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional, type = "number"))]
    pub repeat: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional, type = "number"))]
    pub skip: Option<u64>,
}

/// A timeline placed as one event: which, and the clips it plays.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct WebTimelineRef {
    #[serde(rename = "ref")]
    #[cfg_attr(feature = "ts", ts(rename = "ref"))]
    pub ref_: String,
    pub clips: Vec<WebClip>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", ts(rename_all = "camelCase"))]
pub struct WebClip {
    pub id: String,
    #[serde(rename = "type")]
    #[cfg_attr(feature = "ts", ts(rename = "type", type = "\"audio\" | \"midi\""))]
    pub kind: ClipKind,
    pub name: String,
    pub start: f64,
    pub length: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub offset: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub source_from: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub source_to: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub source_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub loop_start: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub loop_end: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub warped: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub warp_off: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub disabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub fade_in: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub fade_out: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub frozen: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional, type = "WebEvent"))]
    pub event: Option<EventDoc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional, type = "{ of: string; take: string }"))]
    pub comp: Option<CompMark>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub pieces: Option<Vec<WebClip>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional, type = "{ in?: boolean; out?: boolean }"))]
    pub crossfade: Option<CrossfadeMark>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub timeline: Option<WebTimelineRef>,
    pub notes: Vec<WebNote>,
}

/// JavaScript's truthiness of a number: 0 and NaN are false.
fn truthy(n: f64) -> bool {
    n != 0.0 && !n.is_nan()
}

fn some_if(b: bool) -> Option<bool> {
    b.then_some(true)
}

/// The extent a clip DRAWS at, in beats, for one whose document states none.
fn natural_beats(c: &ClipDoc, tempo: f64, m: &mut dyn Measure) -> Result<f64, SourceError> {
    if c.kind != ClipKind::Audio || c.source.is_empty() || tempo <= 0.0 {
        return Ok(0.0);
    }
    let Some(secs) = m.seconds(&c.source)? else {
        return Ok(0.0);
    };
    let rest = secs - c.source_from.unwrap_or(0.0);
    // A product past the double range would be `inf`, which JSON has no spelling for.
    Ok(if rest > 0.0 {
        ((rest * tempo) / 60.0).min(f64::MAX)
    } else {
        0.0
    })
}

/// One clip as the browser reads it; `tempo` turns the file's seconds into beats when the document states no length.
pub fn web_clip(c: &ClipDoc, tempo: Option<f64>, m: &mut dyn Measure) -> Result<WebClip, SourceError> {
    let length = if truthy(c.length) {
        c.length
    } else {
        match tempo {
            Some(t) => natural_beats(c, t, m)?,
            None => 0.0,
        }
    };
    Ok(WebClip {
        id: c.id.clone(),
        kind: c.kind,
        name: c.name.clone(),
        start: c.start,
        length,
        offset: Some(c.offset),
        source_from: c.source_from,
        source_to: c.source_to,
        source_name: (!c.source.is_empty()).then(|| c.source.rsplit('/').next().unwrap_or("").to_owned()),
        loop_start: Some(c.loop_start),
        loop_end: Some(c.loop_end),
        warped: some_if(c.warp.is_some() || c.warp_off == Some(true)),
        warp_off: some_if(c.warp_off == Some(true)),
        disabled: some_if(c.disabled == Some(true)),
        fade_in: truthy(c.fade_in).then_some(c.fade_in),
        fade_out: truthy(c.fade_out).then_some(c.fade_out),
        frozen: some_if(c.id.ends_with(FROZEN_CLIP_SUFFIX)),
        event: c.event.clone(),
        comp: c.comp.clone(),
        pieces: None,
        crossfade: c.crossfade,
        timeline: None,
        notes: c
            .notes
            .iter()
            .map(|n| WebNote {
                time: n.time,
                duration: n.duration,
                key: n.key,
                vel: (n.vel != 1.0).then_some(n.vel),
                repeat: n.repeat.filter(|r| *r != 0),
                skip: n.skip.filter(|r| *r != 0),
            })
            .collect(),
    })
}

/// A lane's clips, each as `web_clip` reads it, without any fold.
pub fn web_clips(clips: &[ClipDoc], tempo: Option<f64>, m: &mut dyn Measure) -> Result<Vec<WebClip>, SourceError> {
    clips.iter().map(|c| web_clip(c, tempo, m)).collect()
}
