// graph/strip.rs — a lane's channel strip: the last node of its first run that is the strip of its kind.
// A reader would get wrong that only the picture lanes' strips come from here; the sound arm is read by step 3's verbs (436).

use super::{GraphDoc, GraphModule, Index};
use crate::resolve::MIXER_PLUGIN;
use indexmap::IndexMap;

/// The strip every sound lane extends, and the picture lane's.
pub const SHARED_STRIP: &str = "@meta/audio/mixer";
pub const PICTURE_STRIP: &str = "@meta/shaders/video-mixer";

/// A module resolving to the engine's Mixer is a strip too, whatever it extends.
fn is_audio_strip(m: &GraphModule<'_>) -> bool {
    m.m.authored.plugin.as_ref().and_then(|p| p.name.as_deref()) == Some(MIXER_PLUGIN)
        || m.extends == Some(SHARED_STRIP)
}

fn is_picture_strip(m: &GraphModule<'_>) -> bool {
    m.extends == Some(PICTURE_STRIP)
}

/// The lane's strip, the LAST of its kind in the run its source starts, or none.
pub fn lane_strip<'g>(g: &'g GraphDoc<'_>, idx: &Index<'g>, lane: &str) -> Option<&'g str> {
    let is: fn(&GraphModule<'_>) -> bool = if g.video_tracks.contains_key(lane) {
        is_picture_strip
    } else {
        is_audio_strip
    };
    idx.first_chain_from(lane)?
        .use_steps
        .iter()
        .rev()
        .map(|s| s.module.as_str())
        .find(|m| g.modules.get(m).is_some_and(is))
}

/// Each of these lanes' strips by lane: what the graph pins as the lane's face.
pub fn lane_strips_of<'g>(g: &'g GraphDoc<'_>, idx: &Index<'g>, lanes: &[&'g str]) -> IndexMap<&'g str, &'g str> {
    lanes
        .iter()
        .filter_map(|lane| lane_strip(g, idx, lane).map(|s| (*lane, s)))
        .collect()
}

/// The picture lanes.
pub fn picture_lanes<'g>(g: &GraphDoc<'g>) -> Vec<&'g str> {
    g.video_tracks.keys().copied().collect()
}
