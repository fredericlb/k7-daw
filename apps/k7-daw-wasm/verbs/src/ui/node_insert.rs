// node_insert.rs — what the graph's `+` types into the command bar: the creating line, the wiring line, and the name a new node lands with.
// A reader would get wrong the order of the creating line: its trailing space is where the node's name lands, and the arrow goes first for that.

use serde::{Deserialize, Serialize};
use song_edit::instance_name::instance_name;
use song_edit::js_text::is_js_space;
use std::collections::HashSet;

/// One chain of the graph, by the lane that feeds it: the nodes it uses and what it sends to.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ChainNames {
    pub lane: String,
    pub uses: Vec<String>,
    pub outs: Vec<String>,
}

/// The names a graph spells, in the browser's view of it: lanes and chains, picture lanes, notes and curves.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct GraphNames {
    #[serde(default)]
    pub tracks: Vec<String>,
    #[serde(default)]
    pub chains: Vec<ChainNames>,
    #[serde(default)]
    pub video_tracks: Vec<String>,
    #[serde(default)]
    pub modules: Vec<String>,
    #[serde(default)]
    pub curves: Vec<String>,
}

/// The pending half of a `+` click: the arrow the module is being dropped onto.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PendingInsert {
    pub before: String,
    pub after: String,
    /// A bottom `+`: the new node is stacked under `before` rather than spliced into an arrow.
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub stack: Option<bool>,
}

/// Where the click landed, in the canvas's own units.
#[derive(Clone, Copy, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Spawn {
    pub x: f64,
    pub y: f64,
}

/// The node of a `before`: its text without a port.
fn node_of(s: &str) -> &str {
    s.split('#').next().unwrap_or("")
}

/// The lane a `+` on this arrow drops onto: a chain is called after its lane, so the key is the answer, port or no port.
#[must_use]
pub fn lane_of_arrow<'a>(graph: &'a GraphNames, before: Option<&str>) -> Option<&'a str> {
    let node = node_of(before.filter(|b| !b.is_empty())?);
    if let Some(c) = graph.chains.iter().find(|c| c.lane == node) {
        return Some(&c.lane);
    }
    graph
        .chains
        .iter()
        .find(|c| c.uses.iter().any(|u| node_of(u) == node))
        .map(|c| c.lane.as_str())
}

/// Every name this graph already spells: only what the browser can see, and `node new` refuses a collision out loud.
fn names_in(graph: &GraphNames) -> HashSet<&str> {
    let mut out: HashSet<&str> = graph.tracks.iter().map(String::as_str).collect();
    for c in &graph.chains {
        out.insert(&c.lane);
        out.extend(c.uses.iter().map(|u| node_of(u)));
        out.extend(c.outs.iter().map(|o| node_of(o)));
    }
    out.extend(graph.video_tracks.iter().map(String::as_str));
    out.extend(graph.modules.iter().map(String::as_str));
    out.extend(graph.curves.iter().map(String::as_str));
    out
}

/// What a node of `module` dropped on this arrow should be called: the lane read off the arrow, the first free number after it.
#[must_use]
pub fn node_name_for(graph: &GraphNames, module: &str, before: Option<&str>) -> String {
    let taken = names_in(graph);
    instance_name(module, lane_of_arrow(graph, before), &|n| taken.contains(n))
}

/// What a chain should name, given the creating command that just ran: a node is named by its file alone.
#[must_use]
pub fn chain_ref_from_create(line: &str) -> Option<String> {
    let parts: Vec<&str> = line
        .trim_matches(is_js_space)
        .split(is_js_space)
        .filter(|p| !p.is_empty())
        .collect();
    if parts.first() != Some(&"node") || parts.get(1) != Some(&"new") {
        return None;
    }
    let args = &parts[2..];
    // "<type>/<name>": the module's own folder. Anything else is a line the command rejects.
    let segs: Vec<&str> = args.first().copied().unwrap_or("").split('/').collect();
    if segs.len() != 2 || segs[0].is_empty() || segs[1].is_empty() {
        return None;
    }
    // A catalogue pick's ref rides on the line; a node name never carries a slash, so that is the filter.
    args.iter().skip(1).find(|p| !p.contains('/')).map(|p| (*p).to_owned())
}

/// The line the `+` pre-writes: `node new`, which puts the node in the graph itself.
#[must_use]
pub fn create_for_graph_command(kind: &str, name: &str) -> String {
    format!("node new {kind}/{name} ")
}

/// The catalogue ref that rides on the line, which the node extends directly.
#[must_use]
pub fn node_new_ref_arg(catalogue_ref: Option<&str>) -> String {
    catalogue_ref
        .filter(|r| !r.is_empty())
        .map_or_else(String::new, |r| format!(" {r}"))
}

/// `Math.round`: halves go up.
fn round(n: f64) -> i64 {
    (n + 0.5).floor() as i64
}

/// Where the node that line makes should go: a splice on an arrow, and on the bare canvas only the pin, and only with a point.
#[must_use]
pub fn wire_command(node: &str, at: Option<&PendingInsert>, spawn: Option<Spawn>) -> Option<String> {
    let (x, y) = spawn.map_or((0, 0), |s| (round(s.x), round(s.y)));
    match at {
        Some(a) if a.stack == Some(true) => Some(format!("node stack {} {node}", a.before)),
        Some(a) => Some(format!(
            "node insert {} {} {node}{}",
            a.before,
            a.after,
            if spawn.is_some() {
                format!(" {x},{y}")
            } else {
                String::new()
            }
        )),
        None => spawn.map(|_| format!("graph move {node} {x} {y}")),
    }
}

/// The answer of `lane_of_arrow` as the wire carries it.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Lane {
    pub lane: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chain(lane: &str, uses: &[&str]) -> ChainNames {
        ChainNames {
            lane: lane.into(),
            uses: uses.iter().map(|s| (*s).to_owned()).collect(),
            outs: vec!["OUT".into()],
        }
    }

    fn graph(chains: Vec<ChainNames>) -> GraphNames {
        GraphNames {
            tracks: chains.iter().map(|c| c.lane.clone()).collect(),
            chains,
            video_tracks: vec![],
            modules: vec![],
            curves: vec![],
        }
    }

    fn arrow() -> PendingInsert {
        PendingInsert {
            before: "melody".into(),
            after: "OUT".into(),
            stack: None,
        }
    }

    #[test]
    fn the_plus_creates_the_node_on_a_line_ending_where_its_name_goes() {
        let line = create_for_graph_command("audio", "adt");
        assert_eq!(line, "node new audio/adt ");
        assert!(line.ends_with(' '));
        assert_eq!(line.split_whitespace().count(), 3);
    }

    #[test]
    fn the_catalogue_ref_goes_between_the_name_and_the_spawn_token() {
        assert_eq!(
            format!(
                "{}a1{} 120,80",
                create_for_graph_command("audio", "adt"),
                node_new_ref_arg(Some("@engine/audio/delay/adt"))
            ),
            "node new audio/adt a1 @engine/audio/delay/adt 120,80"
        );
        assert_eq!(node_new_ref_arg(None), "");
        assert_eq!(
            chain_ref_from_create("node new audio/adt a1 @engine/audio/delay/adt").as_deref(),
            Some("a1")
        );
    }

    #[test]
    fn the_shell_wires_by_splicing_pinning_or_stacking() {
        assert_eq!(
            wire_command("adt1", Some(&arrow()), Some(Spawn { x: 300.4, y: 140.6 })).as_deref(),
            Some("node insert melody OUT adt1 300,141")
        );
        assert_eq!(
            wire_command("adt1", None, Some(Spawn { x: 12.0, y: 8.0 })).as_deref(),
            Some("graph move adt1 12 8")
        );
        assert_eq!(wire_command("adt1", None, None), None);
        let stack = PendingInsert {
            before: "amp".into(),
            after: "amp".into(),
            stack: Some(true),
        };
        assert_eq!(
            wire_command("adt1", Some(&stack), Some(Spawn { x: 1.0, y: 2.0 })).as_deref(),
            Some("node stack amp adt1")
        );
        assert_eq!(
            wire_command("adt1", Some(&arrow()), None).as_deref(),
            Some("node insert melody OUT adt1")
        );
        let neural = PendingInsert {
            before: "neural-amp-modeler".into(),
            after: "OUT".into(),
            stack: None,
        };
        assert!(
            wire_command("wah1", Some(&neural), None)
                .unwrap()
                .contains("neural-amp-modeler OUT")
        );
    }

    #[test]
    fn the_node_is_read_back_off_a_node_new_line_and_nothing_else() {
        assert_eq!(chain_ref_from_create("node new audio/adt a1").as_deref(), Some("a1"));
        assert_eq!(chain_ref_from_create("module new audio/adt"), None);
        assert_eq!(
            chain_ref_from_create("module new audio/adt @engine/audio/delay/adt"),
            None
        );
        assert_eq!(chain_ref_from_create("node insert gtr amp adt1"), None);
        assert_eq!(chain_ref_from_create("node new adt"), None);
    }

    fn lanes() -> GraphNames {
        graph(vec![chain("vo.jo", &["amp", "eq"]), chain("da.main", &["comp"])])
    }

    #[test]
    fn an_arrow_is_on_the_lane_that_feeds_or_plays_it() {
        let g = lanes();
        assert_eq!(lane_of_arrow(&g, Some("vo.jo")), Some("vo.jo"));
        assert_eq!(lane_of_arrow(&g, Some("amp")), Some("vo.jo"));
        assert_eq!(lane_of_arrow(&g, Some("eq")), Some("vo.jo"));
        assert_eq!(lane_of_arrow(&g, Some("comp")), Some("da.main"));
        // A port says nothing about which run this is.
        assert_eq!(lane_of_arrow(&g, Some("amp#b")), Some("vo.jo"));
        assert_eq!(lane_of_arrow(&g, Some("vo.jo#midi")), Some("vo.jo"));
        assert_eq!(lane_of_arrow(&g, None), None);
        assert_eq!(lane_of_arrow(&g, Some("")), None);
        assert_eq!(lane_of_arrow(&g, Some("stranger")), None);
    }

    #[test]
    fn a_new_node_lands_as_lane_module_number_over_every_name_taken() {
        let g = graph(vec![chain("vo.jo", &["amp"])]);
        assert_eq!(node_name_for(&g, "reverb", Some("amp")), "vo_jo_reverb_1");
        assert_eq!(node_name_for(&g, "reverb", None), "reverb_1");
        let taken = graph(vec![chain("vo.jo", &["amp", "vo_jo_reverb_1"])]);
        assert_eq!(node_name_for(&taken, "reverb", Some("amp")), "vo_jo_reverb_2");
        let mut busy = graph(vec![chain("vo.jo", &["amp"])]);
        busy.curves = vec!["vo_jo_reverb_1".into()];
        busy.modules = vec!["vo_jo_reverb_2".into()];
        assert_eq!(node_name_for(&busy, "reverb", Some("amp")), "vo_jo_reverb_3");
    }

    #[test]
    fn the_two_halves_join_on_the_trailing_space() {
        let g = graph(vec![chain("vo.jo", &["amp"])]);
        let line = format!(
            "{}{}",
            create_for_graph_command("audio", "reverb"),
            node_name_for(&g, "reverb", Some("amp"))
        );
        assert_eq!(line, "node new audio/reverb vo_jo_reverb_1");
        assert_eq!(chain_ref_from_create(&line).as_deref(), Some("vo_jo_reverb_1"));
    }
}
