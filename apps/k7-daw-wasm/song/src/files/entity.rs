// files/entity.rs — the clips/ and sequences/ files, each a flat struct keyed by its own filename; after entitySchemas.ts.
// A reader would get wrong that clips/ holds three kinds: the source's extension tells them apart, and an audio clip is the fallback.

use super::Check;
use super::de::{DeError, from_map};
use super::notes::Notes;
use super::scalar::{
    AtLeast2, CAPTION_EXTS, CaptionSource, IMAGE_EXTS, MidiKey, NonEmpty, NonNeg, PictureSource, Pos, SeqEdge,
    SoundFile, Unit, VIDEO_EXTS, WindowEdge, has_ext,
};
use k7x::{MapNode, Node, PathSeg, Scalar};
use serde::{Deserialize, Serialize};

/// One note, and on a step sequencer one pad.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Note {
    pub t: NonNeg,
    pub d: Pos,
    pub k: MidiKey,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub v: Option<Unit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repeat: Option<AtLeast2>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip: Option<AtLeast2>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sequence {
    pub notes: Notes,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<SeqEdge>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<SeqEdge>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_rack: Option<NonEmpty>,
}

impl Check for Sequence {}

/// A stretch map from the DAW a clip was imported from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Warp {
    pub algorithm: NonEmpty,
    pub markers: Vec<(f64, f64)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disable: Option<bool>,
}

/// Corrections to the onsets the engine detected, in source seconds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OnsetEdits {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub added: Option<Vec<NonNeg>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub removed: Option<Vec<NonNeg>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioClip {
    pub source: NonEmpty,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<WindowEdge>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<WindowEdge>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warp: Option<Warp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub onsets: Option<OnsetEdits>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VideoClip {
    pub source: PictureSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sound: Option<SoundFile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub optimized: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Align {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CaptionPosition {
    Bottom,
    Top,
    Middle,
}

/// A subtitle file placed on a picture lane, and how its cues are set.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptionClip {
    pub source: CaptionSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size: Option<Pos>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align: Option<Align>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<CaptionPosition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub margin: Option<NonNeg>,
}

/// Audio, video or captions, told apart by the source's extension.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Clip {
    Audio(AudioClip),
    Video(VideoClip),
    Caption(CaptionClip),
}

impl Check for Clip {
    fn check(&self) -> Option<(Vec<PathSeg>, String)> {
        let Clip::Audio(a) = self else { return None };
        // A backwards window plays nothing, so it is refused where the line can be named.
        if let (Some(from), Some(to)) = (
            a.from.as_ref().and_then(WindowEdge::seconds),
            a.to.as_ref().and_then(WindowEdge::seconds),
        ) && to <= from
        {
            return Some((
                super::finding::keys(&["to"]),
                "'to' must be greater than 'from'".to_owned(),
            ));
        }
        match &a.warp {
            Some(w) if w.markers.len() < 2 => Some((
                super::finding::keys(&["warp", "markers"]),
                "expected at least 2 points".to_owned(),
            )),
            _ => None,
        }
    }
}

/// Decodes a clip file's map, trying the kind its source names and falling back to audio, as the union does.
pub(crate) fn decode_clip(root: &MapNode) -> Result<Clip, DeError> {
    let source = root
        .entries
        .iter()
        .rev()
        .find(|e| e.key == "source")
        .and_then(|e| match e.value {
            Node::Scalar {
                value: Scalar::Str(s), ..
            } => Some(s),
            _ => None,
        });
    let audio = || from_map::<AudioClip>(root).map(Clip::Audio);
    match source {
        Some(s) if has_ext(s, VIDEO_EXTS) || has_ext(s, IMAGE_EXTS) => from_map::<VideoClip>(root)
            .map(Clip::Video)
            .or_else(|first| audio().map_err(|_| first)),
        Some(s) if has_ext(s, CAPTION_EXTS) => from_map::<CaptionClip>(root)
            .map(Clip::Caption)
            .or_else(|first| audio().map_err(|_| first)),
        _ => audio(),
    }
}
