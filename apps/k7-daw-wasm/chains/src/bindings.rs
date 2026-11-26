// bindings.rs — joining a surface's `mapping:` to the compiled chains: which control moves which param, on which plugin, and which widget states its range.
// A reader would get wrong that the SCALE is not decided here: the desk normalises through the control's range, the mastering strip refuses to.

use indexmap::{IndexMap, IndexSet};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One line of a `mapping:`: the module and param a control drives, mirrored when `invert` is set.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct SurfaceTarget {
    pub module: String,
    pub param: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invert: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct SurfaceBinding {
    pub module: String,
    pub param: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invert: Option<bool>,
    /// A second param this control drives; at most one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub also: Option<SurfaceTarget>,
}

impl SurfaceBinding {
    fn target(&self) -> SurfaceTarget {
        SurfaceTarget {
            module: self.module.clone(),
            param: self.param.clone(),
            invert: self.invert,
        }
    }
}

/// May a step that loads no plugin still be bound? A desk binds it by its track, a mastering strip refuses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum PluglessPolicy {
    Track,
    Refuse,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct Panel {
    #[serde(default)]
    widgets: IndexMap<String, Value>,
    #[serde(default)]
    tabs: Vec<Tab>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct Tab {
    #[serde(default)]
    widgets: IndexMap<String, Value>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct Step {
    #[serde(default)]
    module: String,
    #[serde(default, rename = "pluginId")]
    plugin_id: Option<String>,
    ui: Option<Panel>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct Chain {
    from: Option<String>,
    #[serde(default, rename = "use")]
    steps: Vec<Step>,
}

/// The chains of the page's graph, as the join reads them.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct BindGraph {
    #[serde(default)]
    chains: IndexMap<String, Chain>,
}

/// A step found by the module name a mapping names, carrying its chain's source track.
struct Found<'a> {
    step: &'a Step,
    track: Option<&'a str>,
}

/// Every chain's steps by the name the chains file uses; the first wins, a mapping names a node rather than a use.
fn steps_by_module(graph: &BindGraph) -> IndexMap<&str, Found<'_>> {
    let mut steps = IndexMap::new();
    for chain in graph.chains.values() {
        for step in &chain.steps {
            steps.entry(step.module.as_str()).or_insert(Found {
                step,
                track: chain.from.as_deref().filter(|t| !t.is_empty()),
            });
        }
    }
    steps
}

/// The widget that draws `param` on this step, on any tab: a control is the module's whichever page shows it.
fn widget_for(step: &Step, param: &str) -> Option<Value> {
    let ui = step.ui.as_ref()?;
    let all = ui
        .widgets
        .values()
        .chain(ui.tabs.iter().flat_map(|t| t.widgets.values()));
    all.into_iter()
        .find(|w| w.get("bind").and_then(Value::as_str) == Some(param))
        .cloned()
}

/// What a resolved target addresses.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct ResolvedTarget {
    pub module: String,
    pub param: String,
    #[serde(rename = "pluginId", skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(rename = "pluginId"))]
    pub plugin_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional, type = "import(\"@studio/graph-model\").PageWidget"))]
    pub widget: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invert: Option<bool>,
}

fn resolve_target(
    found: Option<&Found<'_>>,
    target: &SurfaceTarget,
    plugless: PluglessPolicy,
) -> Result<ResolvedTarget, String> {
    let Some(found) = found else {
        return Err(format!("no module “{}” in this project's chains", target.module));
    };
    let plugin = found.step.plugin_id.clone().filter(|p| !p.is_empty());
    if plugin.is_none() {
        if plugless == PluglessPolicy::Refuse {
            return Err(format!(
                "“{}” loads no plugin — nothing to set while the sound runs",
                target.module
            ));
        }
        if found.track.is_none() {
            return Err(format!("“{}” has neither a plugin nor a track to move", target.module));
        }
    }
    Ok(ResolvedTarget {
        module: target.module.clone(),
        param: target.param.clone(),
        plugin_id: plugin,
        track: found.track.map(str::to_owned),
        widget: widget_for(found.step, &target.param),
        invert: target.invert.filter(|i| *i),
    })
}

/// One line that resolved: the control's id, what it addresses, and the line as written.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct ResolvedRow {
    pub control: String,
    pub target: ResolvedTarget,
    /// The second target of the line, when it names one and `also` was asked for.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub also: Option<ResolvedTarget>,
    pub binding: SurfaceBinding,
}

/// A line that could not be resolved, and the sentence saying why.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct BindingProblem {
    pub control: String,
    pub message: String,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ResolvedMapping {
    pub rows: Vec<ResolvedRow>,
    pub problems: Vec<BindingProblem>,
}

/// Resolve a whole mapping against the graph: three failures, all reported rather than dropped.
pub fn resolve_mapping(
    mapping: &IndexMap<String, Option<SurfaceBinding>>,
    graph: &BindGraph,
    controls: &IndexSet<String>,
    noun: &str,
    plugless: PluglessPolicy,
    with_also: bool,
) -> ResolvedMapping {
    let steps = steps_by_module(graph);
    let mut out = ResolvedMapping {
        rows: Vec::new(),
        problems: Vec::new(),
    };
    // The second targets' problems come after every line's own, as the desk always listed them.
    let mut late = Vec::new();
    for (id, binding) in mapping {
        if !controls.contains(id) {
            out.problems.push(BindingProblem {
                control: id.clone(),
                message: format!("“{id}” is not a control {noun}"),
            });
            continue;
        }
        let Some(binding) = binding else { continue };
        match resolve_target(steps.get(binding.module.as_str()), &binding.target(), plugless) {
            Ok(target) => {
                let mut also = None;
                if let Some(second) = binding.also.as_ref().filter(|_| with_also) {
                    match resolve_target(steps.get(second.module.as_str()), second, plugless) {
                        Ok(t) => also = Some(t),
                        Err(_) => late.push(BindingProblem {
                            control: id.clone(),
                            message: format!(
                                "the second module “{}” has neither a plugin nor a track to move",
                                second.module
                            ),
                        }),
                    }
                }
                out.rows.push(ResolvedRow {
                    control: id.clone(),
                    target,
                    also,
                    binding: binding.clone(),
                });
            }
            Err(message) => out.problems.push(BindingProblem {
                control: id.clone(),
                message,
            }),
        }
    }
    out.problems.extend(late);
    out
}

/// Resolve one target alone: what a test of the join asks, the sentence saying why it did not or what it addresses.
pub fn resolve_one(
    graph: &BindGraph,
    target: &SurfaceTarget,
    plugless: PluglessPolicy,
) -> Result<ResolvedTarget, String> {
    let steps = steps_by_module(graph);
    resolve_target(steps.get(target.module.as_str()), target, plugless)
}

/// The controls a mapping does not name: the gate, reported once over the whole surface.
pub fn unmapped_controls(controls: &[String], mapping: &IndexMap<String, Option<SurfaceBinding>>) -> Vec<String> {
    controls
        .iter()
        .filter(|c| !matches!(mapping.get(*c), Some(Some(_))))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests;
