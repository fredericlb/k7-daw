// node_freeze.rs — `node freeze <node>`, a toggle: everything upstream bounced to one wav, or a frozen node thawed; after verbs/node.ts.
// A reader would get wrong the order: the bounce lands in the project first, and only then does `frozen:` name it.

use super::node_shared::{plugin_module, split_args};
use super::routes::{ENGINE, call_or_throw, render_to};
use super::shared::{Done, join};
use crate::host::{Host, Thrown};
use serde_json::json;
use song::Cache;
use song::clips::Budget;
use song::freeze::{BUDGET, Blocked, Edges, FreezePlan, freeze_cut, freeze_plans};
use song::graph::{TrackType, load_graph};
use song::lanes::inputs::load_inputs;
use song::lanes::rig::Rig;
use song_edit::confine::confine_to;
use song_edit::loader::{resolve_node, roots_for};
use song_edit::project_edit::set_module_frozen;

/// Where a freeze render lands: one folder for the lot, at the project root.
const FROZEN_DIR: &str = "renders";

fn spent() -> Thrown {
    Thrown("freeze: the graph is too large to walk".to_owned())
}

/// A freeze's plan, the strudel lanes it would bake, and whether the node is a lane.
type Planned = (FreezePlan, Vec<String>, bool);

/// What the graph says of a freeze at `module`: its plan, the strudel lanes in it, whether it is a lane; Err is the refusal.
fn plan_of(h: &Host, module: &str, render: &str) -> Result<Result<Planned, String>, Thrown> {
    let roots = roots_for(&*h.fs, &h.env, &h.song_path);
    let mut cache = Cache::new();
    let inputs = load_inputs(&*h.fs, &mut cache, &roots, Rig::default()).map_err(|e| Thrown(e.to_string()))?;
    let g = load_graph(&*h.fs, &mut cache, &roots, &inputs).map_err(|e| Thrown(format!("{e:?}")))?;
    let edges = Edges::of(&g.chains);
    let mut budget = Budget::new(BUDGET);
    let inside = edges.upstream_of(module, &mut budget).ok_or_else(spent)?;
    match edges.blocked_by(module, &inside, &mut budget).ok_or_else(spent)? {
        Some(Blocked::NoSuchModule) => return Ok(Err(format!("no module \"{module}\" in the chains"))),
        Some(Blocked::FeedsOutside { from, to }) => {
            return Ok(Err(format!(
                "'{from}' is upstream of '{module}' but also feeds '{to}', which is not — its signal would be in the render AND still playing. Freeze at the point they meet instead."
            )));
        }
        None => {}
    }
    let is_lane = |n: &str| g.tracks.contains_key(n);
    let at = [(module.to_owned(), render.to_owned())];
    let plans = freeze_plans(&edges, &at, &is_lane, &mut budget).ok_or_else(spent)?;
    let Some(plan) = plans.into_iter().next() else {
        return Ok(Err(format!("no module \"{module}\" in the chains")));
    };
    let strudel = plan
        .lanes
        .iter()
        .filter(|n| g.tracks.get(n.as_str()).is_some_and(|t| t.kind == TrackType::Strudel))
        .cloned()
        .collect();
    Ok(Ok((plan, strudel, is_lane(module))))
}

/// `node freeze <node>` — a frozen node is thawed and its render deleted; a live one is bounced, then frozen.
pub fn node_freeze(a: &str, h: &mut Host) -> Done {
    let w = split_args(a);
    let Some(module) = w.first().cloned() else {
        h.refuse(["usage: node freeze <node>".to_owned()]);
        return Ok(());
    };
    let dir = h.song_path.clone();
    let node = resolve_node(&*h.fs, &h.env, &dir, &module)?;
    let current = node
        .as_ref()
        .and_then(|n| n.authored.frozen.as_ref())
        .map(|f| f.0.clone());
    if let Some(current) = current {
        set_module_frozen(h.fs, &h.env, &dir, &module, None)?;
        // Only a render inside the project is deleted: `frozen:` is text a file holds.
        let render = confine_to(&*h.fs, std::slice::from_ref(&dir), &join(&dir, &current));
        if let Some(r) = &render {
            h.fs.remove(r).map_err(|e| Thrown(e.to_string()))?;
        }
        h.log([format!("{module}: unfrozen — everything upstream is live again")]);
        if render.is_none() {
            h.log([format!("  {current} is outside the project — left alone")]);
        }
        h.reload();
        return Ok(());
    }
    let render = format!("{FROZEN_DIR}/{}.wav", module.replace([':', '/'], "-"));
    let (plan, strudel, lane) = match plan_of(h, &module, &render)? {
        Ok(x) => x,
        Err(why) => {
            h.refuse([format!("freeze: {why}")]);
            return Ok(());
        }
    };
    // A stopped strudel lane has no clips at all, so the render would be a file of silence.
    if let Some(first) = strudel.first() {
        h.refuse([format!(
            "freeze: '{first}' is a strudel lane — its material only exists while it plays, so it can't be baked"
        )]);
        return Ok(());
    }
    let p = h.project()?;
    // The track the render will play ON, decided by the compiler rather than guessed.
    let owner = lane
        || p.tracks
            .iter()
            .any(|t| t.plugins.iter().any(|pl| plugin_module(&pl.id) == module));
    if !owner {
        h.refuse([format!(
            "freeze: '{module}' compiles to no plugin, so there is no signal at it to render"
        )]);
        return Ok(());
    }
    // Stopped first: a freeze that lands mid-playback swaps a track's contents under a rolling transport.
    call_or_throw(h, ENGINE, json!({ "op": "transport", "action": "stop" }))?;
    let cut = freeze_cut(&p, &plan, &mut Budget::new(BUDGET)).ok_or_else(spent)?;
    if let Err(e) = render_to(h, &cut, &join(&dir, &render))? {
        h.refuse([format!("freeze failed: {e}")]);
        return Ok(());
    }
    set_module_frozen(h.fs, &h.env, &dir, &module, Some(&render))?;
    let inside = plan.lanes.len() + plan.modules.len() - 1;
    let baked = match inside {
        0 => String::new(),
        1 => " (1 node upstream baked in)".to_owned(),
        n => format!(" ({n} nodes upstream baked in)"),
    };
    h.log([format!(
        "{module}: frozen to {render}{baked} — everything downstream stays live"
    )]);
    h.reload();
    Ok(())
}
