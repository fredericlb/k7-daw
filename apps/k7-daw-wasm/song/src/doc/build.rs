// doc/build.rs — one pass: the plugins walked, the tracks made, the curves baked, the colours given, the console placed.
// A reader would get wrong that a pass changes nothing it was given: the passes of one compile share `Pass` and build a document each.

use super::automation::bake_automations;
use super::compile::Spent;
use super::finish::{colour, head_fields, place_console};
use super::freeze_points::{FreezePoints, freeze_points, frozen_clip};
use super::lane_tracks::{LaneEnv, Patterns, Wiring, lane_tracks};
use super::lanes::{Halves, Pictures, lane_sound_ref, picture_cost, picture_sound_track, track_cost};
use super::notes::Notes;
use super::plugins::compile_plugins;
use super::texts::Texts;
use super::{BUDGET, GroupDoc, ProjectDoc, TrackDoc, order_tracks, text_weight};
use crate::cache::Cache;
use crate::clips::{Budget, Grid, Materials, Placer, mark_crossfades};
use crate::console::Console;
use crate::freeze::{self, Edges, FreezePlan};
use crate::lanes::inputs::Inputs;
use crate::lanes::live::LiveInput;
use crate::lanes::ownership::Ownership;
use crate::lanes::{LaneKind, Lanes, Material};
use crate::modules::assets::Places;
use crate::pictures::picture_sounds;
use crate::source::{Source, SourceError};
use indexmap::IndexMap;
use std::collections::{HashMap, HashSet};

/// What every pass of one compile shares, read once.
pub(super) struct Pass<'c> {
    pub source: &'c dyn Source,
    pub inputs: &'c Inputs,
    pub lanes: &'c Lanes<'c>,
    pub own: &'c Ownership,
    pub edges: &'c Edges<'c>,
    pub live: &'c IndexMap<String, LiveInput>,
    pub console: &'c Console,
    pub materials: &'c Materials,
    pub patterns: &'c Patterns,
    pub halves: &'c Halves<'c>,
    pub grid: Grid,
    pub names: &'c [&'c str],
    pub places: &'c Places,
    pub seqs: HashSet<&'c str>,
    pub clip_ids: HashSet<&'c str>,
}

impl Pass<'_> {
    /// What an event's ref names, which is the whole of what decides its output port.
    pub fn material_of(&self, r: &str) -> Material {
        if self.seqs.contains(r) {
            Material::Midi
        } else if self.clip_ids.contains(r) {
            Material::Audio
        } else {
            Material::Missing
        }
    }
}

/// A pass's document, its findings and what it spent.
pub(super) struct Built {
    pub doc: ProjectDoc,
    pub notes: Notes,
    pub spent: Spent,
    pub plans: Vec<FreezePlan>,
}

const CAP_WORDS: &str = "this project asks for more than one compile will build — the rest is left out";

/// The one ERROR a spent budget gives, at the file where the pass first stopped.
pub(super) fn cap(notes: &mut Notes, file: &str) {
    if !notes.stop.iter().any(|f| f.message == CAP_WORDS) {
        notes.stop_at(file, CAP_WORDS);
    }
}

/// A pass over `auto`, the automatic renders in force (module → render), laid over the modules (decision 307).
pub(super) fn build_doc<'c>(
    p: &Pass<'c>,
    cache: &mut Cache,
    auto: &HashMap<&str, &str>,
    freeze_budget: &mut Budget,
) -> Result<Built, SourceError> {
    let inputs = p.inputs;
    let mut notes = Notes::default();
    let mut budget = Budget::new(BUDGET);
    let mut spent = Spent::default();
    let mut doc = ProjectDoc::empty();
    let file = inputs.head.root.as_deref().and_then(|r| r.project.as_ref());
    let settings = file.and_then(|f| f.project.as_ref());
    let is_lane = |n: &str| p.lanes.sound.contains_key(n);
    let fp = freeze_points(
        p.source,
        inputs,
        p.own,
        p.edges,
        p.names,
        &is_lane,
        auto,
        &mut notes,
        freeze_budget,
    )?;
    if fp.capped {
        notes.stop_at(
            "project.k7x",
            "the frozen modules of this project need more work than one compile does",
        );
    }
    let mut texts = Texts::new(p.source, &inputs.project, p.places);
    let mut cp = compile_plugins(inputs, p.own, &fp.plugins, &mut texts, &mut notes, &mut budget)?;
    if let Some(at) = cp.capped_at.take() {
        cap(&mut notes, &at);
    }
    let mut placer = Placer::new(p.grid, p.materials);
    let (pictures, joined) = picture_join(p, &mut placer, &mut notes)?;
    cp.claims = claims_of(p, &pictures, &fp);
    let soloed: HashSet<&str> = file
        .into_iter()
        .flat_map(|f| f.soloed.iter().flatten())
        .map(String::as_str)
        .collect();
    let lit: HashSet<&str> = file
        .into_iter()
        .flat_map(|f| f.highlighted.iter().flatten())
        .map(String::as_str)
        .collect();
    let mut group_of: HashMap<&str, &str> = HashMap::new();
    for (name, l) in &p.lanes.sound {
        if l.kind == LaneKind::Group {
            for m in l.tracks {
                group_of.insert(m, name);
            }
        }
    }
    let mut group_ok = true;
    for (id, l) in &p.lanes.sound {
        if l.kind != LaneKind::Group || l.disabled {
            continue;
        }
        let color = l.color.unwrap_or("");
        if !budget.charge(1 + 2 * text_weight(id) + text_weight(color)) {
            group_ok = false;
            break;
        }
        doc.groups.push(GroupDoc {
            id: (*id).to_owned(),
            name: (*id).to_owned(),
            color: color.to_owned(),
            mute: inputs.bypassed.contains(*id),
            solo: soloed.contains(id),
        });
    }
    if !group_ok {
        cap(&mut notes, "project.k7x");
    }
    let mut tracks: Vec<TrackDoc> = Vec::new();
    {
        let sound: Vec<&str> = p
            .lanes
            .sound
            .iter()
            .filter(|(_, l)| l.kind != LaneKind::Group && !l.disabled)
            .map(|(k, _)| *k)
            .collect();
        let sounding: Vec<&str> = pictures.picture_sounds.keys().map(String::as_str).collect();
        let names: Vec<&str> = sound.iter().chain(&sounding).copied().collect();
        let stated: &[String] = file.and_then(|f| f.track_order.as_deref()).unwrap_or_default();
        let order = order_tracks(&names, stated);
        let mut env = LaneEnv {
            pass: p,
            fp: &fp,
            cp: &mut cp,
            placer: &mut placer,
            notes: &mut notes,
            budget: &mut budget,
            pictures: &pictures,
            wiring: Wiring::after(joined),
            soloed: &soloed,
            lit: &lit,
            group_of: &group_of,
        };
        for at in order {
            if env.wiring.done {
                break;
            }
            if at < sound.len() {
                let name = names[at];
                let Some(lane) = p.lanes.sound.get(name) else { continue };
                match lane_tracks(&mut env, name, lane) {
                    Some(t) => tracks.extend(t),
                    None => {
                        cap(env.notes, "project.k7x");
                        break;
                    }
                }
            } else if !picture_tracks(&mut env, names[at], &mut tracks) {
                cap(env.notes, "project.k7x");
                break;
            }
        }
    }
    let label_of = |key: &str| -> String {
        inputs
            .module(key)
            .and_then(|m| m.chain.clone())
            .unwrap_or_else(|| key.to_owned())
    };
    for name in &p.own.input_chains {
        let label = label_of(name);
        let channels = p.live.get(name).map_or(0, |i| i.channels.len()) + p.own.cue_pair.len();
        if !budget.charge(track_cost(&[name, &label], channels, None)) {
            cap(&mut notes, "project.k7x");
            break;
        }
        let mut t = TrackDoc::new(name);
        t.name = label;
        t.mute = inputs.bypassed.contains(name);
        t.solo = soloed.contains(name.as_str());
        if let Some(i) = p.live.get(name) {
            t.input_channels.clone_from(&i.channels);
            t.monitor = i.monitor.name().to_owned();
            t.input_gain = i.gain;
        }
        if p.own.is_cued(name) {
            t.output_channels.clone_from(&p.own.cue_pair);
        }
        let Some((plugins, sends)) = cp.take(name, &mut budget) else {
            cap(&mut notes, "project.k7x");
            break;
        };
        t.plugins = plugins;
        t.sends = sends;
        tracks.push(t);
    }
    let taps = p.own.tap_chains.keys().filter(|n| !p.own.bus_chains.contains(*n));
    for name in p.own.bus_chains.iter().chain(taps) {
        let label = label_of(name);
        let render = fp.render_on.get(name).map(String::as_str);
        if !budget.charge(track_cost(&[name, &label], p.own.cue_pair.len(), render)) {
            cap(&mut notes, "project.k7x");
            break;
        }
        let mut t = TrackDoc::new(name);
        t.name = label;
        t.role = "effect".to_owned();
        if p.own.is_cued(name) {
            t.output_channels.clone_from(&p.own.cue_pair);
        }
        t.mute = inputs.bypassed.contains(name)
            || inputs.bus_only.contains(name)
            || (p.own.is_cued(name) && p.own.cue_pair.is_empty());
        t.solo = soloed.contains(name.as_str());
        if let Some(r) = fp.render_on.get(name) {
            t.clips = vec![frozen_clip(name, r)];
        }
        let Some((plugins, sends)) = cp.take(name, &mut budget) else {
            cap(&mut notes, "project.k7x");
            break;
        };
        t.plugins = plugins;
        t.sends = sends;
        if let Some(tap) = p.own.tap_chains.get(name)
            && let Some(plugin) = cp.plugin_of_node.get(tap.node.as_str())
        {
            if !budget.charge(1 + text_weight(plugin)) {
                cap(&mut notes, "project.k7x");
                break;
            }
            t.input_from = Some(super::InputFromDoc {
                plugin: plugin.clone(),
                pair: tap.pair,
            });
        }
        tracks.push(t);
    }
    let baked = bake_automations(
        p.source,
        cache,
        inputs,
        p.own,
        &cp.raw,
        &mut tracks,
        p.grid.sig,
        p.grid.tempo,
        &mut notes,
        &mut budget,
    )?;
    spent.baker = baked.baker;
    colour(&mut tracks, &cp, &mut budget);
    doc.tracks = tracks;
    head_fields(&mut doc, p, settings, &mut notes, &mut budget);
    doc.master_plugins = cp.track_plugins.remove(crate::lanes::MASTER_OWNER).unwrap_or_default();
    if !inputs.timelines.values.is_empty() {
        let mut tp = Placer::new(p.grid, p.materials);
        doc.timelines = Some(tp.timeline_docs());
        let (found, capped, w) = tp.finish();
        spent.placer = spent.placer.max(w);
        placer_findings(&mut notes, found, capped);
    }
    let (found, capped, w) = placer.finish();
    spent.placer = spent.placer.max(w);
    placer_findings(&mut notes, found, capped);
    place_console(&mut doc, p.console, &mut budget, &mut notes);
    if !doc.finite() {
        notes.stop_at(
            "project.k7x",
            "a number in the compiled document is not finite, so the engine could not read it",
        );
    }
    // The bake and the colours stop on a spent budget without a word of their own (decision 385).
    if budget.capped() {
        cap(&mut notes, "project.k7x");
    }
    spent.doc = budget.spent();
    spent.freeze = freeze_budget.spent();
    Ok(Built {
        doc,
        notes,
        spent,
        plans: fp.plans,
    })
}

fn placer_findings(notes: &mut Notes, found: Vec<crate::files::Finding>, _capped: bool) {
    for f in found {
        if f.severity == crate::files::Severity::Error {
            notes.stops(f);
        } else {
            notes.push(f);
        }
    }
}

/// The pass's picture sounds on its own `Placer` (decision 526); ERRORs stop and WARNINGs do not, and the stops are counted for 311's list.
fn picture_join(p: &Pass<'_>, placer: &mut Placer<'_>, notes: &mut Notes) -> Result<(Pictures, usize), SourceError> {
    let joined = picture_sounds(p.source, p.inputs, p.lanes, placer)?;
    let mut stops = 0;
    for f in joined.findings {
        if f.severity == crate::files::Severity::Error {
            stops += 1;
            notes.stops(f);
        } else {
            notes.push(f);
        }
    }
    let pictures = Pictures {
        video_sounds: joined.video_sounds,
        picture_sounds: joined.picture_sounds,
    };
    Ok((pictures, stops))
}

/// How many tracks ask for each owner's plugins: a chain that is a half, an input and a tap at once has three (decision 394).
fn claims_of(p: &Pass<'_>, pictures: &Pictures, _fp: &FreezePoints) -> HashMap<String, usize> {
    let mut out: HashMap<String, usize> = HashMap::new();
    let mut add = |id: &str| *out.entry(id.to_owned()).or_default() += 1;
    for (name, l) in &p.lanes.sound {
        if l.kind != LaneKind::Group && !l.disabled {
            p.halves.of_lane(name).iter().for_each(|h| add(h.id));
        }
    }
    for lane in pictures.picture_sounds.keys() {
        let halves = p.halves.audio_of(lane);
        if halves.is_empty() {
            add(&lane_sound_ref(lane));
        }
        halves.iter().for_each(|h| add(h.id));
    }
    p.own.input_chains.iter().for_each(|n| add(n));
    let taps = p.own.tap_chains.keys().filter(|n| !p.own.bus_chains.contains(*n));
    p.own.bus_chains.iter().chain(taps).for_each(|n| add(n));
    out
}

/// A picture lane's own sound (decision 333); false when the budget is spent.
fn picture_tracks<'c>(env: &mut LaneEnv<'_, 'c>, lane: &'c str, out: &mut Vec<TrackDoc>) -> bool {
    let p = env.pass;
    let own = lane_sound_ref(lane);
    let color = p.lanes.picture.get(lane).and_then(|l| l.color).unwrap_or("");
    let halves = p.halves.audio_of(lane).to_vec();
    let count = halves.len().max(1);
    for i in 0..count {
        let half = halves.get(i);
        let clips = if i == 0 {
            let src = env.pictures.picture_sounds.get(lane).cloned().unwrap_or_default();
            if !env
                .budget
                .charge(picture_cost(lane, color) + src.iter().map(crate::clips::ClipDoc::weight).sum::<usize>())
            {
                return false;
            }
            mark_crossfades(src, env.placer.budget())
        } else {
            Vec::new()
        };
        let mut t = picture_sound_track(
            lane,
            half,
            i == 0,
            color,
            p.inputs.bypassed.contains(own.as_str()),
            env.soloed.contains(own.as_str()),
            clips,
        );
        let key = half.map_or(own.as_str(), |h| h.id);
        let Some((plugins, sends)) = env.cp.take(key, env.budget) else {
            return false;
        };
        t.plugins = plugins;
        t.sends = sends;
        out.push(t);
    }
    true
}

/// The freeze budget of a compile, for whoever builds a pass.
pub(super) fn new_freeze_budget() -> Budget {
    Budget::new(freeze::BUDGET)
}
