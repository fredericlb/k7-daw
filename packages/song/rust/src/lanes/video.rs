// lanes/video.rs — the picture lanes as the picture compile reads them: a rect, a mute off the graph's bypass, the events as placed.
// A reader would get wrong that a lane's own `disabled` and the graph's bypass are both a mute here, and that nothing is read again.

use super::Lanes;
use crate::files::Placement;
use indexmap::IndexMap;
use std::collections::HashSet;

/// One picture lane, borrowed from the resolved lanes (272).
#[derive(Clone, Debug, PartialEq)]
pub struct VideoTrack<'m> {
    pub name: &'m str,
    pub color: Option<&'m str>,
    pub mute: bool,
    pub frame: Option<[f64; 4]>,
    pub events: &'m [Placement],
}

/// The picture lanes by name, in the map's order, the mute being the bypass or the lane's own `disabled`.
pub fn video_tracks<'m>(lanes: &'m Lanes<'m>, bypassed: &HashSet<String>) -> IndexMap<&'m str, VideoTrack<'m>> {
    lanes
        .picture
        .iter()
        .map(|(name, lane)| {
            (
                *name,
                VideoTrack {
                    name,
                    color: lane.color,
                    mute: bypassed.contains(*name) || lane.disabled,
                    frame: lane.frame,
                    events: lane.events,
                },
            )
        })
        .collect()
}
