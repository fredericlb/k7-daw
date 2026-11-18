// doc/wire.rs — what a document goes through on its way out: the group fold and the finite walk (decisions 331, 354).
// A reader would get wrong that the compile never folds: `with_group_mix` is the file service's, applied at the send.

use super::{GroupDoc, PluginDoc, ProjectDoc, TrackDoc};
use crate::clips::ClipDoc;
use std::collections::HashMap;

/// Fold each group's mute and solo onto its members: OR, not override (docs/mixFolding.md).
pub fn with_group_mix(doc: &mut ProjectDoc) {
    if doc.groups.is_empty() {
        return;
    }
    // A group listed twice is read as `new Map(pairs)` reads it: the last wins.
    let by_id: HashMap<&str, &GroupDoc> = doc.groups.iter().map(|g| (g.id.as_str(), g)).collect();
    for t in &mut doc.tracks {
        if let Some(g) = by_id.get(t.group.as_str()).filter(|g| g.mute || g.solo) {
            t.mute = t.mute || g.mute;
            t.solo = t.solo || g.solo;
        }
    }
}

fn plugin_finite(p: &PluginDoc) -> bool {
    p.params.values().all(|v| v.is_finite())
        && p.rack
            .iter()
            .flatten()
            .all(|r| r.start.is_finite() && r.length.is_finite())
}

fn track_finite(t: &TrackDoc) -> bool {
    t.input_gain.is_finite()
        && t.clips.iter().all(ClipDoc::finite)
        && t.frozen_material.iter().flatten().all(ClipDoc::finite)
        && t.plugins.iter().all(plugin_finite)
        && t.sends.iter().all(|s| s.level.is_finite())
        && t.automation
            .iter()
            .all(|a| a.points.iter().all(|p| p[0].is_finite() && p[1].is_finite()))
}

impl ProjectDoc {
    /// Whether every number the engine would read is finite; JSON writes the others as `null` (decision 354).
    pub fn finite(&self) -> bool {
        [self.tempo, self.master_volume, self.settings.metronome_level]
            .iter()
            .all(|v| v.is_finite())
            && self.master_plugins.iter().all(plugin_finite)
            && self.tracks.iter().all(track_finite)
            && self.markers.iter().all(|m| m.beat.is_finite())
            && self
                .timelines
                .iter()
                .flatten()
                .all(|t| t.length.is_finite() && t.lanes.iter().all(|l| l.clips.iter().all(ClipDoc::finite)))
    }
}
