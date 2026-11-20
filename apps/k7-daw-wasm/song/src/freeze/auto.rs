// freeze/auto.rs — the automatic freezes: the last VST of each run, its render named by a fingerprint of its inputs.
// A reader would get wrong that the compile writes nothing here: `ready` is a stat and the host alone touches a render (decision 347).
// Decisions 339 to 341, 346 to 348; docs/autoFreeze.md.

use super::fingerprint::Fingerprinter;
use super::{Edges, FreezePlan, freeze_cut, freeze_plans, plugin_module};
use crate::cache::Cache;
use crate::clips::Budget;
use crate::doc::ProjectDoc;
use crate::modules::types::hosts_plugin;
use crate::modules::{LoadedModule, ModuleKind, Scan};
use crate::roots::spell;
use crate::source::path::{PathRule, join};
use crate::source::{Source, SourceError};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// The plugin formats the engine loads from disk, as `EXTERNAL_PLUGIN_FORMATS` of engine-protocol spells them.
const EXTERNAL_FORMATS: [&str; 9] = ["vst3", "vst", "vst2", "au", "audiounit", "clap", "lv2", "ladspa", "aax"];

/// A plugin the render needs its rendering engine to have.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FreezePlugin {
    pub format: String,
    pub name: String,
}

/// One automatic freeze: where it is taken, what it bakes, and the render it would play (decision 359: sets are arrays).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutoFreeze {
    pub module: String,
    pub render: String,
    pub plan: FreezePlan,
    pub cut: ProjectDoc,
    pub plugins: Vec<FreezePlugin>,
    pub ready: bool,
    pub hosted: bool,
}

impl AutoFreeze {
    /// Whether the render stands in for its modules: made, and the engine cannot host them itself.
    pub fn plays_render(&self) -> bool {
        self.ready && !self.hosted
    }
}

/// What the automatic freezes read: the graph, the modules, the live document and the engine's scan.
pub struct AutoInput<'a> {
    pub project: &'a str,
    pub renders: Option<&'a str>,
    pub edges: &'a Edges<'a>,
    /// The module keys, local then extra, in that order.
    pub names: &'a [&'a str],
    pub module: &'a dyn Fn(&str) -> Option<&'a LoadedModule>,
    /// A sound lane, not disabled.
    pub is_lane: &'a dyn Fn(&str) -> bool,
    pub is_strudel: &'a dyn Fn(&str) -> bool,
    pub manual: &'a [FreezePlan],
    pub thawed: &'a HashSet<String>,
    pub scan: &'a Scan,
    pub no_vsts: bool,
    pub live: &'a ProjectDoc,
    pub rule: PathRule,
}

/// The automatic freezes of a compile; `capped` says the budget stopped them and there are none.
#[derive(Clone, Debug, PartialEq)]
pub struct AutoFreezes {
    pub freezes: Vec<AutoFreeze>,
    pub capped: bool,
}

/// Is this module a VST: an external plugin format in a plugin slot? A module with no kind is a plugin.
pub fn is_vst_module(m: Option<&LoadedModule>) -> bool {
    let Some(m) = m else { return false };
    if !matches!(m.kind, None | Some(ModuleKind::Plugin)) || !hosts_plugin(m.category.as_deref()) {
        return false;
    }
    let Some(plugin) = &m.authored.plugin else { return false };
    let format = plugin.plugin_type.as_deref().unwrap_or_default().to_lowercase();
    EXTERNAL_FORMATS.contains(&format.as_str()) && plugin.name.as_deref().is_some_and(|n| !n.is_empty())
}

/// A module name as a file name: `:`, `/` and `\` become `-` on every OS.
fn file_safe(module: &str) -> String {
    module.replace([':', '/', '\\'], "-")
}

/// The render file an automatic freeze of `module` with `fingerprint` plays, under `renders` (decision 346).
pub fn auto_render_path(renders: &str, module: &str, fingerprint: &str) -> String {
    join(renders, &format!("{}-{fingerprint}.wav", file_safe(module)))
}

/// Whether a render is there: `spell`, then a stat of any kind; a refused path is not (decision 335). `Missing` is the Err.
pub fn render_ready(source: &dyn Source, path: &str, rule: PathRule) -> Result<bool, SourceError> {
    let Ok(at) = spell(path, rule) else { return Ok(false) };
    match source.stat(&at) {
        Ok(_) => Ok(true),
        Err(SourceError::Missing) => Err(SourceError::Missing),
        Err(_) => Ok(false),
    }
}

/// Where to freeze: the LAST VST of each run, so one render covers every VST before it. None: the budget is spent.
pub fn auto_freeze_points(input: &AutoInput<'_>, budget: &mut Budget) -> Option<Vec<String>> {
    if !budget.charge(input.manual.iter().map(|p| 1 + p.modules.len()).sum()) {
        return None;
    }
    let under_manual: HashSet<&str> = input
        .manual
        .iter()
        .flat_map(|p| p.modules.iter().map(String::as_str))
        .collect();
    let mut seen: HashSet<&str> = HashSet::new();
    let mut allowed: Vec<&str> = Vec::new();
    // How many allowed modules each node is upstream of, itself included; one means it is last.
    let mut marks: HashMap<&str, usize> = HashMap::new();
    for &name in input.names {
        if !seen.insert(name) {
            continue;
        }
        if !budget.charge(1) {
            return None;
        }
        if !input.edges.on_a_chain(name) || !is_vst_module((input.module)(name)) {
            continue;
        }
        if under_manual.contains(name) || input.thawed.contains(name) {
            continue;
        }
        let inside = input.edges.upstream_of(name, budget)?;
        if input.edges.blocked_by(name, &inside, budget)?.is_some() {
            continue;
        }
        if inside
            .iter()
            .any(|n| (input.is_strudel)(n) || input.thawed.contains(*n))
        {
            continue;
        }
        for n in inside {
            *marks.entry(n).or_insert(0) += 1;
        }
        allowed.push(name);
    }
    Some(
        allowed
            .into_iter()
            .filter(|c| marks.get(c) == Some(&1))
            .map(str::to_owned)
            .collect(),
    )
}

/// Every automatic freeze of a project against its LIVE document (no automatic freeze applied) (decisions 339 to 348).
pub fn auto_freezes(
    source: &dyn Source,
    cache: &mut Cache,
    input: &AutoInput<'_>,
    budget: &mut Budget,
) -> Result<AutoFreezes, SourceError> {
    let spent = AutoFreezes {
        freezes: Vec::new(),
        capped: true,
    };
    let Some(renders) = input.renders.and_then(|r| spell(r, input.rule).ok()) else {
        return Ok(AutoFreezes {
            freezes: Vec::new(),
            capped: false,
        });
    };
    if budget.capped() {
        return Ok(spent);
    }
    let Some(points) = auto_freeze_points(input, budget) else {
        return Ok(spent);
    };
    if !budget.charge(input.live.tracks.iter().map(|t| 1 + t.plugins.len()).sum()) {
        return Ok(spent);
    }
    // Only a module the compiler turned into a plugin has a signal to render.
    let compiled: HashSet<&str> = input
        .live
        .tracks
        .iter()
        .flat_map(|t| t.plugins.iter().map(|p| plugin_module(&p.id)))
        .collect();
    let mut fingerprints = Fingerprinter::new(input.rule);
    let mut freezes = Vec::new();
    for module in points {
        if !compiled.contains(module.as_str()) {
            continue;
        }
        let at = [(module.clone(), String::new())];
        let Some(plans) = freeze_plans(input.edges, &at, input.is_lane, budget) else {
            return Ok(spent);
        };
        let Some(plan) = plans.into_iter().next() else {
            continue;
        };
        let Some(cut) = freeze_cut(input.live, &plan, budget) else {
            return Ok(spent);
        };
        let Some(fingerprint) = fingerprints.fingerprint(source, cache, &cut, input.project, budget)? else {
            return Ok(spent);
        };
        if !budget.charge(1) {
            return Ok(spent);
        }
        let render = auto_render_path(&renders, &module, &fingerprint);
        let plugins: Vec<FreezePlugin> = plan
            .modules
            .iter()
            .filter_map(|n| (input.module)(n).filter(|m| is_vst_module(Some(m))))
            .filter_map(|m| m.authored.plugin.as_ref())
            .map(|p| FreezePlugin {
                format: p.plugin_type.as_deref().unwrap_or_default().to_lowercase(),
                name: p.name.clone().unwrap_or_default(),
            })
            .collect();
        let pairs: Vec<(&str, &str)> = plugins.iter().map(|p| (p.format.as_str(), p.name.as_str())).collect();
        let hosted = !input.no_vsts && input.scan.hosts_all(&pairs);
        let ready = render_ready(source, &render, input.rule)?;
        freezes.push(AutoFreeze {
            module,
            render: render.clone(),
            plan: FreezePlan { render, ..plan },
            cut,
            plugins,
            ready,
            hosted,
        });
    }
    Ok(AutoFreezes { freezes, capped: false })
}
