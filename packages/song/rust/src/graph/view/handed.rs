// graph/view/handed.rs — `Handed` read off the compile's answer and off the graph's own frozen modules (Part B of 2k).
// A reader would get wrong that the manual plans come from the GRAPH's chains and lanes: they stand when no document does.

use super::{Answer, AutoMark, ClipExtent, FreezeMark, Handed, warning, web_graph};
use crate::cache::Cache;
use crate::clips::Budget;
use crate::doc::{Compiled, ProjectDoc};
use crate::freeze::{self, AutoFreeze, Edges, FreezePlan, freeze_plans, name_units, render_ready};
use crate::graph::{GraphDoc, GraphError, Spend, load_graph};
use crate::jsorder::own_key_order;
use crate::lanes::inputs::{Inputs, load_inputs};
use crate::lanes::rig::Rig;
use crate::resolve::MIXER_PLUGIN;
use crate::roots::{Roots, is_absolute};
use crate::source::path::{PathRule, join};
use crate::source::{Source, SourceError};
use indexmap::IndexMap;

/// Which folder a graph is drawn for: the open project draws its automatic freezes, any other folder (a library chain's) none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Folder {
    Open,
    Other,
}

/// What a stat of a render costs the freeze budget, as the compile charges it.
const STAT_COST: usize = 4;

/// Whether a render is there: an absolute path as it stands, else under the project; the Source decides what it may see (335).
pub fn render_exists(source: &dyn Source, project: &str, render: &str, rule: PathRule) -> Result<bool, SourceError> {
    let at = if is_absolute(render, rule) {
        render.to_owned()
    } else {
        join(project, render)
    };
    render_ready(source, &at, rule)
}

/// Each track's clips by id, when it has any (`clipExtentsOf`); None when the spend stops, never half a map.
fn extents_of(doc: &ProjectDoc, spend: &mut Spend) -> Option<IndexMap<String, Vec<ClipExtent>>> {
    let mut out = IndexMap::new();
    for t in doc.tracks.iter().filter(|t| !t.clips.is_empty()) {
        if !spend.charge(2 + t.id.len() / 64 + t.clips.len() / 4) {
            return None;
        }
        let clips = t
            .clips
            .iter()
            .map(|c| ClipExtent {
                start: c.start,
                length: c.length,
            })
            .collect();
        out.insert(t.id.clone(), clips);
    }
    own_key_order(&mut out);
    Some(out)
}

/// The LAST mixer of each track, by the plugin's name (`trackMixersOf`); the level lives on the last.
fn mixers_of(doc: &ProjectDoc, spend: &mut Spend) -> Option<IndexMap<String, String>> {
    let mut out = IndexMap::new();
    for t in &doc.tracks {
        let Some(mixer) = t.plugins.iter().rev().find(|p| p.plugin_type == MIXER_PLUGIN) else {
            continue;
        };
        if !spend.bytes(t.id.len() + mixer.name.len()) {
            return None;
        }
        out.insert(t.id.clone(), mixer.name.clone());
    }
    own_key_order(&mut out);
    Some(out)
}

/// Each lane's compiled colour by NAME, the first track that has one (`trackColorsOf`).
fn colors_of(doc: &ProjectDoc, spend: &mut Spend) -> Option<IndexMap<String, String>> {
    let mut out: IndexMap<String, String> = IndexMap::new();
    for t in doc.tracks.iter().filter(|t| !t.color.is_empty()) {
        if out.contains_key(&t.name) {
            continue;
        }
        if !spend.bytes(t.name.len() + t.color.len()) {
            return None;
        }
        out.insert(t.name.clone(), t.color.clone());
    }
    own_key_order(&mut out);
    Some(out)
}

/// The plans in force over the GRAPH's chains and lanes (`activeFreezePlans` as `graphDoc.ts` asks it); None when a budget stops (463).
fn manual_marks(
    source: &dyn Source,
    inputs: &Inputs,
    g: &GraphDoc<'_>,
    spend: &mut Spend,
) -> Result<Option<Vec<FreezeMark>>, SourceError> {
    let mut budget = Budget::new(freeze::BUDGET);
    let mut at: Vec<(String, String)> = Vec::new();
    for (name, m) in &g.modules {
        let Some(render) =
            m.m.authored
                .frozen
                .as_ref()
                .map(|f| f.0.as_str())
                .filter(|r| !r.is_empty())
        else {
            continue;
        };
        if !budget.charge(STAT_COST) {
            return Ok(None);
        }
        if render_exists(source, &inputs.project, render, inputs.rule)? {
            at.push(((*name).to_owned(), render.to_owned()));
        }
    }
    if at.is_empty() {
        return Ok(Some(Vec::new()));
    }
    let edges = Edges::of(&g.chains);
    let is_lane = |n: &str| g.tracks.contains_key(n);
    let Some(plans) = freeze_plans(&edges, &at, &is_lane, &mut budget) else {
        return Ok(None);
    };
    let mut out = Vec::with_capacity(plans.len());
    for FreezePlan { at, lanes, modules, .. } in plans {
        let cost = name_units(&at) + lanes.iter().chain(&modules).map(|n| name_units(n)).sum::<usize>();
        if !spend.charge(cost) {
            return Ok(None);
        }
        out.push(FreezeMark {
            at,
            lanes: lanes.into_iter().collect(),
            modules: modules.into_iter().collect(),
        });
    }
    Ok(Some(out))
}

/// The compile's automatic freezes as plain marks, each paid by the names it copies; None when the spend stops.
fn auto_marks(freezes: &[AutoFreeze], spend: &mut Spend) -> Option<Vec<AutoMark>> {
    let mut out = Vec::new();
    for f in freezes {
        let names = f.plan.lanes.iter().chain(&f.plan.modules);
        let cost = names.map(|n| name_units(n)).sum::<usize>() + name_units(&f.module);
        if !spend.charge(cost) {
            return None;
        }
        out.push(AutoMark {
            module: f.module.clone(),
            ready: f.ready,
            hosted: f.hosted,
            lanes: f.plan.lanes.iter().cloned().collect(),
            modules: f.plan.modules.iter().cloned().collect(),
        });
    }
    Some(out)
}

/// What the compile hands the graph (430) and whether a spend stopped a product, which is then absent whole (463).
pub(super) fn made(
    source: &dyn Source,
    inputs: &Inputs,
    g: &GraphDoc<'_>,
    compiled: Option<&Compiled>,
    folder: Folder,
) -> Result<(Handed, bool), SourceError> {
    let mut spend = Spend::new();
    let mut h = Handed::default();
    let mut stopped = false;
    let doc = compiled.and_then(|c| c.engine.as_ref().ok());
    if let Some(doc) = doc {
        h.extents = extents_of(doc, &mut spend);
        h.mixers = mixers_of(doc, &mut spend);
        h.colors = colors_of(doc, &mut spend);
        stopped = h.extents.is_none() || h.mixers.is_none() || h.colors.is_none();
    }
    match manual_marks(source, inputs, g, &mut spend)? {
        Some(manual) => h.manual = manual,
        None => stopped = true,
    }
    if let (Some(c), Some(_), Folder::Open) = (compiled, doc, folder) {
        match auto_marks(&c.freezes, &mut spend) {
            Some(auto) => h.auto = auto,
            None => stopped = true,
        }
    }
    Ok((h, stopped))
}

/// What the compile hands the graph of the open project; a host reaches it through `load_compiled_graph`.
pub fn handed_of(
    source: &dyn Source,
    inputs: &Inputs,
    g: &GraphDoc<'_>,
    compiled: &Compiled,
) -> Result<Handed, SourceError> {
    made(source, inputs, g, Some(compiled), Folder::Open).map(|(h, _)| h)
}

/// A project read, its `Handed` made from the compile and drawn: the one call a host with a compile makes (462).
pub fn load_compiled_graph(
    source: &dyn Source,
    cache: &mut Cache,
    roots: &Roots,
    rig: Rig,
    compiled: Option<&Compiled>,
    folder: Folder,
) -> Result<Answer, GraphError> {
    let inputs = load_inputs(source, cache, roots, rig)?;
    let g = load_graph(source, cache, roots, &inputs)?;
    let (handed, stopped) = made(source, &inputs, &g, compiled, folder)?;
    let mut a = web_graph(source, cache, roots, &inputs, &g, &handed)?;
    if stopped {
        a.capped = true;
        a.findings.push(warning(
            "project.k7x".to_owned(),
            "the compile's values for this graph are more than one answer holds — what did not fit is left out"
                .to_owned(),
        ));
    }
    Ok(a)
}

/// What `Workspace.missingModelOf` finds for a node: no such node, the model there, or the model it lacks and where it comes from (2446).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelNeed {
    NoNode,
    Have,
    Needs { file: String, url: Option<String> },
}

/// A node's missing model read off the graph with no compile, as `loadGraph(...).modules[node]` reads it.
pub fn model_need(
    source: &dyn Source,
    cache: &mut Cache,
    roots: &Roots,
    rig: Rig,
    node: &str,
) -> Result<ModelNeed, GraphError> {
    let inputs = load_inputs(source, cache, roots, rig)?;
    let g = load_graph(source, cache, roots, &inputs)?;
    Ok(match g.modules.get(node) {
        None => ModelNeed::NoNode,
        Some(m) => match super::step::missing_model(m) {
            None => ModelNeed::Have,
            Some((file, url)) => ModelNeed::Needs { file, url },
        },
    })
}
