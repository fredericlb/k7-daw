// chains_ops.rs — the Chains tab's rules as JSON ops over the `chains` crate: a widget's value maths, the cables' flow, the lane reads, the bindings.
// A reader would get wrong that the graph travels whole in a lane read or a binding: each runs once per document change, not per frame.

use chains::bindings::{self, BindGraph, PluglessPolicy, ResolvedMapping, SurfaceBinding};
use chains::curve_placement::{self, Box2, Moved, PlaceEdge};
use chains::flow::{self, FlowEdge, LaneSource, NodeFlow};
use chains::subgraph::{self, GraphView, KeptChain};
use chains::widget_value::WidgetScale;
use indexmap::{IndexMap, IndexSet};
use serde::Deserialize;
use std::collections::HashMap;

/// A widget's scale and a value on it: what `widget.norm`, `widget.denorm` and `widget.pair` take.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct WidgetValueArgs {
    pub widget: WidgetScale,
    pub value: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct WidgetTextArgs {
    pub widget: WidgetScale,
    pub value: Option<f64>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct WidgetTypedArgs {
    pub widget: WidgetScale,
    pub text: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct WidgetOnlyArgs {
    pub widget: WidgetScale,
}

pub fn widget_norm(a: WidgetValueArgs) -> f64 {
    a.widget.norm(a.value)
}

pub fn widget_denorm(a: WidgetValueArgs) -> f64 {
    a.widget.denorm(a.value)
}

pub fn widget_pair(a: WidgetValueArgs) -> f64 {
    a.widget.pair_value(a.value)
}

pub fn widget_text(a: WidgetTextArgs) -> String {
    a.widget.text(a.value)
}

pub fn widget_typed(a: WidgetTypedArgs) -> Option<f64> {
    a.widget.typed(&a.text)
}

pub fn widget_is_mix(a: WidgetOnlyArgs) -> bool {
    a.widget.is_mix_knob()
}

/// The cables, the seeds the lanes put out, and where a folded lane's signal leaves from.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
#[serde(rename_all = "camelCase")]
pub struct FlowArgs {
    pub edges: Vec<FlowEdge>,
    pub seeds: Vec<(String, NodeFlow)>,
    pub folded_at: Option<Vec<(String, String)>>,
}

pub fn flow_compute(a: FlowArgs) -> Vec<(String, f64)> {
    let folded: HashMap<String, String> = a.folded_at.unwrap_or_default().into_iter().collect();
    flow::compute_flow(&a.edges, &a.seeds, &folded).into_iter().collect()
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FlowSeedsArgs {
    #[cfg_attr(feature = "ts", ts(as = "HashMap<String, LaneSource>"))]
    pub lanes: IndexMap<String, LaneSource>,
}

pub fn flow_seeds(a: FlowSeedsArgs) -> Vec<(String, NodeFlow)> {
    flow::seed_flows(&a.lanes).into_iter().collect()
}

/// A graph and the lane a read is about.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LaneArgs {
    #[cfg_attr(feature = "ts", ts(type = "import(\"@studio/graph-model\").WebGraph"))]
    pub graph: GraphView,
    pub track: String,
}

pub fn subgraph_chains(a: LaneArgs) -> Vec<KeptChain> {
    subgraph::subgraph_chains(&a.graph, &a.track)
}

pub fn subgraph_modules(a: LaneArgs) -> Vec<String> {
    subgraph::track_modules(&a.graph, &a.track)
}

pub fn subgraph_inputs(a: LaneArgs) -> Vec<String> {
    subgraph::track_inputs(&a.graph, &a.track)
}

/// A surface's mapping, the graph it joins, and the controls the surface has.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
#[serde(rename_all = "camelCase")]
pub struct BindingsArgs {
    #[cfg_attr(feature = "ts", ts(as = "Option<HashMap<String, Option<SurfaceBinding>>>"))]
    pub mapping: Option<IndexMap<String, Option<SurfaceBinding>>>,
    #[cfg_attr(feature = "ts", ts(type = "import(\"@studio/graph-model\").WebGraph | null"))]
    pub graph: Option<BindGraph>,
    pub controls: Vec<String>,
    pub noun: String,
    pub plugless: PluglessPolicy,
    /// Resolve each line's second target too: the desk does, the mastering strip does not.
    pub with_also: bool,
}

pub fn bindings_resolve(a: BindingsArgs) -> ResolvedMapping {
    let controls: IndexSet<String> = a.controls.into_iter().collect();
    let mapping = a.mapping.unwrap_or_default();
    bindings::resolve_mapping(
        &mapping,
        &a.graph.unwrap_or_default(),
        &controls,
        &a.noun,
        a.plugless,
        a.with_also,
    )
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct UnmappedArgs {
    #[cfg_attr(feature = "ts", ts(as = "Option<HashMap<String, Option<SurfaceBinding>>>"))]
    pub mapping: Option<IndexMap<String, Option<SurfaceBinding>>>,
    pub controls: Vec<String>,
}

pub fn bindings_unmapped(a: UnmappedArgs) -> Vec<String> {
    bindings::unmapped_controls(&a.controls, &a.mapping.unwrap_or_default())
}

/// The cards a placement reads and the curve-to-target map it follows.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PlaceArgs {
    pub nodes: Vec<Box2>,
    #[cfg_attr(feature = "ts", ts(as = "HashMap<String, String>"))]
    pub targets: IndexMap<String, String>,
}

/// The cables of a graph and the ids of its curve cards: what `place.targets` reads.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(rename = "PlaceTargetsArgs"))]
#[serde(rename_all = "camelCase")]
pub struct PlaceTargetsArgs {
    pub edges: Vec<PlaceEdge>,
    pub curve_ids: Vec<String>,
}

pub fn place_targets(a: PlaceTargetsArgs) -> IndexMap<String, String> {
    let curves: IndexSet<&str> = a.curve_ids.iter().map(String::as_str).collect();
    curve_placement::curve_targets(&a.edges, |id| curves.contains(id))
}

pub fn place_row_keys(a: PlaceArgs) -> Vec<f64> {
    curve_placement::row_sort_keys(&a.nodes, &a.targets)
}

pub fn place_left_of(a: PlaceArgs) -> Vec<Moved> {
    curve_placement::curves_left_of_targets(&a.nodes, &a.targets)
}

pub fn place_under_left(a: PlaceArgs) -> Vec<Moved> {
    curve_placement::curves_under_left(&a.nodes, &a.targets)
}

/// Each node's id and kind, and each cable's two ends.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct WithoutCurvesArgs {
    pub nodes: Vec<(String, String)>,
    pub edges: Vec<(String, String)>,
}

/// What the zoomed-out view keeps: the indexes of its nodes and cables, or none when the graph holds no curve.
#[derive(Clone, Debug, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Kept {
    pub nodes: Vec<usize>,
    pub edges: Vec<usize>,
}

pub fn place_without_curves(a: WithoutCurvesArgs) -> Option<Kept> {
    curve_placement::without_curves(&a.nodes, &a.edges).map(|(nodes, edges)| Kept { nodes, edges })
}
