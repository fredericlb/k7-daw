// views/web/mod.rs — a compiled song's clips as the BROWSER reads them: lengths, names, comps and timelines folded.
// A reader would get wrong that a comped event and a placed timeline are ONE clip to every surface, pieces inside it.

mod clip;
mod fold;
mod project;

pub use clip::{WebClip, WebNote, WebTimelineRef, web_clip, web_clips};
pub use fold::{fold_comps, fold_nests};
pub use project::{WebGroup, WebMarker, WebMixer, WebPlugin, WebProject, WebTimeline, WebTrack, web_project_of};

/// What a render's clip id ends with, reserved by the compiler.
pub(crate) use crate::doc::FROZEN_CLIP_SUFFIX;
