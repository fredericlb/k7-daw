// doc/plugin.rs — one step's PluginDoc: a rack's sampler, or a module's plugin with its files, levels and sidechain.
// A reader would get wrong that every copy is paid for BEFORE it is made: `cost` is an upper bound the budget is charged first.

use super::notes::{Notes, finding};
use super::own_mix::{
    BlendShape, INPUT_VOLUME_PARAM, MIX_PARAM, OUTPUT_VOLUME_PARAM, held_params, owns_blend, takes_input_volume,
    takes_output_volume,
};
use super::texts::{RackPath, Texts};
use super::{PluginDoc, RackSoundDoc, TapOutDoc, text_weight};
use crate::clips::Budget;
use crate::console::engine_plugin;
use crate::files::Severity;
use crate::jsorder::{own_key_entries, own_key_order};
use crate::lanes::inputs::Inputs;
use crate::lanes::ownership::Ownership;
use crate::modules::files::reaches_engine;
use crate::modules::preset::is_surge_patch;
use crate::modules::types::module_emits;
use crate::modules::{IoKind, ParamValue, PortType, RackType};
use crate::resolve::{InstrumentType, Resolved, resolved_rack, resolved_type};
use crate::routing::UseStep;
use crate::routing::tap_outs_for;
use crate::source::SourceError;
use crate::source::path::join;
use indexmap::IndexMap;
use std::collections::HashMap;

/// What a step became.
pub(super) enum Built {
    Skip,
    Plugin(Box<PluginDoc>),
    /// The doc budget is spent.
    Capped,
}

/// Everything one step is built from, borrowed from the pass.
pub(super) struct Cx<'c, 'p> {
    pub inputs: &'c Inputs,
    pub own: &'c Ownership,
    pub texts: &'p mut Texts<'c>,
    pub notes: &'p mut Notes,
    pub budget: &'p mut Budget,
    pub plugin_of_node: &'p mut HashMap<&'c str, String>,
}

fn io_word(io: IoKind) -> &'static str {
    match io {
        IoKind::Out => "out",
        IoKind::In => "in",
        IoKind::VideoOut => "video_out",
        IoKind::File => "file",
        IoKind::Camera => "camera",
        IoKind::Midi => "midi",
    }
}

/// How many bytes of names a message quotes before it says "and more".
const QUOTED: usize = 5;

fn names_of(names: &[&str]) -> String {
    let mut out: Vec<String> = names
        .iter()
        .take(QUOTED)
        .map(|n| format!("'{}'", n.chars().take(40).collect::<String>()))
        .collect();
    if names.len() > QUOTED {
        out.push(format!("and {} more", names.len() - QUOTED));
    }
    out.join(", ")
}

/// An upper bound of what building this step copies, in the budget's unit (decision 323).
fn cost(chain: &str, step: &UseStep, m: &Resolved, project: &str) -> usize {
    let a = &m.authored;
    let w = |s: &str| 1 + text_weight(s);
    let opt = |s: &Option<String>| s.as_deref().map_or(0, w);
    let params: usize = a.params.iter().flatten().map(|(k, _)| w(k)).sum::<usize>() + 3;
    let rack: usize = a
        .rack
        .iter()
        .flat_map(|r| r.symbols.iter())
        .map(|(k, e)| {
            w(k) + e.clip.as_ref().map_or(0, |c| w(&c.0))
                + a.rack.as_ref().and_then(|r| r.source.as_deref()).map_or(0, w)
                + text_weight(project)
        })
        .sum();
    let taps = a.kit.as_ref().map_or(0, |k| k.0.len()) + a.tap_outs.as_ref().map_or(0, |k| k.0.len());
    let faust_files: usize = a
        .params
        .iter()
        .flatten()
        .map(|(k, v)| match v {
            ParamValue::Text(t) => w(k) + w(t) + text_weight(project),
            ParamValue::Number(_) => 0,
        })
        .sum();
    let plugin = a.plugin.as_ref().map_or(0, |p| opt(&p.plugin_type) + opt(&p.name));
    w(chain)
        + plugin
        + opt(&m.category)
        + w(&step.module)
        + w(&step.module)
        + opt(&m.preset)
        + opt(&m.model)
        + opt(&m.ir)
        + opt(&m.onnx)
        + w(project) * 2
        + w(&m.file)
        + opt(&m.node)
        + opt(&m.home)
        + opt(&m.preset_rel)
        + params
        + rack
        + taps
        + faust_files
        + 8
}

fn state_of(plugin: Option<&crate::modules::Plugin>) -> String {
    plugin
        .and_then(|p| p.plugin_type.clone())
        .unwrap_or_default()
        .to_lowercase()
}

/// A step's PluginDoc: a rack's sampler or a module's plugin; Skip for what the engine does not play (decision 312).
pub(super) fn step_plugin<'c>(
    cx: &mut Cx<'c, '_>,
    chain: &str,
    step: &'c UseStep,
    m: &'c Resolved,
    enabled: bool,
) -> Result<Built, SourceError> {
    let label = m.own_label();
    if !cx.budget.charge(cost(chain, step, m, &cx.inputs.project)) {
        return Ok(Built::Capped);
    }
    let id = format!("{chain}:{}", step.module);
    if resolved_type(Some(m)) == InstrumentType::Rack {
        return rack_plugin(cx, id, step, m, &label, enabled);
    }
    let shape = BlendShape::of(m);
    let params_keys = m.authored.params.iter().flatten().map(|(k, _)| k.as_str());
    let held = held_params(Some(shape), params_keys);
    if !held.is_empty() {
        cx.notes.stop_at(
            &label,
            format!(
                "module '{}': params set {} — the engine's own stage holds these: state the blend as `mix:` and the levels as `input_volume:` / `output_volume:` (dB) instead",
                step.module.chars().take(40).collect::<String>(),
                names_of(&held)
            ),
        );
        return Ok(Built::Skip);
    }
    let plugin = m.authored.plugin.as_ref();
    let raw = state_of(plugin);
    if raw == "offline" || !reaches_engine(&raw) {
        return Ok(Built::Skip);
    }
    let emits = module_emits(m.category.as_deref(), m.authored.io.map(io_word), m.outputs()).map(|p| match p {
        PortType::Audio => "audio",
        PortType::Midi => "midi",
        PortType::Video => "video",
    });
    let (kind, name) = engine_plugin(plugin, &step.module, emits);
    if kind.is_empty() {
        return Ok(Built::Skip);
    }
    let mut doc = PluginDoc::new(&id);
    doc.plugin_type = kind;
    doc.name = name;
    let surge = is_surge_patch(m.preset.as_deref());
    doc.path = if surge {
        String::new()
    } else {
        m.preset.clone().unwrap_or_default()
    };
    if surge {
        doc.surge_patch = m.preset.clone();
    }
    doc.nam_file = m.model.clone().filter(|s| !s.is_empty());
    doc.ir_file = m.ir.clone().filter(|s| !s.is_empty());
    doc.sfz_file = match m.authored.params.as_ref().and_then(|p| p.get("sfz")) {
        Some(ParamValue::Text(t)) if !t.is_empty() => Some(t.clone()),
        _ => None,
    };
    doc.tap_outs = tap_outs(cx, &id, step, m, &label);
    if let Some(src) = m.jsdsp.as_deref().filter(|s| !s.is_empty()) {
        let g = cx.texts.graph(src, &label, &step.module, cx.notes)?;
        if !cx.budget.charge(1 + text_weight(&g.text)) {
            return Ok(Built::Capped);
        }
        doc.dsp_graph = Some(g.text.to_string());
    }
    if let Some(src) = m.faust.as_deref().filter(|s| !s.is_empty())
        && let Some(text) = cx.texts.faust_text(src, &label, cx.notes)?
    {
        if !cx.budget.charge(1 + text_weight(&text)) {
            return Ok(Built::Capped);
        }
        doc.faust_dsp = Some(text.to_string());
        doc.faust_files = faust_files(cx, m)?;
    }
    let mut seen = 0;
    let own = cx.own;
    let side = step.inputs.iter().flatten().find_map(|i| {
        seen += 1;
        own.side_input_track(i)
    });
    if !cx.budget.charge(seen + side.map_or(0, text_weight)) {
        return Ok(Built::Capped);
    }
    doc.sidechain_from = side.map(str::to_owned);
    doc.preset_save = preset_save(&cx.inputs.project, m, &step.module);
    doc.role = String::new();
    doc.enabled = enabled;
    doc.params = params_of(cx, m, shape, &label);
    Ok(Built::Plugin(Box::new(doc)))
}

/// Where the engine writes the state of this step (decision 319); a category of none is the word `undefined`.
fn preset_save(project: &str, m: &Resolved, module: &str) -> String {
    if let Some(rel) = m.preset_rel.as_deref().filter(|s| !s.is_empty()) {
        return join(project, rel);
    }
    if let Some(home) = m.home.as_deref().filter(|s| !s.is_empty()) {
        return join(
            project,
            &format!("{home}/{}.vstpreset", m.node.as_deref().unwrap_or("undefined")),
        );
    }
    let file = if m.file.is_empty() { module } else { &m.file };
    let leaf = m.node.as_deref().filter(|s| !s.is_empty()).unwrap_or(file);
    let category = m.category.as_deref().unwrap_or("undefined");
    join(project, &format!("modules/{category}/{file}/{leaf}.vstpreset"))
}

fn params_of(cx: &mut Cx<'_, '_>, m: &Resolved, shape: BlendShape<'_>, label: &str) -> IndexMap<String, f64> {
    let mut out: IndexMap<String, f64> = IndexMap::new();
    for (k, v) in m.authored.params.iter().flatten() {
        if let ParamValue::Number(n) = v {
            if n.is_finite() {
                out.insert(k.clone(), *n);
            } else {
                cx.notes
                    .stop_at(label, "a parameter of this module is not a finite number");
            }
        }
    }
    let mix = m.authored.mix.as_ref().map(|u| u.0);
    if owns_blend(Some(shape)) {
        out.insert(MIX_PARAM.to_owned(), mix.unwrap_or(1.0));
    } else if let Some(v) = mix {
        out.insert(MIX_PARAM.to_owned(), v);
    }
    if takes_input_volume(Some(shape)) {
        out.insert(
            INPUT_VOLUME_PARAM.to_owned(),
            m.authored.input_volume.as_ref().map_or(0.0, |v| v.0),
        );
    }
    if takes_output_volume(Some(shape)) {
        out.insert(
            OUTPUT_VOLUME_PARAM.to_owned(),
            m.authored.output_volume.as_ref().map_or(0.0, |v| v.0),
        );
    }
    own_key_order(&mut out);
    out
}

fn faust_files(cx: &mut Cx<'_, '_>, m: &Resolved) -> Result<Option<IndexMap<String, String>>, SourceError> {
    let mut out = IndexMap::new();
    for (slot, v) in m.authored.params.iter().flatten() {
        let ParamValue::Text(r) = v else { continue };
        if r.is_empty() {
            continue;
        }
        if let Some(f) = cx.texts.faust_file(r)? {
            out.insert(slot.clone(), f);
        }
    }
    own_key_order(&mut out);
    Ok((!out.is_empty()).then_some(out))
}

/// The pairs a node's routed taps leave by; a tap the node does not have is one ERROR on its file (decision 320).
fn tap_outs<'c>(cx: &mut Cx<'c, '_>, id: &str, step: &'c UseStep, m: &Resolved, label: &str) -> Option<Vec<TapOutDoc>> {
    let routed = cx.inputs.routed_taps.get(&step.module).filter(|r| !r.is_empty())?;
    cx.plugin_of_node.insert(step.module.as_str(), id.to_owned());
    let rack = resolved_type(Some(m)) == InstrumentType::Rack;
    let symbols = rack.then(|| resolved_rack(Some(m)).symbols);
    let note_of = |tap: &str| match &symbols {
        Some(s) => s.get(tap).map(|e| e.note.0),
        None => m.authored.kit.as_ref().and_then(|k| k.0.get(tap)).map(|k| k.0),
    };
    let mut out = Vec::new();
    for t in routed {
        match tap_outs_for(&step.module, std::slice::from_ref(t), &note_of) {
            Ok(list) => out.extend(list.into_iter().map(|o| TapOutDoc {
                note: o.note,
                pair: o.pair,
            })),
            Err(message) => cx.notes.stops(finding(label, Severity::Error, message)),
        }
    }
    Some(out)
}

fn rack_plugin<'c>(
    cx: &mut Cx<'c, '_>,
    id: String,
    step: &'c UseStep,
    m: &'c Resolved,
    label: &str,
    enabled: bool,
) -> Result<Built, SourceError> {
    let rack = resolved_rack(Some(m));
    let slice = rack.rack_type == RackType::Slice;
    let source = match rack.source.as_deref().filter(|_| slice) {
        Some(p) => cx.texts.rack_path(p)?,
        None => RackPath::Empty,
    };
    let mut sounds = Vec::new();
    for (symbol, e) in own_key_entries(&rack.symbols) {
        if e.mute == Some(true) {
            continue;
        }
        let note = e.note.0;
        if slice {
            let RackPath::Kept(clip) = &source else {
                refused(cx, &source, label);
                continue;
            };
            let from = e.from.as_ref().map_or(0.0, |v| v.0);
            let to = e.to.as_ref().map_or(0.0, |v| v.0);
            sounds.push(RackSoundDoc {
                symbol: symbol.clone(),
                note,
                clip: clip.clone(),
                start: from.max(0.0),
                length: (to - from).max(0.0),
            });
        } else {
            let path = cx.texts.rack_path(e.clip.as_ref().map_or("", |c| c.0.as_str()))?;
            let RackPath::Kept(clip) = path else {
                refused(cx, &path, label);
                continue;
            };
            sounds.push(RackSoundDoc {
                symbol: symbol.clone(),
                note,
                clip,
                start: 0.0,
                length: 0.0,
            });
        }
    }
    let mut doc = PluginDoc::new(&id);
    doc.plugin_type = "sampler".to_owned();
    doc.name = step.module.clone();
    doc.role = "instrument".to_owned();
    doc.enabled = enabled;
    doc.rack = Some(sounds);
    cx.plugin_of_node.insert(step.module.as_str(), id.clone());
    doc.tap_outs = tap_outs(cx, &id, step, m, label);
    Ok(Built::Plugin(Box::new(doc)))
}

fn refused(cx: &mut Cx<'_, '_>, path: &RackPath, label: &str) {
    if *path == RackPath::Outside {
        cx.notes.push(finding(
            label,
            Severity::Warning,
            "a sound of this rack lies outside the project's folders, so it is left out",
        ));
    }
}
