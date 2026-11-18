// views/hints/mod.rs — a project's entities for the editor's completion: names, no substance, read tolerantly and weighed.
// A reader would get wrong that a file that will not read costs only itself here: completion on a half-written file is the point.

mod params;
mod paths;

use super::walk::{Assets, media_of};
use super::{Spend, capped_finding};
use crate::cache::Cache;
use crate::curves::read_curves;
use crate::files::Finding;
use crate::jsorder::own_key_entries;
use crate::lanes::inputs::Inputs;
use crate::lanes::wiring::{clip_source, reads_as_picture};
use crate::lanes::{CUE_NODE, OUT_NODE, VIDEO_OUT_NODE};
use crate::modules::types::{COLD_TYPE, CONSOLE_TYPE, completion_order, is_engine_ref, module_status_of};
use crate::modules::{IoKind, LoadedModule, ModuleKind, base_module_refs};
use crate::resolve::{InstrumentType, resolved_rack, resolved_type};
use crate::roots::{Roots, spell};
use crate::routing::{MIX_NODE, RENDER_NODE, derive_chains};
use crate::source::{Source, SourceError};
use indexmap::IndexMap;
use paths::{Names, paths_table};
use serde::Serialize;

/// What a built-in plugin's param spans.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ParamRange {
    pub min: f64,
    pub max: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub def: Option<f64>,
}

/// One pad of a rack and the note it answers to; `from` and `to` window a slice.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RackSymbol {
    pub symbol: String,
    pub note: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub from: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub to: Option<f64>,
}

/// A module's effective rack, for the note editors.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", ts(rename_all = "camelCase"))]
pub struct RackHint {
    #[cfg_attr(feature = "ts", ts(type = "\"samples\" | \"slice\""))]
    pub rack_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub source: Option<String>,
    pub symbols: Vec<RackSymbol>,
}

/// The completion vocabulary; every key is there when `hints_of` answers, and `empty` is the one without a project.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", ts(rename_all = "camelCase"))]
pub struct Hints {
    pub base_modules: Vec<String>,
    pub io_sinks: Vec<String>,
    pub modules: Vec<String>,
    pub tracks: Vec<String>,
    pub channels: Vec<String>,
    pub clips: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub audio_clips: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub video_clips: Option<Vec<String>>,
    pub video_tracks: Vec<String>,
    pub sequences: Vec<String>,
    pub chains: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub curves: Option<Vec<String>>,
    #[cfg_attr(feature = "ts", ts(type = "Record<string, string[]>"))]
    pub module_assets: IndexMap<String, Vec<String>>,
    #[cfg_attr(feature = "ts", ts(type = "Record<string, string[]>"))]
    pub params_by_base: IndexMap<String, Vec<String>>,
    #[cfg_attr(feature = "ts", ts(type = "Record<string, Record<string, ParamRange>>"))]
    pub param_meta_by_base: IndexMap<String, IndexMap<String, ParamRange>>,
    #[cfg_attr(feature = "ts", ts(type = "Record<string, RackHint>"))]
    pub racks: IndexMap<String, RackHint>,
    pub keywords: Vec<String>,
    #[cfg_attr(feature = "ts", ts(type = "Record<string, string[]>"))]
    pub plugin_names: IndexMap<String, Vec<String>>,
    #[cfg_attr(feature = "ts", ts(type = "Record<string, string>"))]
    pub paths: IndexMap<String, String>,
    pub project_root: String,
    #[cfg_attr(feature = "ts", ts(type = "Record<string, string[]>"))]
    pub media: IndexMap<String, Vec<String>>,
}

impl Hints {
    /// The vocabulary with no project open, or one that will not read: `EMPTY_HINTS`.
    pub fn empty() -> Hints {
        Hints {
            base_modules: Vec::new(),
            io_sinks: Vec::new(),
            modules: Vec::new(),
            tracks: Vec::new(),
            channels: Vec::new(),
            clips: Vec::new(),
            audio_clips: None,
            video_clips: None,
            video_tracks: Vec::new(),
            sequences: Vec::new(),
            chains: Vec::new(),
            curves: None,
            module_assets: IndexMap::new(),
            params_by_base: IndexMap::new(),
            param_meta_by_base: IndexMap::new(),
            racks: IndexMap::new(),
            keywords: Vec::new(),
            plugin_names: IndexMap::new(),
            paths: IndexMap::new(),
            project_root: String::new(),
            media: IndexMap::new(),
        }
    }

    /// An answer's starting point: every list there, the words k7x gives meaning to already in.
    fn answering(project: String) -> Hints {
        Hints {
            audio_clips: Some(Vec::new()),
            video_clips: Some(Vec::new()),
            curves: Some(Vec::new()),
            keywords: ["true", "false", "null"].map(str::to_owned).to_vec(),
            project_root: project,
            ..Hints::empty()
        }
    }
}

/// The vocabulary, what the walk found while reading, and whether the budget cut it short.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct HintsAnswer {
    pub hints: Hints,
    pub findings: Vec<Finding>,
    pub capped: bool,
    pub spent: usize,
}

/// Copies names into a list, each charged by its bytes; false, for good, once the budget ends.
fn copy_names<'a>(names: impl IntoIterator<Item = &'a str>, spend: &mut Spend, into: &mut Vec<String>) -> bool {
    for n in names {
        if !spend.bytes(n.len()) {
            return false;
        }
        into.push(n.to_owned());
    }
    true
}

fn status_of(m: Option<&LoadedModule>) -> &'static str {
    // `Status` is private to its module: its Debug spelling is the name the file wrote, capitalised.
    let status = m
        .and_then(|m| m.authored.status)
        .map(|s| format!("{s:?}").to_lowercase());
    module_status_of(status.as_deref())
}

fn sorted<'a>(names: impl Iterator<Item = &'a str>) -> Vec<&'a str> {
    let mut v: Vec<&str> = names.collect();
    v.sort_unstable();
    v
}

/// The sinks a chain can route to: the built-in outputs, then the vocabulary's io modules that are not inputs.
fn io_sinks(inputs: &Inputs, spend: &mut Spend, out: &mut Vec<String>) -> bool {
    let mut builtin = vec![MIX_NODE, OUT_NODE, CUE_NODE];
    if !inputs.lanes().picture.is_empty() {
        builtin.extend([RENDER_NODE, VIDEO_OUT_NODE]);
    }
    if !copy_names(builtin, spend, out) {
        return false;
    }
    let mut ios = Vec::new();
    for (k, m) in &inputs.vocabulary.modules {
        if !spend.step() {
            return false;
        }
        if m.kind == Some(ModuleKind::Io) && m.authored.io != Some(IoKind::In) {
            ios.push(k.as_str());
        }
    }
    ios.sort_unstable();
    copy_names(ios, spend, out)
}

/// The names a chain step may use: the project's nodes, then inherited modules, favourites first, hidden ones dropped.
fn module_names(inputs: &Inputs, spend: &mut Spend) -> Vec<String> {
    let nodes = sorted(inputs.nodes.nodes.keys().map(String::as_str));
    let mut shared = Vec::new();
    for (k, m) in &inputs.vocabulary.modules {
        if !spend.step() {
            return Vec::new();
        }
        let engine = is_engine_ref(k) || m.engine_of.is_some();
        let half = matches!(m.category.as_deref(), Some(CONSOLE_TYPE | COLD_TYPE));
        if k.starts_with('@') && !engine && !half {
            shared.push(k.as_str());
        }
    }
    shared.sort_unstable();
    let mut names = Vec::new();
    // What was copied before the budget ended is kept, in order.
    copy_names(nodes.into_iter().chain(shared), spend, &mut names);
    let lookup = |n: &String| status_of(inputs.nodes.nodes.get(n).or_else(|| inputs.vocabulary.modules.get(n)));
    completion_order(names, lookup)
}

/// Each module's own assets and its ancestors', nearest first (decision 631); false once the budget ends.
fn module_assets(source: &dyn Source, inputs: &Inputs, h: &mut Hints, spend: &mut Spend) -> Result<bool, SourceError> {
    const ANCESTRY: usize = 16;
    let mut assets = Assets::default();
    for (name, m) in &inputs.nodes.nodes {
        let Some(own) = m.folder_abs.as_deref() else { continue };
        let mut folders = vec![own];
        let mut r = m.authored.extends.as_deref();
        for _ in 0..ANCESTRY {
            let Some(parent) = r
                .filter(|r| !r.is_empty())
                .and_then(|r| inputs.vocabulary.modules.get(r))
            else {
                break;
            };
            folders.extend(parent.folder_abs.as_deref());
            r = parent.authored.extends.as_deref();
        }
        let mut held = Vec::new();
        for folder in folders {
            held.push(assets.assets_in(source, folder, spend)?);
            if spend.capped() {
                return Ok(false);
            }
        }
        let mut seen = std::collections::HashSet::new();
        let mut files: Vec<String> = Vec::new();
        for listed in &held {
            for f in listed.iter() {
                if !spend.step() {
                    return Ok(false);
                }
                if seen.insert(f.as_str()) {
                    if !spend.bytes(f.len()) {
                        return Ok(false);
                    }
                    files.push(f.clone());
                }
            }
        }
        if !files.is_empty() {
            h.module_assets.insert(name.clone(), files);
        }
    }
    Ok(true)
}

/// The effective rack of every resolved node that runs one, with the symbols that carry a note.
fn racks(inputs: &Inputs, h: &mut Hints, spend: &mut Spend) -> bool {
    for (name, m) in &inputs.local.modules {
        if !spend.step() {
            return false;
        }
        if resolved_type(Some(m)) != InstrumentType::Rack {
            continue;
        }
        let rack = resolved_rack(Some(m));
        let mut symbols = Vec::new();
        // Object.entries lists integer-like names first, ascending (decision 392).
        for (symbol, s) in own_key_entries(&rack.symbols) {
            if !spend.bytes(symbol.len()) {
                return false;
            }
            symbols.push(RackSymbol {
                symbol: symbol.clone(),
                note: s.note.0,
                from: s.from.as_ref().map(|v| v.0),
                to: s.to.as_ref().map(|v| v.0),
            });
        }
        if symbols.is_empty() {
            continue;
        }
        let rack_type = match rack.rack_type {
            crate::modules::RackType::Samples => "samples",
            crate::modules::RackType::Slice => "slice",
        };
        let source = rack.source.filter(|s| !s.is_empty());
        if !spend.bytes(name.len() + source.as_ref().map_or(0, String::len)) {
            return false;
        }
        h.racks.insert(
            name.clone(),
            RackHint {
                rack_type: rack_type.to_owned(),
                source,
                symbols,
            },
        );
    }
    true
}

/// The vocabulary, in the order of the TypeScript's `Hints`; a stage the budget ends leaves the later ones empty.
fn fill(
    source: &dyn Source,
    cache: &mut Cache,
    inputs: &Inputs,
    h: &mut Hints,
    spend: &mut Spend,
    media_spend: &mut Spend,
) -> Result<(), SourceError> {
    macro_rules! stage {
        ($ok:expr) => {
            if !$ok || spend.capped() {
                return Ok(());
            }
        };
    }
    for r in base_module_refs(&inputs.vocabulary.modules) {
        stage!(spend.step() && spend.bytes(r.len()));
        h.base_modules.push(r);
    }
    stage!(io_sinks(inputs, spend, &mut h.io_sinks));
    h.modules = module_names(inputs, spend);
    stage!(true);
    let lanes = inputs.lanes();
    stage!(copy_names(sorted(lanes.sound.keys().copied()), spend, &mut h.tracks));
    let labels = sorted(inputs.rig.channels_in.iter().map(|c| c.label.as_str()));
    stage!(copy_names(labels, spend, &mut h.channels));
    let clip_ids = || sorted(inputs.clips.values.iter().map(|(id, _)| id.as_str()));
    stage!(copy_names(clip_ids(), spend, &mut h.clips));
    let (mut audio, mut video) = (Vec::new(), Vec::new());
    for (id, c) in &inputs.clips.values {
        stage!(spend.step());
        if reads_as_picture(clip_source(c)) {
            video.push(id.as_str());
        } else {
            audio.push(id.as_str());
        }
    }
    stage!(copy_names(
        sorted(audio.into_iter()),
        spend,
        h.audio_clips.get_or_insert_with(Vec::new)
    ));
    stage!(copy_names(
        sorted(video.into_iter()),
        spend,
        h.video_clips.get_or_insert_with(Vec::new)
    ));
    stage!(copy_names(
        sorted(lanes.picture.keys().copied()),
        spend,
        &mut h.video_tracks
    ));
    stage!(copy_names(
        sorted(inputs.sequences.values.iter().map(|(id, _)| id.as_str())),
        spend,
        &mut h.sequences
    ));
    let chains = derive_chains(&inputs.routing.segments, &|_| false);
    stage!(copy_names(
        sorted(chains.keys().map(String::as_str)),
        spend,
        &mut h.chains
    ));
    let curves = read_curves(source, cache, &inputs.project)?;
    stage!(copy_names(
        sorted(curves.curves.keys().map(String::as_str)),
        spend,
        h.curves.get_or_insert_with(Vec::new)
    ));
    stage!(module_assets(source, inputs, h, spend)?);
    let bases = std::mem::take(&mut h.base_modules);
    params::params_by_base(h, inputs, &bases, spend);
    h.base_modules = bases;
    stage!(true);
    stage!(racks(inputs, h, spend));
    for (format, names) in inputs.vocabulary.scan.names() {
        stage!(spend.bytes(format.len()) && spend.steps(names.len()));
        let mut kept = Vec::new();
        stage!(copy_names(names.iter().map(String::as_str), spend, &mut kept));
        h.plugin_names.insert(format, kept);
    }
    let names = Names {
        tracks: &h.tracks,
        clips: &h.clips,
        sequences: &h.sequences,
        chains: &h.chains,
    };
    h.paths = paths_table(source, inputs, &names, spend)?;
    stage!(true);
    // The media walk pays for every name it lists on its own spend, so what it found is kept whatever the cap.
    h.media.extend(media_of(source, &inputs.project, media_spend)?);
    Ok(())
}

/// A project's completion vocabulary over what a compile reads (decisions 625 to 635); `Err` is `Missing` alone.
pub fn hints_of(
    source: &dyn Source,
    cache: &mut Cache,
    roots: &Roots,
    inputs: &Inputs,
) -> Result<HintsAnswer, SourceError> {
    let mut hints = Hints::answering(spell(&roots.project, roots.paths)?);
    let (mut spend, mut media) = (Spend::new(), Spend::new());
    fill(source, cache, inputs, &mut hints, &mut spend, &mut media)?;
    let mut findings = Vec::new();
    if spend.capped() {
        findings.push(capped_finding("project.k7x", "the completion vocabulary"));
    }
    if media.capped() {
        findings.push(capped_finding("project.k7x", "the media folders"));
    }
    Ok(HintsAnswer {
        hints,
        findings,
        capped: spend.capped() || media.capped(),
        spent: spend.spent() + media.spent(),
    })
}
