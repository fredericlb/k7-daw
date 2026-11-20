// resolve/alternatives.rs — a module's other attribute SETS: which it has, which is on, and a module once the chosen one is applied.
// A reader would get wrong that a set naming another module SWAPS the node into it; where that is refused it is a patch, its `extends` ignored.

use super::{Resolver, weigh};
use crate::files::scalar::js_space;
use crate::modules::{Alternative, AlternativePick, LoadedModule, Module, ModuleKind, merge_authored};

/// The mixer's fader: with a lane, what the arrangement and the mixer stand on, so a set may not swap it.
pub const MIXER_PLUGIN: &str = "Mixer";

/// One entry as a surface lists it: what to call it, and where it sits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlternativeInfo {
    pub name: String,
    /// 1-based; 1 is the module's own fields, which are the first alternative.
    pub at: usize,
}

/// Every alternative a module has, the module's OWN fields first and answering to "1".
pub fn alternatives_of(m: &Module) -> Vec<AlternativeInfo> {
    let mut out = vec![AlternativeInfo {
        name: "1".to_owned(),
        at: 1,
    }];
    for (i, a) in m.alternatives.iter().flatten().enumerate() {
        out.push(AlternativeInfo {
            name: a.name.as_ref().map_or_else(|| (i + 2).to_string(), |n| n.0.clone()),
            at: i + 2,
        });
    }
    out
}

/// Which one `want` names, a NAME winning over a number and a miss falling back to the first.
pub fn pick_alternative(names: &[String], want: Option<&AlternativePick>) -> usize {
    let Some(want) = want else { return 0 };
    let s = match want {
        AlternativePick::Name(n) => n.clone(),
        AlternativePick::Index(i) => i.to_string(),
    };
    if let Some(at) = names.iter().position(|n| *n == s) {
        return at;
    }
    match js_number(&s) {
        Some(n) if n.fract() == 0.0 && n >= 1.0 && n <= names.len() as f64 => n as usize - 1,
        _ => 0,
    }
}

/// JavaScript's `Number(s)` for the cases that can be a whole number: space around it, a decimal, an exponent, `0x`, `0o`, `0b`.
fn js_number(s: &str) -> Option<f64> {
    let t = s.trim_matches(js_space);
    let radix = |prefix: &str, base: u32| {
        let digits = t
            .strip_prefix(prefix)
            .or_else(|| t.strip_prefix(&prefix.to_uppercase()))?;
        u64::from_str_radix(digits, base).ok().map(|n| n as f64)
    };
    radix("0x", 16)
        .or_else(|| radix("0o", 8))
        .or_else(|| radix("0b", 2))
        .or_else(|| t.parse::<f64>().ok().filter(|n| n.is_finite()))
}

/// Which alternative is ON, as an index into `alternatives_of`.
pub fn current_alternative(m: &Module) -> usize {
    let names: Vec<String> = alternatives_of(m).into_iter().map(|a| a.name).collect();
    pick_alternative(&names, m.alternative.as_ref())
}

/// A lane and a lane's fader are what the arrangement and the mixer are built on.
pub fn keeps_its_module(m: &LoadedModule) -> bool {
    m.kind == Some(ModuleKind::Track)
        || m.authored.plugin.as_ref().and_then(|p| p.name.as_deref()) == Some(MIXER_PLUGIN)
}

/// The selected set, or None when the module's own fields are on.
fn chosen_set(m: &Module) -> Option<&Alternative> {
    let at = current_alternative(m);
    if at == 0 {
        return None;
    }
    m.alternatives.as_ref()?.get(at - 1)
}

/// A set's fields as a module's, minus its `name` and `extends`.
fn patch_of(a: &Alternative) -> Module {
    Module {
        plugin: a.plugin.clone(),
        io: a.io,
        inputs: a.inputs.clone(),
        outputs: a.outputs.clone(),
        module_type: a.module_type,
        shader: a.shader.clone(),
        shape: a.shape,
        font: a.font.clone(),
        align: a.align,
        background: a.background.clone(),
        shadow: a.shadow,
        params: a.params.clone(),
        rack: a.rack.clone(),
        tap_outs: a.tap_outs.clone(),
        kit: a.kit.clone(),
        cold: a.cold,
        model_url: a.model_url.clone(),
        channel: a.channel.clone(),
        camera: a.camera.clone(),
        device: a.device.clone(),
        frame: a.frame,
        source: a.source.clone(),
        monitor: a.monitor.clone(),
        gain: a.gain,
        mix: a.mix,
        input_volume: a.input_volume,
        output_volume: a.output_volume,
        base: a.base.clone(),
        ui: a.ui.clone(),
        automations: a.automations.clone(),
        content: a.content.clone(),
        events: a.events.clone(),
        color: a.color.clone(),
        disabled: a.disabled,
        tracks: a.tracks.clone(),
        strudel: a.strudel.clone(),
        video_source: a.video_source.clone(),
        lut: a.lut.clone(),
        frozen: a.frozen.clone(),
        engine: a.engine.clone(),
        engines: a.engines.clone(),
        ..Module::default()
    }
}

impl Resolver<'_> {
    /// `own` with its selected alternative merged over its own fields: a PATCH, its `extends` ignored.
    pub(super) fn apply_alternative(&mut self, own: LoadedModule) -> LoadedModule {
        let Some(set) = chosen_set(&own.authored) else {
            return own;
        };
        let patch = patch_of(set);
        let spent = weigh(&own).saturating_add(patch.params.as_ref().map_or(0, indexmap::IndexMap::len));
        if !self.charge(spent) {
            return own;
        }
        LoadedModule {
            authored: merge_authored(&own.authored, &patch),
            ..own
        }
    }

    /// The node as ANOTHER module, when its selected set names one: its identity kept, set "1"'s fields not.
    pub(super) fn swapped(&self, m: &LoadedModule) -> Option<LoadedModule> {
        swapped_module(m)
    }
}

/// The node as ANOTHER module, when its selected set names one: its identity kept, set "1"'s fields not.
pub fn swapped_module(m: &LoadedModule) -> Option<LoadedModule> {
    let set = chosen_set(&m.authored)?;
    let target = set.extends.as_ref().filter(|e| !e.0.is_empty())?;
    let mut authored = patch_of(set);
    authored.extends = Some(target.0.clone());
    authored.alternatives = m.authored.alternatives.clone();
    authored.alternative = m.authored.alternative.clone();
    let mut out = LoadedModule::bare(ModuleKind::Plugin, "", &m.file);
    out.kind = None;
    out.category = None;
    out.name = m.name.clone();
    out.node = m.node.clone();
    out.home = m.home.clone();
    out.chain = m.chain.clone();
    out.node_id = m.node_id.clone();
    out.preset_rel = m.preset_rel.clone();
    out.folder_abs = m.folder_abs.clone();
    out.authored = authored;
    Some(out)
}
