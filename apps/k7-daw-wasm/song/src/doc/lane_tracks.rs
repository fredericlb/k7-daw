// doc/lane_tracks.rs — a lane's halves made into tracks: the pattern plugin heading each, the events placed, the render that replaces them.
// A reader would get wrong the three states of `clips`, whose ORDER matters: a render on the half, a frozen lane, a sounding picture lane, else the events.

use super::build::Pass;
use super::freeze_points::{FreezePoints, frozen_clip};
use super::lanes::{HalfIdx, Pictures, content_type, track_cost};
use super::notes::{Notes, finding};
use super::plugins::CompiledPlugins;
use super::{PluginDoc, TrackDoc, highlighted, text_weight};
use crate::clips::{
    Budget, ClipDoc, Half, Leaf, PatternPlugin, PatternShare, Placer, distinct_ids, mark_crossfades, pattern_plugins,
    pattern_shares, read_pattern, sample_banks,
};
use crate::files::{Finding, Severity};
use crate::lanes::{Lane, LaneKind, Material, is_pattern_lane, pattern_lanes};
use crate::source::SourceError;
use indexmap::IndexMap;
use std::collections::{HashMap, HashSet};

/// How many unwired events are named before the walk stops (decision 311).
const MAX_WIRING: usize = 1_000;

/// The pattern lanes' files read once per compile, with their shares and sample banks.
pub(super) struct Patterns {
    pub shares: IndexMap<String, PatternShare>,
    pub texts: HashMap<String, String>,
    pub banks: Vec<String>,
    pub findings: Vec<Finding>,
}

pub(super) fn read_patterns(
    source: &dyn crate::source::Source,
    roots: &crate::roots::Roots,
    project: &str,
    lanes: &crate::lanes::Lanes<'_>,
) -> Result<Patterns, SourceError> {
    let list = pattern_lanes(lanes);
    let (shares, mut findings) = pattern_shares(project, &list, roots.paths);
    let mut texts = HashMap::new();
    for (lane, strudel) in &list {
        if !shares.contains_key(*lane) {
            continue;
        }
        let (text, warning) = read_pattern(source, project, strudel, roots.paths)?;
        findings.extend(warning);
        texts.insert((*lane).to_owned(), text);
    }
    Ok(Patterns {
        shares,
        texts,
        banks: sample_banks(roots),
        findings,
    })
}

/// The ERRORs of events with no half, one per `(lane, ref)`, at most `MAX_WIRING`.
#[derive(Default)]
pub(super) struct Wiring<'c> {
    seen: HashSet<(&'c str, &'c str)>,
    count: usize,
    pub done: bool,
}

impl Wiring<'_> {
    /// A list that starts with the ERRORs the pictures already made, which count toward the same limit.
    pub fn after(count: usize) -> Wiring<'static> {
        Wiring {
            count,
            ..Wiring::default()
        }
    }
}

/// What the lane walk shares.
pub(super) struct LaneEnv<'a, 'c> {
    pub pass: &'a Pass<'c>,
    pub fp: &'a FreezePoints,
    pub cp: &'a mut CompiledPlugins<'c>,
    pub placer: &'a mut Placer<'c>,
    pub notes: &'a mut Notes,
    pub budget: &'a mut Budget,
    pub pictures: &'a Pictures,
    pub wiring: Wiring<'c>,
    pub soloed: &'a HashSet<&'c str>,
    pub lit: &'a HashSet<&'c str>,
    pub group_of: &'a HashMap<&'c str, &'c str>,
}

fn quote(s: &str) -> String {
    s.chars().take(40).collect()
}

fn pattern_doc(p: PatternPlugin) -> PluginDoc {
    let mut d = PluginDoc::new(&p.id);
    d.plugin_type = p.plugin_type;
    d.name = p.name;
    d.path = p.path;
    d.preset_save = p.preset_save;
    d.role = p.role;
    d.enabled = p.enabled;
    d.params = p.params;
    d.pattern_source = Some(p.pattern_source);
    d.pattern_label = Some(p.pattern_label);
    d.pattern_catch_all = Some(p.pattern_catch_all);
    d.pattern_claimed = Some(p.pattern_claimed);
    d.pattern_plays = Some(p.pattern_plays);
    d.pattern_only = Some(p.pattern_only);
    d.pattern_banks = Some(p.pattern_banks);
    d
}

/// The pattern plugin of each half of `name`, charged before the text is copied (decision 323).
fn patterns_of(
    env: &mut LaneEnv<'_, '_>,
    name: &str,
    lane: &Lane<'_>,
    halves: &[Half<'_>],
) -> Option<IndexMap<String, PluginDoc>> {
    if !is_pattern_lane(lane) {
        return None;
    }
    let p = env.pass.patterns;
    let share = p.shares.get(name)?;
    let text = p.texts.get(name)?;
    let per = 1 + text_weight(text) + share.claimed.iter().map(|c| 1 + text_weight(c)).sum::<usize>() + p.banks.len();
    if !env.budget.charge(per * halves.len().min(2)) {
        return None;
    }
    let made = pattern_plugins(name, halves, text, share, &p.banks);
    Some(made.into_iter().map(|(k, v)| (k, pattern_doc(v))).collect())
}

/// The tracks of one sound lane, one per half; `None` when the doc budget is spent.
pub(super) fn lane_tracks<'c>(env: &mut LaneEnv<'_, 'c>, name: &'c str, lane: &Lane<'c>) -> Option<Vec<TrackDoc>> {
    let pass = env.pass;
    let halves = pass.halves.of_lane(name);
    let idx = HalfIdx::of(&halves);
    let label = pass
        .inputs
        .module(name)
        .map_or_else(|| name.to_owned(), |m| m.own_label());
    let mut patterns = patterns_of(env, name, lane, &halves);
    let leaves: Vec<Leaf<'c>> = env.placer.expand_events(lane.events, &label);
    if !env.budget.charge(1 + leaves.len()) {
        return None;
    }
    let mut per_half: Vec<Vec<usize>> = vec![Vec::new(); halves.len()];
    let mut kinds: Vec<(bool, bool)> = vec![(false, false); halves.len()];
    for (li, leaf) in leaves.iter().enumerate() {
        let r: &'c str = leaf.p.ref_.as_ref().map_or("", |r| r.0.as_str());
        let m = pass.material_of(r);
        match idx.for_material(m) {
            Some(h) => {
                per_half[h].push(li);
                kinds[h].0 |= m == Material::Midi;
                kinds[h].1 |= m != Material::Midi;
            }
            None => {
                if !env.wiring.done && env.wiring.seen.insert((name, r)) {
                    wiring_error(env, name, r, m, &halves, &label);
                }
            }
        }
    }
    let group: &str = env.group_of.get(name).copied().unwrap_or("");
    let frozen = env.fp.lanes.contains(name);
    let lit = env.lit.contains(name);
    let audio_half = idx.for_material(Material::Audio);
    let mut out = Vec::with_capacity(halves.len());
    for (hi, half) in halves.iter().enumerate() {
        let color = lane.color.unwrap_or("");
        let input = pass
            .live
            .get(half.id)
            .or_else(|| (audio_half == Some(hi)).then(|| pass.live.get(name)).flatten());
        let render = env.fp.render_on.get(half.id).map(String::as_str);
        let channels = input.map_or(0, |i| i.channels.len());
        if !env
            .budget
            .charge(track_cost(&[half.id, half.id, group, color], channels, render))
        {
            return None;
        }
        let mut t = TrackDoc::new(half.id);
        t.content_type = content_type(half.port, lane.kind == LaneKind::Strudel, kinds[hi].0, kinds[hi].1).to_owned();
        t.group = group.to_owned();
        t.generated = lane.kind == LaneKind::Strudel;
        t.color = color.to_owned();
        t.mute = pass.inputs.bypassed.contains(name);
        t.solo = env.soloed.contains(name) || env.soloed.contains(group);
        if let Some(i) = input {
            t.input_channels.clone_from(&i.channels);
            t.monitor = i.monitor.name().to_owned();
            t.input_gain = i.gain;
        }
        let placed = |env: &mut LaneEnv<'_, 'c>| -> Vec<ClipDoc> {
            let d = distinct_ids(per_half[hi].iter().map(|&li| env.placer.place(&leaves[li])));
            for (r, start) in &d.twins {
                twin_warning(env, name, &label, r, *start);
            }
            d.clips
        };
        if let Some(render) = env.fp.render_on.get(half.id) {
            t.clips = vec![frozen_clip(half.id, render)];
        } else if frozen {
        } else if let Some(sound) = env.pictures.video_sounds.get(name) {
            if audio_half == Some(hi) {
                if !env.budget.charge(sound.iter().map(ClipDoc::weight).sum()) {
                    return None;
                }
                let sound = distinct_ids(sound.iter().map(|c| vec![c.clone()])).clips;
                t.clips = mark_crossfades(sound, env.placer.budget());
            }
        } else {
            let all = placed(env);
            t.clips = mark_crossfades(all, env.placer.budget());
        }
        if frozen && !per_half[hi].is_empty() {
            t.frozen_material = Some(placed(env));
        }
        let mut plugins: Vec<PluginDoc> = Vec::new();
        if let Some(p) = patterns.as_mut().and_then(|p| p.shift_remove(half.id)) {
            plugins.push(p);
        }
        let (own_plugins, own_sends) = env.cp.take(half.id, env.budget)?;
        plugins.extend(own_plugins);
        if lit {
            highlighted(&mut plugins);
            t.highlight = Some(true);
        }
        t.plugins = plugins;
        t.sends = own_sends;
        out.push(t);
    }
    Some(out)
}

/// A WARNING for an event placed again exactly where an identical one already plays: almost always a duplicate by accident.
fn twin_warning(env: &mut LaneEnv<'_, '_>, lane: &str, label: &str, r: &str, start: f64) {
    let at = song_schema::beats_to_position(start, env.placer.grid().sig).unwrap_or_else(|_| start.to_string());
    env.notes.push(finding(
        label,
        Severity::Warning,
        format!(
            "the same clip '{}' is placed twice at {at} on lane '{}'",
            quote(r),
            quote(lane)
        ),
    ));
}

fn wiring_error(env: &mut LaneEnv<'_, '_>, lane: &str, r: &str, m: Material, halves: &[Half<'_>], label: &str) {
    if env.wiring.count >= MAX_WIRING {
        env.wiring.done = true;
        env.notes.stop_at(
            "project.k7x",
            "more events than this compile names have no output wired — the rest are left out of the list",
        );
        return;
    }
    env.wiring.count += 1;
    let (kind, word) = match m {
        Material::Midi => ("sequence", "midi"),
        Material::Audio => ("clip", "audio"),
        Material::Missing => ("clip", "missing"),
    };
    let ports: Vec<&str> = halves.iter().map(|h| h.port.unwrap_or("")).take(8).collect();
    env.notes.stops(finding(
        label,
        Severity::Error,
        format!(
            "lane '{}': event '{}' is a {kind}, but only the lane's '{}' output is wired — add a '{}#{word}:' key to graph.k7x",
            quote(lane),
            quote(r),
            ports.join("', '"),
            quote(lane)
        ),
    ));
}
