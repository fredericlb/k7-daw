// doc/plugins.rs — every chain's steps walked into the plugins and sends of the track that owns them, in compilePlugins' order.
// A reader would get wrong that the order is the document: the engine plays `trackPlugins` as listed, and the walk is one pass.

use super::notes::Notes;
use super::plugin::{Built, Cx, step_plugin};
use super::texts::Texts;
use super::{SendDoc, text_weight};
use crate::clips::Budget;
use crate::curves::curve_id_from_ref;
use crate::lanes::inputs::Inputs;
use crate::lanes::ownership::Ownership;
use crate::modules::ModuleKind;
use crate::resolve::{InstrumentType, resolved_type};
use crate::source::SourceError;
use std::collections::{HashMap, HashSet};

/// A curve bound to a plugin's parameter, held by reference until the song's length is known.
#[derive(Clone, Copy, Debug)]
pub(super) struct RawAutomation<'c> {
    pub owner: &'c str,
    pub chain: &'c str,
    pub module: &'c str,
    pub param: &'c str,
    pub curve: &'c str,
    /// The module whose note-ons retrigger the curve.
    pub trigger: &'c str,
}

/// What the walk made.
#[derive(Default)]
pub(super) struct CompiledPlugins<'c> {
    pub track_plugins: HashMap<&'c str, Vec<super::PluginDoc>>,
    /// Per owner, the words (`extends`, type) of the modules it plays, in chain order.
    pub lane_hints: HashMap<&'c str, Vec<&'c str>>,
    pub track_sends: HashMap<&'c str, Vec<SendDoc>>,
    pub raw: Vec<RawAutomation<'c>>,
    pub plugin_of_node: HashMap<&'c str, String>,
    /// The module the budget stopped the walk at: its file, for the one ERROR.
    pub capped_at: Option<String>,
    /// How many tracks still ask for each owner's plugins and sends (decision 394).
    pub claims: HashMap<String, usize>,
}

impl CompiledPlugins<'_> {
    /// One more track's plugins and sends: moved on the last claim, copied and paid for before that; None when the budget is spent.
    pub fn take(&mut self, owner: &str, budget: &mut Budget) -> Option<(Vec<super::PluginDoc>, Vec<SendDoc>)> {
        let left = self.claims.get_mut(owner).map_or(0, |n| {
            *n = n.saturating_sub(1);
            *n
        });
        if left == 0 {
            let plugins = self.track_plugins.remove(owner).unwrap_or_default();
            return Some((plugins, self.track_sends.remove(owner).unwrap_or_default()));
        }
        let plugins = self.track_plugins.get(owner).map_or(&[][..], Vec::as_slice);
        let sends = self.track_sends.get(owner).map_or(&[][..], Vec::as_slice);
        let cost = plugins.iter().map(super::PluginDoc::weight).sum::<usize>()
            + sends.iter().map(|s| 1 + text_weight(&s.to)).sum::<usize>();
        budget.charge(cost).then(|| (plugins.to_vec(), sends.to_vec()))
    }
}

/// Every chain's steps compiled; `frozen` are the modules a render stands in for (decision 312).
pub(super) fn compile_plugins<'c>(
    inputs: &'c Inputs,
    own: &'c Ownership,
    frozen: &HashSet<String>,
    texts: &mut Texts<'c>,
    notes: &mut Notes,
    budget: &mut Budget,
) -> Result<CompiledPlugins<'c>, SourceError> {
    let mut out = CompiledPlugins::default();
    'chains: for (name, c) in &inputs.chains {
        if !budget.charge(1) {
            out.capped_at = Some("project.k7x".to_owned());
            break;
        }
        let Some(owner) = own.owner_of(name) else { continue };
        for step in &c.use_steps {
            if !budget.charge(1) {
                out.capped_at = Some(
                    inputs
                        .module(&step.module)
                        .map_or_else(|| "project.k7x".to_owned(), |m| m.own_label()),
                );
                break 'chains;
            }
            if frozen.contains(&step.module) {
                continue;
            }
            let module = inputs.module(&step.module);
            for a in module.iter().flat_map(|m| m.authored.automations.iter().flatten()) {
                if !budget.charge(1 + (name.len() + step.module.len()) / 16) {
                    out.capped_at = module.map(|m| m.own_label());
                    break 'chains;
                }
                out.raw.push(RawAutomation {
                    owner,
                    chain: name,
                    module: &step.module,
                    param: &a.param.0,
                    curve: curve_id_from_ref(&a.curve.0),
                    trigger: a.trigger.as_ref().map_or(step.module.as_str(), |t| t.0.as_str()),
                });
            }
            let Some(module) = module else { continue };
            let hints = out.lane_hints.entry(owner).or_default();
            hints.push(module.authored.extends.as_deref().unwrap_or(""));
            hints.push(match resolved_type(Some(module)) {
                InstrumentType::Rack => "rack",
                InstrumentType::Plugin => "plugin",
            });
            if module.kind != Some(ModuleKind::Plugin) {
                continue;
            }
            let mut cx = Cx {
                inputs,
                own,
                texts: &mut *texts,
                notes: &mut *notes,
                budget: &mut *budget,
                plugin_of_node: &mut out.plugin_of_node,
            };
            match step_plugin(&mut cx, name, step, module, step.enabled != Some(false))? {
                Built::Skip => {}
                Built::Plugin(p) => out.track_plugins.entry(owner).or_default().push(*p),
                Built::Capped => {
                    out.capped_at = Some(module.own_label());
                    break 'chains;
                }
            }
        }
        for o in &c.out {
            if !own.bus_chains.contains(&o.to) {
                continue;
            }
            if !budget.charge(1 + o.to.len() / 16) {
                out.capped_at = Some("project.k7x".to_owned());
                break 'chains;
            }
            out.track_sends.entry(owner).or_default().push(SendDoc {
                to: o.to.clone(),
                level: o.gain,
                enabled: true,
            });
        }
    }
    Ok(out)
}
