// console.rs — a summing CONSOLE over the whole mix: a system is a folder of a library's `console/` holding a `channel` and a `buss`.
// A reader would get wrong that nothing is placed here: 2h edits the document, and clones a `HalfPlugin` per track (279).
// A system with a half missing is one fault, not a throw (280); docs/console.md has the rationale.

use crate::files::Finding;
use crate::lanes::fault::{Fault, FaultKind, Holder};
use crate::modules::types::{CONSOLE_TYPE, DSP_TYPE, JSDSP_TYPE};
use crate::modules::{ParamValue, Plugin, Vocabulary};
use crate::resolve::{MIXER_PLUGIN, Resolved, resolve_ref};
use crate::roots::Roots;
use crate::source::{Source, SourceError};
use indexmap::IndexMap;
use serde::Serialize;

/// What the halves' plugin ids start with: `@` begins a ref and never a node name, so no chain step can produce one.
pub const CONSOLE_ID: &str = "@console:";

/// The two halves a system folder holds, by their folder name.
pub const CONSOLE_HALVES: [&str; 2] = ["channel", "buss"];

const DSP_GRAPH_PLUGIN: &str = "DSP Graph";
const MIDI_GRAPH_PLUGIN: &str = "MIDI Graph";
const FAUST_TYPE: &str = "faust";
const FAUST_PLUGIN: &str = "Faust";

/// A system as a picker lists it: the ref a project names, and what to call it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ConsoleSystem {
    #[serde(rename = "ref")]
    pub ref_: String,
    pub label: String,
}

/// The ref of a console system's half: `@k7-core/console/console-6` gives `…/channel`.
fn half_ref(system: &str, half: &str) -> String {
    format!("{system}/{half}")
}

/// `@<lib>/console/<system>` when `r` is `@<lib>/console/<system>/channel`, the regexp of the TypeScript read by hand.
fn system_of(r: &str) -> Option<&str> {
    let system = r.strip_suffix("/channel")?;
    let mut parts = system.split('/');
    let (lib, mid, name, more) = (parts.next()?, parts.next()?, parts.next()?, parts.next());
    let shaped = more.is_none() && mid == CONSOLE_TYPE && lib.len() > 1 && lib.starts_with('@') && !name.is_empty();
    shaped.then_some(system)
}

/// Every system the vocabulary holds: a `console/<system>/` with BOTH halves in it, sorted by label in code points (279).
pub fn console_systems(vocab: &Vocabulary) -> Vec<ConsoleSystem> {
    let mut out: Vec<ConsoleSystem> = vocab
        .modules
        .keys()
        .filter_map(|r| system_of(r))
        .filter(|system| vocab.modules.contains_key(&half_ref(system, "buss")))
        .map(|system| ConsoleSystem {
            ref_: system.to_owned(),
            label: system
                .rsplit('/')
                .next()
                .filter(|l| !l.is_empty())
                .unwrap_or(system)
                .to_owned(),
        })
        .collect();
    out.sort_by(|a, b| a.label.cmp(&b.label));
    out
}

/// Maps an authored `plugin: {type, name}` onto the engine's older wire form, `fallback` being the module name (docs/enginePlugin.md).
pub fn engine_plugin(plugin: Option<&Plugin>, fallback: &str, emits: Option<&str>) -> (String, String) {
    let kind = plugin.and_then(|p| p.plugin_type.as_deref()).unwrap_or("");
    let name = plugin.and_then(|p| p.name.as_deref()).unwrap_or("");
    // `jsdsp` is what the AUTHOR writes; the engine loads the graph host and is handed the compiled graph.
    if kind == JSDSP_TYPE || kind == DSP_TYPE {
        let host = if emits == Some("midi") {
            MIDI_GRAPH_PLUGIN
        } else {
            DSP_GRAPH_PLUGIN
        };
        return (host.to_owned(), fallback.to_owned());
    }
    if kind == FAUST_TYPE {
        return (FAUST_PLUGIN.to_owned(), fallback.to_owned());
    }
    if kind == "internal" {
        return (name.to_owned(), fallback.to_owned());
    }
    // An external format, or the legacy form where `type` is the xmlTypeName itself.
    let shown = if name.is_empty() { fallback } else { name };
    (kind.to_owned(), shown.to_owned())
}

/// One half as the plugin the engine runs, under a `CONSOLE_ID` id: every key of the `PluginDoc` `halfPlugin` writes.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct HalfPlugin {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub name: String,
    pub path: String,
    #[serde(rename = "presetSave")]
    pub preset_save: String,
    pub role: String,
    pub enabled: bool,
    pub params: IndexMap<String, f64>,
    /// Whether the engine name is the plugin's own id (a jsdsp, faust or internal half, or one with no name).
    #[serde(skip)]
    named_by_id: bool,
}

impl HalfPlugin {
    /// The half for one track (`@console:<track>`), or for the master (`@console:master`): a clone with its own id.
    pub fn with_id(&self, track: &str) -> HalfPlugin {
        let id = format!("{CONSOLE_ID}{track}");
        HalfPlugin {
            name: if self.named_by_id {
                id.clone()
            } else {
                self.name.clone()
            },
            id,
            ..self.clone()
        }
    }
}

fn half_plugin(module: &Resolved) -> HalfPlugin {
    let (kind, name) = engine_plugin(module.authored.plugin.as_ref(), CONSOLE_ID, Some("audio"));
    let params = module
        .authored
        .params
        .iter()
        .flatten()
        .filter_map(|(k, v)| match v {
            ParamValue::Number(n) => Some((k.clone(), *n)),
            ParamValue::Text(_) => None,
        })
        .collect();
    let named_by_id = engine_plugin(module.authored.plugin.as_ref(), "", Some("audio"))
        .1
        .is_empty();
    HalfPlugin {
        id: CONSOLE_ID.to_owned(),
        kind,
        name,
        named_by_id,
        path: module.preset.clone().unwrap_or_default(),
        preset_save: String::new(),
        role: String::new(),
        enabled: true,
        params,
    }
}

/// What the project's console asks for: the two halves, or the fault that names the one missing (280).
#[derive(Debug, Default)]
pub struct Console {
    pub channel: Option<HalfPlugin>,
    pub buss: Option<HalfPlugin>,
    pub fault: Option<Fault>,
    pub findings: Vec<Finding>,
}

/// The two halves of `system` resolved; refused, like any unresolved ref, when the system is not there.
pub fn console_of(
    source: &dyn Source,
    roots: &Roots,
    vocab: &Vocabulary,
    system: Option<&str>,
) -> Result<Console, SourceError> {
    let Some(system) = system.filter(|s| !s.is_empty()) else {
        return Ok(Console::default());
    };
    let mut out = Console::default();
    let mut halves = Vec::new();
    for half in CONSOLE_HALVES {
        let (module, mut findings) = resolve_ref(source, roots, vocab, &half_ref(system, half))?;
        out.findings.append(&mut findings);
        halves.push((half, module));
    }
    if let Some((half, _)) = halves.iter().find(|(_, m)| m.is_none()) {
        out.fault = Some(Fault::new(FaultKind::ConsoleAbsent, Holder::Console, half));
        return Ok(out);
    }
    let [(_, channel), (_, buss)] = <[_; 2]>::try_from(halves).unwrap_or_default();
    out.channel = channel.as_ref().map(half_plugin);
    out.buss = buss.as_ref().map(half_plugin);
    Ok(out)
}

/// Where a lane's channel half goes: right after the strip, so the fader and the pan act before the encode; the end when it has none.
pub fn channel_slot(plugin_types: &[&str]) -> usize {
    plugin_types
        .iter()
        .rposition(|t| *t == MIXER_PLUGIN)
        .map_or(plugin_types.len(), |at| at + 1)
}

/// Whether a track takes a channel half: a lane leaving by CUE never reaches the sum, and a bus return was encoded by the lanes sending to it.
pub fn takes_channel(output_channels: &[u32], role: &str) -> bool {
    output_channels.is_empty() && role != "effect"
}
