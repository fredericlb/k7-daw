// graph/view/flags.rs — what a step says about its plugin: whether it has a slot, whether k7 can load it, which stage it takes, what it plays at.
// A reader would get wrong that a shader, a shape and a transition ARE their behaviour: no slot, so an absent `plugin:` is not a hole.

use crate::graph::GraphModule;
use crate::modules::files::is_external_format;
use crate::modules::types::{DSP_TYPE, JSDSP_TYPE, hosts_plugin};
use crate::modules::{ModuleKind, Scan, Tab, Widget, builtin_plugin};
use crate::resolve::{MIXER_PLUGIN, ResolvedUi, resolved_type};
use indexmap::IndexMap;
use std::collections::HashSet;

pub const OFFLINE_TYPE: &str = "offline";
pub const INPUT_VOLUME_PARAM: &str = "@input-volume";
pub const MIX_PARAM: &str = "@mix";
pub const OUTPUT_VOLUME_PARAM: &str = "@output-volume";

/// What a module is as a plugin host, found once per module.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PluginFlags {
    pub slot: bool,
    pub stub: bool,
    pub not_supported: bool,
    pub unknown: bool,
    pub native_ui: bool,
    pub offline: bool,
}

/// The lower-cased format of a module's plugin, or empty.
fn format_of(m: &GraphModule<'_>) -> String {
    m.m.authored
        .plugin
        .as_ref()
        .and_then(|p| p.plugin_type.as_deref())
        .map(str::to_lowercase)
        .unwrap_or_default()
}

/// The flags of a module (decision table "What to decide as the TypeScript does").
pub fn flags_of(m: &GraphModule<'_>, scan: &Scan) -> PluginFlags {
    let plugin = m.m.authored.plugin.as_ref();
    let slot = m.kind == ModuleKind::Plugin && hosts_plugin(m.category);
    let fmt = format_of(m);
    let rack = resolved_type(Some(m.m)) == crate::resolve::InstrumentType::Rack;
    let stub = slot && !rack && plugin.is_none();
    let external = is_external_format(&fmt);
    let known = fmt == "internal" || fmt == JSDSP_TYPE || fmt == DSP_TYPE || fmt == OFFLINE_TYPE || external;
    let not_supported = slot && fmt == "not_supported";
    let name = plugin.and_then(|p| p.name.as_deref()).unwrap_or("");
    let unscanned = slot && external && !scan.is_plugin_scanned(&fmt, name);
    let unknown = slot && plugin.is_some() && !not_supported && (!known || unscanned);
    PluginFlags {
        slot,
        stub,
        not_supported,
        unknown,
        native_ui: slot && plugin.is_some() && !not_supported && !unknown && external,
        offline: fmt == OFFLINE_TYPE,
    }
}

/// Whether the engine owns a module's blend: an effect with at most one declared input.
fn owns_blend(m: &GraphModule<'_>) -> bool {
    m.kind == ModuleKind::Plugin && m.category == Some("audio") && m.inputs.len() <= 1
}

fn takes_input_volume(m: &GraphModule<'_>) -> bool {
    m.kind == ModuleKind::Plugin && m.category == Some("audio")
}

fn takes_output_volume(m: &GraphModule<'_>) -> bool {
    m.kind == ModuleKind::Plugin && matches!(m.category, Some("audio" | "instruments"))
}

/// The engine's stage a module takes, in the order its card's footer draws them (427).
pub fn stage_of(m: &GraphModule<'_>) -> Vec<String> {
    let mut out = Vec::new();
    if takes_input_volume(m) {
        out.push(INPUT_VOLUME_PARAM.to_owned());
    }
    if owns_blend(m) {
        out.push(MIX_PARAM.to_owned());
    }
    if takes_output_volume(m) {
        out.push(OUTPUT_VOLUME_PARAM.to_owned());
    }
    out
}

/// A strip carries its own Volume, and an offline module is no live stage: neither takes the engine's.
pub fn takes_stage(m: &GraphModule<'_>) -> bool {
    let plugin = m.m.authored.plugin.as_ref();
    plugin.and_then(|p| p.plugin_type.as_deref()) != Some(OFFLINE_TYPE)
        && plugin.and_then(|p| p.name.as_deref()) != Some(MIXER_PLUGIN)
}

/// The panel a module SHOWS, for the step: hidden widgets dropped.
pub fn shown_ui(m: &GraphModule<'_>) -> Option<ResolvedUi> {
    crate::resolve::resolved_ui(Some(m.m))
}

/// The params the widgets of one face of a panel bind or pair.
fn take<'a>(bound: &mut HashSet<&'a str>, ws: &'a IndexMap<String, Widget>, tabs: &'a Option<Vec<Tab>>) {
    for w in ws
        .values()
        .chain(tabs.iter().flatten().flat_map(|t| t.widgets.values()))
    {
        for b in [w.bind.as_ref(), w.pair.as_ref()].into_iter().flatten() {
            bound.insert(&b.0);
        }
    }
}

/// What each param a panel binds plays at unset, for a BUILT-IN plugin, both faces of the panel counting.
pub fn param_defaults(m: &GraphModule<'_>, ui: &ResolvedUi) -> Option<IndexMap<String, f64>> {
    let plugin = m.m.authored.plugin.as_ref()?;
    let builtin = builtin_plugin(plugin.plugin_type.as_deref(), plugin.name.as_deref())?;
    if builtin.params.is_empty() {
        return None;
    }
    let mut bound: HashSet<&str> = HashSet::new();
    take(&mut bound, &ui.widgets, &ui.tabs);
    if let Some(v) = &ui.vertical {
        take(&mut bound, &v.widgets, &v.tabs);
    }
    let out: IndexMap<String, f64> = builtin
        .params
        .iter()
        .filter_map(|p| {
            p.def
                .filter(|_| bound.contains(p.name.as_str()))
                .map(|d| (p.name.clone(), d))
        })
        .collect();
    (!out.is_empty()).then_some(out)
}
