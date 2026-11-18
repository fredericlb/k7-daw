// freeze/cut.rs — the document cut down to one freeze plan: what a render is made of, and what its name is a hash of.
// A reader would get wrong that the cut is OUTPUT: it is cloned, so every part of it is charged before the copy (decision 342).

use super::{FreezePlan, plugin_module};
use crate::clips::{Budget, ClipDoc, TimelineLaneDoc};
use crate::doc::{ProjectDoc, TrackDoc, text_weight};
use std::collections::HashSet;

/// The strings of a clip that `ClipDoc::weight` counts none of, at `text_weight` (decision 342).
fn clip_text(c: &ClipDoc) -> usize {
    let opt = |s: &Option<String>| s.as_deref().map_or(0, text_weight);
    let event = c
        .event
        .iter()
        .chain(c.nest.iter().filter_map(|n| n.event.as_ref()))
        .map(|e| {
            let takes = e.takes.iter().flatten().map(|t| text_weight(&t.ref_.0)).sum::<usize>();
            let comp = e.comp.iter().flatten().map(|p| text_weight(&p.take.0)).sum::<usize>();
            opt(&e.fade_in) + opt(&e.fade_out) + takes + comp
        });
    let nest = c
        .nest
        .iter()
        .map(|n| text_weight(&n.of) + text_weight(&n.ref_))
        .sum::<usize>();
    let comp = c
        .comp
        .iter()
        .map(|m| text_weight(&m.of) + text_weight(&m.take))
        .sum::<usize>();
    text_weight(&c.id) + text_weight(&c.name) + text_weight(&c.source) + nest + comp + event.sum::<usize>()
}

fn clips_weight(clips: &[ClipDoc]) -> usize {
    clips.iter().map(|c| c.weight() + clip_text(c)).sum()
}

/// What copying a track costs with its clips and frozen material.
pub(crate) fn track_weight(t: &TrackDoc) -> usize {
    t.weight() + clips_weight(&t.clips) + t.frozen_material.as_deref().map_or(0, clips_weight)
}

/// What the cut copies besides the tracks: its own strings, the groups, the markers and the timelines.
fn rest_weight(doc: &ProjectDoc) -> usize {
    let own = text_weight(&doc.name) + text_weight(&doc.record_dir) + doc.uuid.as_deref().map_or(0, text_weight);
    let groups = doc
        .groups
        .iter()
        .map(|g| 1 + text_weight(&g.id) + text_weight(&g.name) + text_weight(&g.color))
        .sum::<usize>();
    let markers = doc
        .markers
        .iter()
        .map(|m| 1 + text_weight(&m.tc) + text_weight(&m.name))
        .sum::<usize>();
    let lane = |l: &TimelineLaneDoc| {
        1 + text_weight(&l.id) + text_weight(&l.name) + text_weight(&l.color) + clips_weight(&l.clips)
    };
    let timelines: usize = doc
        .timelines
        .iter()
        .flatten()
        .map(|t| 1 + text_weight(&t.id) + t.lanes.iter().map(lane).sum::<usize>())
        .sum();
    1 + own + groups + markers + timelines
}

/// The lanes of the plan and the plugins of its modules, with the sends between them; the master neutral. None: the budget is spent.
pub fn freeze_cut(doc: &ProjectDoc, plan: &FreezePlan, budget: &mut Budget) -> Option<ProjectDoc> {
    let in_plan = |p: &crate::doc::PluginDoc| plan.modules.contains(plugin_module(&p.id));
    let mut kept: Vec<&TrackDoc> = Vec::new();
    for t in &doc.tracks {
        if !budget.charge(1 + t.plugins.len()) {
            return None;
        }
        if plan.lanes.contains(&t.id) || t.plugins.iter().any(in_plan) {
            kept.push(t);
        }
    }
    if !budget.charge(rest_weight(doc)) {
        return None;
    }
    let mut tracks: Vec<TrackDoc> = Vec::with_capacity(kept.len());
    for t in &kept {
        if !budget.charge(track_weight(t)) {
            return None;
        }
        tracks.push((*t).clone());
    }
    let ids: HashSet<&str> = kept.iter().map(|t| t.id.as_str()).collect();
    for t in &mut tracks {
        t.solo = false;
        t.plugins.retain(in_plan);
        t.sends.retain(|s| ids.contains(s.to.as_str()));
    }
    Some(ProjectDoc {
        uuid: doc.uuid.clone(),
        name: doc.name.clone(),
        tempo: doc.tempo,
        sample_rate: doc.sample_rate,
        time_sig_numerator: doc.time_sig_numerator,
        time_sig_denominator: doc.time_sig_denominator,
        master_volume: 0.0,
        master_plugins: Vec::new(),
        record_dir: doc.record_dir.clone(),
        record_mute: doc.record_mute,
        tracks,
        groups: doc.groups.clone(),
        markers: doc.markers.clone(),
        settings: doc.settings,
        timelines: doc.timelines.clone(),
    })
}
