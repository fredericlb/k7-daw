// from_segments.rs — segments back into chain files: runs break where the signal splits or meets, and a diamond is a parallel step.
// A reader would get wrong that this is the inverse of song's `to_segments` only for what it wrote: names and sources come from the caller.

use crate::chain_data::{Chain, Item, Link, Start, Step, is_video_item, lane_of, remove_key, set_key};
use crate::chain_nest::{PrevPlaces, nest_subchains, regroup_calls};
use crate::chain_project::ChainProject;
use crate::error::Res;
use indexmap::IndexMap;
use k7x::edit::Value;
use song::routing::chain_file::CHAIN_SINKS;
use song::routing::{split_key, sub_key};
use std::collections::{HashMap, HashSet};

/// The graph's segments as `from_segments` reads them: the `modules` map, the bypass list and the bus list.
#[derive(Clone, Debug, Default)]
pub struct GraphIn {
    pub modules: IndexMap<String, Vec<String>>,
    pub bypassed: Vec<String>,
    pub bus_only: Vec<String>,
}

impl GraphIn {
    /// The segments of a parsed graph file's data.
    #[must_use]
    pub fn from_value(v: &Value) -> GraphIn {
        let strs = |x: Option<&Value>| -> Vec<String> {
            match x {
                Some(Value::List(l)) => l
                    .iter()
                    .filter_map(|s| if let Value::Str(s) = s { Some(s.clone()) } else { None })
                    .collect(),
                _ => vec![],
            }
        };
        GraphIn {
            modules: match v.get("modules") {
                Some(Value::Map(m)) => m.iter().map(|(k, d)| (k.clone(), strs(Some(d)))).collect(),
                _ => IndexMap::new(),
            },
            bypassed: strs(v.get("bypassed")),
            bus_only: strs(v.get("bus_only")),
        }
    }
}

pub type ItemOf<'a> = &'a dyn Fn(&str) -> Res<Item>;
pub type Test<'a> = &'a dyn Fn(&str) -> Res<bool>;
pub type NameFor<'a> = &'a dyn Fn(&str, &HashSet<String>) -> String;

pub struct FromSegments<'a> {
    /// A node's item as it should be written: the existing one, or one built for a migrated node.
    pub item_of: ItemOf<'a>,
    /// Whether a node with nothing feeding it STARTS a chain (a lane, an input) rather than sitting loose.
    pub is_source: Test<'a>,
    /// The chains as they were, so a rewrite keeps their names and their links' levels.
    pub prev: Option<&'a ChainProject>,
    pub levels: &'a IndexMap<String, f64>,
    /// A name for a chain no previous chain owned, from its head node.
    pub name_for: Option<NameFor<'a>>,
    /// Nodes written though nothing in `modules` names them: kept as loose chains, hidden.
    pub loose: &'a [String],
    /// Chain names a new chain must not take: files this rewrite leaves alone.
    pub reserved: &'a [String],
    /// Whether a node is a PASS over a picture: what a fade's stretch is made of; absent, no run is one.
    pub is_picture_pass: Option<Test<'a>>,
    /// Whether a lane is a PICTURE lane, a `track` item's file saying so: where a stretch may land.
    pub is_picture_lane: Option<&'a dyn Fn(&str) -> bool>,
}

fn is_sink(n: &str) -> bool {
    CHAIN_SINKS.contains(&n)
}

fn is_sum(n: &str) -> bool {
    n == "MIX" || n == "RENDER"
}

fn bare(d: &str) -> &str {
    d.split('#').next().unwrap_or(d)
}

/// The port a destination names: everything after the first `#`, none when it is empty.
fn port_of(d: &str) -> Option<&str> {
    d.split_once('#').map(|x| x.1).filter(|p| !p.is_empty())
}

struct Stretch {
    lane: String,
    nodes: Vec<String>,
}

/// Each STRETCH in the segments: a series of nodes nothing feeds, none a source, whose only way out is a picture lane.
fn stretch_runs(modules: &IndexMap<String, Vec<String>>, o: &FromSegments) -> Res<Vec<Stretch>> {
    let mut out = vec![];
    let Some(pass) = o.is_picture_pass else { return Ok(out) };
    let mut fed: HashMap<&str, usize> = HashMap::new();
    for d in modules.values().flatten() {
        *fed.entry(bare(d)).or_default() += 1;
    }
    let end = |n: &str| is_sink(n) || !modules.contains_key(n);
    for h in modules.keys() {
        if fed.contains_key(h.as_str()) || end(h) || (o.is_source)(h)? || !pass(h)? {
            continue;
        }
        let mut nodes = vec![h.clone()];
        let mut cur = h.as_str();
        loop {
            let ds = modules.get(cur).map_or(&[][..], Vec::as_slice);
            if ds.len() != 1 || port_of(&ds[0]).is_some() {
                break;
            }
            let d = ds[0].as_str();
            // Asked only of a NODE: a writer's `item_of` may have nothing to say about a sink or a name it never saw.
            let video = !is_sink(d) && (o.item_of)(d).is_ok_and(|i| is_video_item(&i));
            if !is_sink(d) && (video || o.is_picture_lane.is_some_and(|f| f(d))) {
                out.push(Stretch {
                    lane: d.to_owned(),
                    nodes,
                });
                break;
            }
            if end(d) || fed.get(d) != Some(&1) || nodes.iter().any(|n| n == d) || !pass(d)? {
                break;
            }
            nodes.push(d.to_owned());
            cur = d;
        }
    }
    Ok(out)
}

/// A stretch's subchain id when it had none: the leaf of what its head extends, as a chain is named.
fn stretch_id(head: &Item, name: &str) -> String {
    let ext = match head.get("extends") {
        Some(Value::Str(s)) => s.as_str(),
        _ => name,
    };
    let leaf = ext.rsplit('/').next().unwrap_or(name);
    let id: String = leaf
        .strip_prefix('@')
        .unwrap_or(leaf)
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if id.is_empty() { "stretch".to_owned() } else { id }
}

struct Walk<'a> {
    modules: IndexMap<String, Vec<String>>,
    preds: HashMap<String, Vec<String>>,
    port_in: HashSet<String>,
    o: &'a FromSegments<'a>,
}

impl Walk<'_> {
    fn preds_of(&self, n: &str) -> &[String] {
        self.preds.get(n).map_or(&[], Vec::as_slice)
    }

    fn dests_of(&self, n: &str) -> &[String] {
        self.modules.get(n).map_or(&[], Vec::as_slice)
    }

    /// A node heads a chain unless exactly one node feeds it, by its main input, and feeds nothing else.
    fn is_head(&self, n: &str) -> bool {
        if self.port_in.contains(n) {
            return true;
        }
        let p = self.preds_of(n);
        p.len() != 1 || self.dests_of(&p[0]).len() != 1 || is_sum(&p[0])
    }

    /// a → [b1..bk] → d, each bi fed by a alone and feeding d alone, d fed by them alone: one parallel step.
    fn diamond(&self, a: &str) -> Res<Option<(Vec<String>, String)>> {
        let ds = self.dests_of(a);
        let node = |d: &str| !is_sink(d);
        if ds.len() < 2 || ds.iter().any(|d| port_of(d).is_some() || !node(d)) {
            return Ok(None);
        }
        let outs: Vec<&[String]> = ds.iter().map(|d| self.dests_of(d)).collect();
        if outs.iter().any(|x| x.len() != 1 || port_of(&x[0]).is_some()) {
            return Ok(None);
        }
        let join = &outs[0][0];
        if !node(join) || outs.iter().any(|x| x[0] != *join) {
            return Ok(None);
        }
        if ds.iter().any(|d| self.preds_of(d).len() != 1) {
            return Ok(None);
        }
        let jp = self.preds_of(join);
        if jp.len() != ds.len() || jp.iter().any(|p| !ds.contains(p)) {
            return Ok(None);
        }
        for d in ds {
            if lane_of(&(self.o.item_of)(d)?).is_some() {
                return Ok(None);
            }
        }
        Ok(Some((ds.to_vec(), join.clone())))
    }

    /// The first answer `pick` gives along a run, head first, following it while it is one node after another.
    fn along_run(&self, h: &str, pick: &dyn Fn(&str) -> Res<Option<String>>) -> Res<Option<String>> {
        let mut seen: HashSet<String> = HashSet::new();
        let mut cur = Some(h.to_owned());
        while let Some(c) = cur.filter(|c| !seen.contains(c)) {
            seen.insert(c.clone());
            if let Some(found) = pick(&c)? {
                return Ok(Some(found));
            }
            let ds = self.dests_of(&c);
            cur = (ds.len() == 1 && !is_sink(&ds[0]) && port_of(&ds[0]).is_none() && !self.is_head(&ds[0]))
                .then(|| ds[0].clone());
        }
        Ok(None)
    }
}

/// Segments back into chain files: runs break where the signal splits or meets, and a diamond is a parallel step.
///
/// # Errors
/// What the caller's `item_of` and tests fail with.
pub fn from_segments(graph: &GraphIn, o: &FromSegments) -> Res<IndexMap<String, Chain>> {
    // The STRETCHES first, and the rest of the walk never sees them: else the lane is absorbed into the run feeding it.
    let stretches = stretch_runs(&graph.modules, o)?;
    let in_stretch: HashSet<&str> = stretches
        .iter()
        .flat_map(|r| r.nodes.iter().map(String::as_str))
        .collect();
    let modules: IndexMap<String, Vec<String>> = graph
        .modules
        .iter()
        .filter(|(n, _)| !in_stretch.contains(n.as_str()))
        .map(|(n, d)| (n.clone(), d.clone()))
        .collect();
    let bypassed_set: HashSet<&str> = graph.bypassed.iter().map(String::as_str).collect();
    let bus_only: HashSet<&str> = graph.bus_only.iter().map(String::as_str).collect();
    let mut preds: HashMap<String, Vec<String>> = HashMap::new();
    let mut port_in = HashSet::new();
    for (from, dests) in &modules {
        for d in dests {
            let b = bare(d).to_owned();
            preds.entry(b.clone()).or_default().push(from.clone());
            if port_of(d).is_some() {
                port_in.insert(b);
            }
        }
    }
    let w = Walk {
        modules,
        preds,
        port_in,
        o,
    };
    let prev_chain_of = |n: &str| -> Option<String> {
        let was = o.prev?.nodes.get(n)?;
        let called = |sub: &str| {
            o.prev
                .and_then(|p| p.chains.get(&was.chain))
                .is_some_and(|c| c.called_subchains().contains(sub))
        };
        match &was.sub {
            Some(sub) if !called(sub) => Some(sub_key(&was.chain, sub)),
            _ => Some(was.chain.clone()),
        }
    };
    let mut taken: HashSet<String> = o.reserved.iter().cloned().collect();
    let mut chain_name: HashMap<String, String> = HashMap::new();
    let default_name = |head: &str, t: &HashSet<String>| {
        let mut n = head.to_owned();
        let mut i = 2;
        while t.contains(&n) {
            n = format!("{head}-{i}");
            i += 1;
        }
        n
    };

    let mut heads: Vec<String> = ["MIX", "RENDER"]
        .into_iter()
        .filter(|s| w.modules.contains_key(*s))
        .map(str::to_owned)
        .collect();
    let mut in_diamond: HashSet<String> = HashSet::new();
    for a in w.modules.keys() {
        if let Some((mids, join)) = w.diamond(a)? {
            in_diamond.extend(mids);
            in_diamond.insert(join);
        }
    }
    for n in w.modules.keys() {
        if !is_sink(n) && w.is_head(n) && !in_diamond.contains(n) {
            heads.push(n.clone());
        }
    }
    for n in o.loose {
        if !w.modules.contains_key(n) && !heads.contains(n) {
            heads.push(n.clone());
        }
    }
    // A lane a stretch lands on needs a chain to hold it, wired or not.
    for r in &stretches {
        if !w.modules.contains_key(&r.lane) && !heads.contains(&r.lane) {
            heads.push(r.lane.clone());
        }
    }
    let lane_in = |h: &str| w.along_run(h, &|n| Ok(lane_of(&(o.item_of)(n)?).map(str::to_owned)));
    // The subchain a run's nodes came from, so a node put at its head does not lift it out of its parent.
    let sub_in = |h: &str| w.along_run(h, &|n| Ok(prev_chain_of(n).filter(|k| k.contains('/'))));

    // Names first, so a link can name a chain that is written later.
    for h in &heads {
        let sum = is_sum(h);
        let prev = if sum {
            o.prev.and_then(|p| {
                p.chains
                    .iter()
                    .find(|(_, c)| c.start.and_then(Start::sum) == Some(h.as_str()))
                    .map(|(n, _)| n.clone())
            })
        } else {
            match prev_chain_of(h) {
                Some(p) => Some(p),
                None => sub_in(h)?,
            }
        };
        // A run holding a lane is that lane's chain, whatever chain its head came from.
        let lane = if sum { None } else { lane_in(h)? };
        let want = match (&lane, &prev) {
            (Some(l), _) if !taken.contains(l) => l.clone(),
            (_, Some(p)) if !taken.contains(p) => p.clone(),
            _ => {
                let head = if sum {
                    if h == "MIX" {
                        "master".to_owned()
                    } else {
                        "picture".to_owned()
                    }
                } else {
                    lane_in(h)?.unwrap_or_else(|| h.clone())
                };
                match o.name_for {
                    Some(f) => f(&head, &taken),
                    None => default_name(&head, &taken),
                }
            }
        };
        taken.insert(want.clone());
        chain_name.insert(h.clone(), want);
    }

    let item = |n: &str| -> Res<Item> {
        let mut it = (o.item_of)(n)?;
        if bypassed_set.contains(n) {
            set_key(&mut it, "bypass", Value::Bool(true));
        } else {
            remove_key(&mut it, "bypass");
        }
        Ok(it)
    };

    let mut out: IndexMap<String, Chain> = IndexMap::new();
    for h in &heads {
        let mut steps: Vec<Step> = vec![];
        let mut seen: HashSet<String> = HashSet::new();
        let mut cur = h.clone();
        if !is_sum(h) {
            steps.push(Step::One(item(h)?));
            seen.insert(h.clone());
        }
        loop {
            if let Some((mids, join)) = w.diamond(&cur)?
                && !seen.contains(&join)
            {
                steps.push(Step::Many(mids.iter().map(|m| item(m)).collect::<Res<_>>()?));
                steps.push(Step::One(item(&join)?));
                seen.extend(mids);
                seen.insert(join.clone());
                cur = join;
                continue;
            }
            let ds = w.dests_of(&cur);
            if ds.len() == 1
                && !is_sink(&ds[0])
                && port_of(&ds[0]).is_none()
                && !w.is_head(&ds[0])
                && !seen.contains(&ds[0])
            {
                cur = ds[0].clone();
                steps.push(Step::One(item(&cur)?));
                seen.insert(cur.clone());
                continue;
            }
            let mut links: Vec<Link> = ds
                .iter()
                .map(|d| {
                    let b = bare(d);
                    Link {
                        chain: if is_sink(b) {
                            b.to_owned()
                        } else {
                            chain_name.get(b).cloned().unwrap_or_else(|| b.to_owned())
                        },
                        port: port_of(d).map(str::to_owned),
                        level: o.levels.get(&format!("{cur}>{d}")).copied(),
                        port_first: true,
                    }
                })
                .collect();
            if links.len() > 1 {
                for l in &mut links {
                    l.level.get_or_insert(1.0);
                }
            }
            if !links.is_empty() {
                steps.push(Step::End(links));
            }
            break;
        }
        let mut c = Chain {
            steps,
            ..Chain::default()
        };
        if is_sum(h) {
            c.start = Some(if h == "MIX" { Start::Mix } else { Start::Render });
        } else if w.preds_of(h).is_empty() && (o.is_source)(h)? {
            c.start = Some(Start::Own);
        } else {
            let mut ports: Vec<String> = vec![];
            for d in w.modules.values().flatten().filter(|d| bare(d) == h) {
                if let Some(p) = port_of(d)
                    && !ports.iter().any(|x| x == p)
                {
                    ports.push(p.to_owned());
                }
            }
            c.ports = ports;
        }
        if !is_sum(h) && bus_only.contains(h.as_str()) {
            c.bus_only = true;
        }
        if let Some(name) = chain_name.get(h) {
            out.insert(name.clone(), c);
        }
    }
    // Each stretch becomes a subchain of the chain holding its lane, under the id it had or one from its head.
    let mut lane_chain: HashMap<String, String> = HashMap::new();
    for h in &heads {
        if let Some(lane) = if is_sum(h) { None } else { lane_in(h)? }
            && let Some(name) = chain_name.get(h)
        {
            lane_chain.insert(lane, name.clone());
        }
    }
    for r in &stretches {
        let Some(parent) = lane_chain.get(&r.lane) else {
            continue;
        };
        let first = &r.nodes[0];
        let prev_sub = prev_chain_of(first).and_then(|was| {
            let (p, sub) = split_key(&was);
            (p == parent).then(|| sub.map(str::to_owned)).flatten()
        });
        let base = match &prev_sub {
            Some(s) => s.clone(),
            None => stretch_id(&(o.item_of)(first)?, first),
        };
        let mut id = base.clone();
        let mut i = 2;
        while out.contains_key(&sub_key(parent, &id)) {
            id = format!("{base}-{i}");
            i += 1;
        }
        let steps = r
            .nodes
            .iter()
            .map(|n| item(n).map(Step::One))
            .collect::<Res<Vec<_>>>()?;
        out.insert(
            sub_key(parent, &id),
            Chain {
                steps,
                ..Chain::default()
            },
        );
    }
    let places = o.prev.map(|p| PrevPlaces {
        nodes: &p.nodes,
        chains: &p.chains,
    });
    Ok(regroup_calls(nest_subchains(out), places.as_ref()))
}
