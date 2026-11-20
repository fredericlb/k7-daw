// lane_strip_plan.rs — what a lane needs to get its strip, and the writes that give it: one chain file where it can, else the graph.
// A reader would get wrong that a chain project tries the one-file insert first: the whole-graph rewrite folds a fade's stretch into its lane.

use crate::chain_data::{Chain, Step};
use crate::chain_graph_edit::{ChainGraphEdit, chain_graph_text, edit_chain_graph};
use crate::chain_item_edit::{CHAINS_DIR, chain_file_path, is_chain_project};
use crate::chain_project::read_chain_project;
use crate::confine::Env;
use crate::error::{Error, Res, fail};
use crate::format_chain::format_chain;
use crate::fs::Fs;
use crate::graph_schema::{GRAPH_FILE, parse_graph};
use crate::ids::NewId;
use crate::lane_strip::{StripKind, StripPlan, apply_lane_strip_plain, mixer_name_for, splice_strip};
use crate::loader::{load_modules, roots_for};
use crate::paths::{join, relative};
use crate::workspace_doc::{ws_text, ws_write_text};
use indexmap::IndexMap;
use k7x::edit::{Document, PathSeg, Value, WriteOptions};
use song::graph::{GraphError, Index, lane_strip, load_graph};
use song::lanes::inputs::load_inputs;
use song::lanes::rig::Rig;
use song::{Cache, PathRule};
use std::collections::{HashMap, HashSet};

/// The lanes of a loaded graph, as far as a strip needs: which are picture lanes, the run each starts, the strip each holds.
#[derive(Clone, Debug, Default)]
pub struct LaneGraph {
    pub tracks: Vec<String>,
    pub video_tracks: Vec<String>,
    /// The names of the graph's chains, which a new node's name must not take.
    pub chains: HashSet<String>,
    runs: HashMap<String, Vec<String>>,
    strips: HashMap<String, String>,
}

fn refused(e: GraphError) -> Error {
    match e {
        GraphError::Source(s) => Error::Msg(s.to_string()),
        GraphError::Refused(r) => match r.findings.first() {
            Some(f) => Error::Msg(f.message.clone()),
            None => Error::Msg(format!("module input wiring invalid ({} faults)", r.faults.len())),
        },
    }
}

impl LaneGraph {
    /// The project's graph as `loadGraph` reads it.
    ///
    /// # Errors
    /// A read the source refuses, or a graph the loader refuses.
    pub fn load(fs: &dyn Fs, env: &Env, dir: &str) -> Res<LaneGraph> {
        let roots = roots_for(fs, env, dir);
        let mut cache = Cache::new();
        let inputs = load_inputs(fs, &mut cache, &roots, Rig::default()).map_err(|e| Error::Msg(e.to_string()))?;
        let g = load_graph(fs, &mut cache, &roots, &inputs).map_err(refused)?;
        let idx = Index::new(&g);
        let mut out = LaneGraph {
            tracks: g.tracks.keys().map(|k| (*k).to_owned()).collect(),
            video_tracks: g.video_tracks.keys().map(|k| (*k).to_owned()).collect(),
            chains: g.chains.keys().cloned().collect(),
            ..LaneGraph::default()
        };
        for from in g.chains.values().filter_map(|c| c.from.as_deref()) {
            if out.runs.contains_key(from) {
                continue;
            }
            let run = idx
                .first_chain_from(from)
                .map(|c| c.use_steps.iter().map(|s| s.module.clone()).collect())
                .unwrap_or_default();
            out.runs.insert(from.to_owned(), run);
            if let Some(s) = lane_strip(&g, &idx, from) {
                out.strips.insert(from.to_owned(), s.to_owned());
            }
        }
        Ok(out)
    }

    /// A picture lane's strip is the shader; anything else a lane can be is heard, so its strip is the Mixer.
    #[must_use]
    pub fn kind_of(&self, lane: &str) -> StripKind {
        if self.video_tracks.iter().any(|l| l == lane) {
            StripKind::Picture
        } else {
            StripKind::Audio
        }
    }

    /// The modules of the run this lane starts, in order.
    #[must_use]
    pub fn run_of(&self, lane: &str) -> &[String] {
        self.runs.get(lane).map_or(&[], Vec::as_slice)
    }

    /// The lane's strip, the LAST of its kind in its run, or None.
    #[must_use]
    pub fn strip_of(&self, lane: &str) -> Option<&str> {
        self.strips.get(lane).map(String::as_str)
    }
}

fn modules_of(text: &str) -> IndexMap<String, Vec<String>> {
    let Some(Value::Map(m)) = parse_graph(text).value.and_then(|v| v.get("modules").cloned()) else {
        return IndexMap::new();
    };
    m.into_iter()
        .map(|(k, d)| {
            let dests = match d {
                Value::List(l) => l
                    .into_iter()
                    .filter_map(|x| if let Value::Str(s) = x { Some(s) } else { None })
                    .collect(),
                _ => vec![],
            };
            (k, dests)
        })
        .collect()
}

/// What `lane` needs, its strip LAST in its run, or None when it has one; a run ending on another sum is refused.
///
/// # Errors
/// A run that ends on another sum, or a read that fails.
pub fn plan_lane_strip(
    fs: &dyn Fs,
    env: &Env,
    dir: &str,
    lane: &str,
    g: &LaneGraph,
    new_id: NewId,
) -> Res<Option<StripPlan>> {
    if g.strip_of(lane).is_some() {
        return Ok(None);
    }
    let kind = g.kind_of(lane);
    let sink = kind.sink();
    let run = g.run_of(lane);
    // The node the strip goes AFTER: the run's last module, or the lane itself when nothing follows it.
    let prev = run.last().map_or(lane, String::as_str);
    let chained = is_chain_project(fs, dir);
    let graph_path = join(&[dir, GRAPH_FILE]);
    let modules = if chained {
        modules_of(&chain_graph_text(fs, dir)?)
    } else if fs.exists(&graph_path) {
        modules_of(&ws_text(fs, &graph_path)?)
    } else {
        IndexMap::new()
    };
    let dests = modules.get(prev).map_or(&[][..], Vec::as_slice);
    if !dests.is_empty() && !dests.iter().any(|d| d == sink) {
        return fail(format!(
            "'{lane}' does not run into {sink} — wire its strip by hand, this would guess"
        ));
    }
    if chained {
        let chain = read_chain_project(fs, dir)?
            .nodes
            .get(prev)
            .map_or_else(|| lane.to_owned(), |n| n.chain.clone());
        return Ok(Some(StripPlan {
            lane: lane.to_owned(),
            kind,
            name: new_id(),
            rel: format!("{CHAINS_DIR}/{chain}.k7x"),
            after: Some(prev.to_owned()),
        }));
    }
    let nodes = load_modules(fs, env, dir)?;
    let name = mixer_name_for(lane, &|n| nodes.contains_key(n) || g.chains.contains(n));
    Ok(Some(StripPlan {
        lane: lane.to_owned(),
        kind,
        rel: format!("nodes/{name}.k7x"),
        name,
        after: modules.contains_key(prev).then(|| prev.to_owned()),
    }))
}

fn key(s: &str) -> PathSeg {
    PathSeg::Key(s.to_owned())
}

fn item(plan: &StripPlan) -> Value {
    Value::Map(vec![
        ("id".to_owned(), Value::Str(plan.name.clone())),
        ("extends".to_owned(), Value::Str(plan.kind.reference().to_owned())),
    ])
}

/// In ONE chain file, the item right after `plan.after`, when the file's only way out is the sum: else None.
fn insert_in_chain_file(fs: &mut dyn Fs, dir: &str, plan: &StripPlan, sink: &str) -> Res<Option<Vec<String>>> {
    let Some(after) = &plan.after else { return Ok(None) };
    let cp = read_chain_project(fs, dir)?;
    let Some(at) = cp.nodes.get(after) else {
        return Ok(None);
    };
    if at.sub.is_some() || at.item.is_some() {
        return Ok(None);
    }
    let path = chain_file_path(dir, &at.chain);
    let mut doc = Document::new(&ws_text(fs, &path)?);
    let Some(Value::List(steps)) = doc.get_in(&[key("steps")])? else {
        return Ok(None);
    };
    // Only past the run's LAST item, and only where nothing but the sum leaves: a send stays before the strip.
    let rest = steps.get(at.step + 1..).unwrap_or_default();
    let Chain { steps: rest, .. } =
        Chain::from_value(&Value::Map(vec![("steps".to_owned(), Value::List(rest.to_vec()))]));
    if !rest.iter().all(Step::is_end) {
        return Ok(None);
    }
    let links = rest
        .iter()
        .flat_map(|s| Chain::links(std::slice::from_ref(s)).to_vec())
        .collect::<Vec<_>>();
    if links
        .iter()
        .any(|l| l.chain != sink || l.port.as_deref().is_some_and(|p| !p.is_empty()))
    {
        return Ok(None);
    }
    let steps_at = [key("steps")];
    doc.insert_in(&steps_at, at.step + 1, &item(plan), WriteOptions::default())?;
    if links.is_empty() {
        let to = Value::Map(vec![(
            "to".to_owned(),
            Value::List(vec![Value::Map(vec![(
                "chain".to_owned(),
                Value::Str(sink.to_owned()),
            )])]),
        )]);
        doc.add_in(&steps_at, &to, WriteOptions::default())?;
    }
    if song::routing::decode_chain("chain", doc.text(), PathRule::HOST).is_err() {
        return Ok(None);
    }
    let chain = Chain::from_value(&doc.to_js()?);
    ws_write_text(fs, &path, &format_chain(&chain, doc.text())?)?;
    Ok(Some(vec![relative(dir, &path)]))
}

/// The writes a plan implies: one chain file where it can, else the whole graph; a node file and an arrow in graph.k7x.
///
/// # Errors
/// What the graph writers or the file system fail with.
pub fn apply_lane_strip(fs: &mut dyn Fs, env: &Env, dir: &str, plan: &StripPlan) -> Res<Vec<String>> {
    let sink = plan.kind.sink();
    if !is_chain_project(fs, dir) {
        return apply_lane_strip_plain(fs, dir, plan);
    }
    // Targeted first: the whole-graph rewrite folds a fade's stretch into its lane.
    if let Some(one) = insert_in_chain_file(fs, dir, plan, sink)? {
        return Ok(one);
    }
    let after = plan.after.clone().unwrap_or_else(|| plan.lane.clone());
    let opts = ChainGraphEdit {
        add: IndexMap::from([(plan.name.clone(), item(plan))]),
        ..ChainGraphEdit::default()
    };
    edit_chain_graph(
        fs,
        env,
        dir,
        |graph| splice_strip(graph, &after, &plan.name, sink),
        &opts,
    )
}

/// Plan and apply in one, which is what a verb wants; None when the lane already had its strip.
///
/// # Errors
/// What `plan_lane_strip` and `apply_lane_strip` fail with.
pub fn add_lane_strip(
    fs: &mut dyn Fs,
    env: &Env,
    dir: &str,
    lane: &str,
    g: &LaneGraph,
    new_id: NewId,
) -> Res<Option<Vec<String>>> {
    match plan_lane_strip(fs, env, dir, lane, g, new_id)? {
        Some(plan) => apply_lane_strip(fs, env, dir, &plan).map(Some),
        None => Ok(None),
    }
}

/// A lane just made gets its strip; one that cannot be wired is SAID in `no_strip`, never refused for.
pub fn strip_new_lane(fs: &mut dyn Fs, env: &Env, dir: &str, lane: &str, new_id: NewId) -> (Vec<String>, String) {
    let done = LaneGraph::load(fs, env, dir).and_then(|g| add_lane_strip(fs, env, dir, lane, &g, new_id));
    match done {
        Ok(written) => (written.unwrap_or_default(), String::new()),
        Err(e) => (vec![], e.to_string()),
    }
}

/// Every listed lane lacking a strip gets one, the back-fill; a DRY pass plans and writes nothing.
///
/// # Errors
/// The first graph load that fails; a lane's own failure is a warning.
pub fn strip_lanes(
    fs: &mut dyn Fs,
    env: &Env,
    dir: &str,
    pick: &dyn Fn(&LaneGraph) -> Vec<String>,
    dry: bool,
    new_id: NewId,
) -> Res<(Vec<StripPlan>, Vec<String>, Vec<String>)> {
    let (mut planned, mut written, mut warnings) = (vec![], vec![], vec![]);
    let mut g = LaneGraph::load(fs, env, dir)?;
    for lane in pick(&g) {
        let step = (|| -> Res<()> {
            let Some(plan) = plan_lane_strip(fs, env, dir, &lane, &g, new_id)? else {
                return Ok(());
            };
            planned.push(plan.clone());
            if dry {
                return Ok(());
            }
            written.extend(apply_lane_strip(fs, env, dir, &plan)?);
            // Re-read only after a write: the next plan must see this strip.
            g = LaneGraph::load(fs, env, dir)?;
            Ok(())
        })();
        if let Err(e) = step {
            warnings.push(format!("{lane}: {e}"));
        }
    }
    Ok((planned, written, warnings))
}
