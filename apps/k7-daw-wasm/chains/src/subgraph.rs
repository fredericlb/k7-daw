// subgraph.rs — one lane's slice of the routing graph: the chains it feeds, the modules on them, and the inputs that feed it.
// A reader would get wrong that the graph is read, never built: the page's graph is a JSON document and only the names and flags come back.

use indexmap::{IndexMap, IndexSet};
use serde::{Deserialize, Serialize};

/// What the module type folders call a picture pass: the one the compiler and the page agree on.
fn is_picture_pass(category: Option<&str>) -> bool {
    matches!(category, Some("shaders") | Some("video_effects"))
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct OutView {
    #[serde(default)]
    pub to: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct StepView {
    pub module: Option<String>,
    pub category: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ChainView {
    pub from: Option<String>,
    #[serde(default)]
    pub out: Vec<OutView>,
    #[serde(default, rename = "use")]
    pub steps: Vec<StepView>,
}

/// The part of the page's graph the lane reads need.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct GraphView {
    #[serde(default)]
    pub chains: IndexMap<String, ChainView>,
    #[serde(default, rename = "videoTracks")]
    pub video_tracks: Option<Vec<String>>,
}

/// A chain a track's slice keeps, and whether its `from` must go: it names another track the slice does not hold.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct KeptChain {
    pub name: String,
    #[serde(rename = "dropFrom")]
    #[cfg_attr(feature = "ts", ts(rename = "dropFrom"))]
    pub drop_from: bool,
}

fn port_free(to: &str) -> &str {
    to.split('#').next().unwrap_or(to)
}

/// The chains a track's signal flows through, in the order they were reached: those it feeds, and those they route to or feed in turn.
pub fn subgraph_chains(graph: &GraphView, track: &str) -> Vec<KeptChain> {
    fn walk<'a>(graph: &'a GraphView, name: &'a str, keep: &mut IndexSet<&'a str>) {
        let Some(chain) = graph.chains.get(name) else { return };
        if !keep.insert(name) {
            return;
        }
        for o in &chain.out {
            walk(graph, &o.to, keep);
        }
        for (n, c) in &graph.chains {
            if c.from.as_deref() == Some(name) {
                walk(graph, n, keep);
            }
        }
    }
    let mut keep = IndexSet::new();
    for (name, c) in &graph.chains {
        if c.from.as_deref() == Some(track) {
            walk(graph, name, &mut keep);
        }
    }
    keep.into_iter()
        .map(|name| {
            let from = graph.chains[name].from.as_deref();
            let foreign = from.is_some_and(|f| !f.is_empty() && f != track && !graph.chains.contains_key(f));
            KeptChain {
                name: name.to_owned(),
                drop_from: foreign,
            }
        })
        .collect()
}

/// The modules one track's signal reaches, in the order it reaches them, its own chains first.
pub fn track_modules(graph: &GraphView, track: &str) -> Vec<String> {
    let kept = subgraph_chains(graph, track);
    let own = |k: &&KeptChain| graph.chains[&k.name].from.as_deref() == Some(track);
    let names = kept.iter().filter(own).chain(kept.iter().filter(|k| !own(k)));
    let mut seen = IndexSet::new();
    for k in names {
        for step in &graph.chains[&k.name].steps {
            if let Some(m) = step.module.as_deref().filter(|m| !m.is_empty()) {
                seen.insert(m.to_owned());
            }
        }
    }
    seen.into_iter().collect()
}

/// Picture passes nothing feeds, landing on a picture lane: a fade's stretch, which carries no signal.
fn is_stretch(graph: &GraphView, c: &ChainView, target: &str) -> bool {
    let Some(head) = c
        .steps
        .first()
        .and_then(|s| s.module.as_deref())
        .filter(|m| !m.is_empty())
    else {
        return false;
    };
    if !graph
        .video_tracks
        .as_ref()
        .is_some_and(|v| v.iter().any(|t| t == target))
    {
        return false;
    }
    if !c.steps.iter().all(|s| is_picture_pass(s.category.as_deref())) {
        return false;
    }
    !graph
        .chains
        .values()
        .any(|o| o.out.iter().any(|x| port_free(&x.to) == head))
}

/// The modules feeding one track, its input and anything between that and the lane, farthest first.
pub fn track_inputs(graph: &GraphView, track: &str) -> Vec<String> {
    fn upstream(graph: &GraphView, target: &str, seen: &mut IndexSet<String>) -> Vec<String> {
        if !seen.insert(target.to_owned()) {
            return Vec::new();
        }
        let mut out = Vec::new();
        for c in graph.chains.values() {
            if c.from.as_deref().is_some_and(|f| !f.is_empty()) || !c.out.iter().any(|o| port_free(&o.to) == target) {
                continue;
            }
            if is_stretch(graph, c, target) {
                continue;
            }
            let run: Vec<&str> = c
                .steps
                .iter()
                .filter_map(|s| s.module.as_deref())
                .filter(|m| !m.is_empty())
                .collect();
            if let Some(first) = run.first() {
                out.extend(upstream(graph, first, seen));
                out.extend(run.iter().map(|m| (*m).to_owned()));
            }
        }
        out
    }
    let mut seen = IndexSet::new();
    upstream(graph, track, &mut seen)
        .into_iter()
        .collect::<IndexSet<_>>()
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn graph(v: serde_json::Value) -> GraphView {
        serde_json::from_value(v).unwrap_or_default()
    }

    fn names(k: &[KeptChain]) -> Vec<&str> {
        k.iter().map(|k| k.name.as_str()).collect()
    }

    #[test]
    fn a_track_keeps_only_the_chains_its_signal_flows_through() {
        let g = graph(json!({"chains": {
            "lead": {"from": "melody", "use": [{"module": "s"}], "out": [{"to": "OUT", "gain": 1}]},
            "low": {"from": "bass", "use": [{"module": "s"}], "out": [{"to": "OUT", "gain": 1}]},
        }}));
        assert_eq!(names(&subgraph_chains(&g, "melody")), vec!["lead"]);
    }

    #[test]
    fn a_chain_fed_by_another_lane_loses_that_input() {
        let g = graph(json!({"chains": {
            "a": {"from": "vox", "use": [], "out": [{"to": "b", "gain": 1}]},
            "b": {"from": "mic", "use": [], "out": []},
        }}));
        let kept = subgraph_chains(&g, "vox");
        assert_eq!(
            kept,
            vec![
                KeptChain {
                    name: "a".into(),
                    drop_from: false
                },
                KeptChain {
                    name: "b".into(),
                    drop_from: true
                },
            ]
        );
    }

    fn song() -> GraphView {
        graph(json!({"chains": {
            "vox": {"from": "vox", "use": [{"module": "comp"}, {"module": "eq"}],
                    "out": [{"to": "OUT", "gain": 1}, {"to": "reverb", "gain": 0.3}]},
            "reverb": {"use": [{"module": "conv"}], "out": [{"to": "OUT", "gain": 1}]},
            "bass": {"from": "bass", "use": [{"module": "drive"}], "out": [{"to": "OUT", "gain": 1}]},
        }}))
    }

    #[test]
    fn a_lane_s_row_is_its_modules_in_signal_order_its_own_chain_first() {
        assert_eq!(track_modules(&song(), "vox"), vec!["comp", "eq", "conv"]);
        assert_eq!(track_modules(&song(), "bass"), vec!["drive"]);
        assert!(track_modules(&song(), "gone").is_empty());
    }

    #[test]
    fn a_shared_module_is_drawn_once() {
        let g = graph(json!({"chains": {
            "a": {"from": "vox", "use": [{"module": "glue"}], "out": [{"to": "b", "gain": 1}]},
            "b": {"use": [{"module": "glue"}], "out": [{"to": "OUT", "gain": 1}]},
        }}));
        assert_eq!(track_modules(&g, "vox"), vec!["glue"]);
    }

    #[test]
    fn the_inputs_feeding_a_lane_come_farthest_first() {
        let g = graph(json!({"chains": {
            "mic": {"use": [{"module": "input"}, {"module": "gate"}], "out": [{"to": "vox#in", "gain": 1}]},
        }}));
        assert_eq!(track_inputs(&g, "vox"), vec!["input", "gate"]);
    }

    #[test]
    fn a_fade_s_stretch_feeds_no_signal() {
        let g = graph(json!({"videoTracks": ["cam"], "chains": {
            "fade": {"use": [{"module": "blur", "category": "video_effects"}], "out": [{"to": "cam", "gain": 1}]},
        }}));
        assert!(track_inputs(&g, "cam").is_empty());
    }
}
