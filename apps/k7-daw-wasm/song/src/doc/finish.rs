// doc/finish.rs — what a pass puts on its finished tracks: the colours, the project's own fields, the console's two halves.
// A reader would get wrong that the console goes on last, over the finished document and before `recordDir` (decision 329).

use super::build::{Pass, cap};
use super::notes::Notes;
use super::plugins::CompiledPlugins;
use super::{PluginDoc, ProjectDoc, TrackDoc, auto_colors, text_weight, track_category};
use crate::clips::Budget;
use crate::console::{Console, HalfPlugin, channel_slot, takes_channel};
use indexmap::IndexMap;

/// A lane that states no colour takes its category's shade, by name so both halves of one share it (decision 327).
pub(super) fn colour(tracks: &mut [TrackDoc], cp: &CompiledPlugins<'_>, budget: &mut Budget) {
    let mut order: IndexMap<&str, Vec<&str>> = IndexMap::new();
    for t in tracks.iter() {
        let hints = order.entry(t.name.as_str()).or_default();
        if let Some(h) = cp.lane_hints.get(t.id.as_str()) {
            hints.extend(h.iter().copied());
        }
    }
    if !budget.charge(order.values().map(|h| 1 + h.len()).sum()) {
        return;
    }
    let cats: Vec<(&str, _)> = order.iter().map(|(n, h)| (*n, track_category(n, h))).collect();
    let shades: IndexMap<&str, String> = auto_colors(&cats);
    let paint: Vec<(usize, String)> = tracks
        .iter()
        .enumerate()
        .filter(|(_, t)| t.color.is_empty())
        .filter_map(|(i, t)| shades.get(t.name.as_str()).map(|c| (i, c.clone())))
        .collect();
    for (i, c) in paint {
        tracks[i].color = c;
    }
}

/// The project's own fields (decision: uuid when non-empty, the defaults of `compileNodeProject`).
pub(super) fn head_fields(
    doc: &mut ProjectDoc,
    p: &Pass<'_>,
    settings: Option<&crate::files::ProjectSettings>,
    notes: &mut Notes,
    budget: &mut Budget,
) {
    let head = &p.inputs.head;
    let named = settings.map_or(0, |s| {
        s.name.as_deref().map_or(0, text_weight) + s.uuid.as_deref().map_or(0, text_weight)
    });
    let markers: usize = head
        .markers
        .iter()
        .map(|m| 1 + text_weight(&m.tc) + text_weight(&m.name))
        .sum();
    if !budget.charge(1 + named + markers) {
        return;
    }
    if let Some(s) = settings {
        doc.uuid = s.uuid.clone().filter(|u| !u.is_empty());
        doc.name = s.name.clone().unwrap_or_else(|| "untitled".to_owned());
        if let Some(rate) = s.sample_rate.as_ref() {
            match u32::try_from(rate.0) {
                Ok(r) => doc.sample_rate = r,
                Err(_) => notes.stop_at("project.k7x", "the sample rate is larger than the engine can read"),
            }
        }
        doc.master_volume = s.master_volume.as_ref().map_or(0.0, |m| m.0);
    }
    doc.tempo = p.grid.tempo;
    doc.time_sig_numerator = p.grid.sig.num();
    doc.time_sig_denominator = p.grid.sig.den();
    doc.markers.clone_from(&p.inputs.head.markers);
    doc.settings = p.inputs.head.settings;
}

/// The console's two halves, on last over the finished document (decision 329).
pub(super) fn place_console(doc: &mut ProjectDoc, console: &Console, budget: &mut Budget, notes: &mut Notes) {
    let (Some(channel), Some(buss)) = (&console.channel, &console.buss) else {
        return;
    };
    let cost = |h: &HalfPlugin, id: &str| {
        1 + h.params.len()
            + h.params.keys().map(|k| text_weight(k)).sum::<usize>()
            + [&h.kind, &h.name, &h.path, &h.preset_save, &h.role]
                .iter()
                .map(|s| text_weight(s))
                .sum::<usize>()
            + 2 * text_weight(id)
    };
    if !budget.charge(cost(buss, "master")) {
        cap(notes, "project.k7x");
        return;
    }
    let mut master = vec![half_doc(buss.with_id("master"))];
    master.append(&mut doc.master_plugins);
    doc.master_plugins = master;
    for t in &mut doc.tracks {
        if !takes_channel(&t.output_channels, &t.role) {
            continue;
        }
        if !budget.charge(cost(channel, &t.id) + t.plugins.len()) {
            cap(notes, "project.k7x");
            return;
        }
        let types: Vec<&str> = t.plugins.iter().map(|p| p.plugin_type.as_str()).collect();
        let at = channel_slot(&types);
        t.plugins.insert(at, half_doc(channel.with_id(&t.id)));
    }
}

fn half_doc(h: HalfPlugin) -> PluginDoc {
    let mut d = PluginDoc::new(&h.id);
    d.plugin_type = h.kind;
    d.name = h.name;
    d.path = h.path;
    d.preset_save = h.preset_save;
    d.role = h.role;
    d.enabled = h.enabled;
    d.params = h.params;
    d
}
