// ops/chains.rs — the Chains tab's rules as JSON ops over the `chains` crate: a widget's value maths, the cables' flow, the lane reads, the bindings.
// A reader would get wrong that the graph travels whole in a lane read or a binding: each runs once per document change, not per frame.

use crate::error::WasmError;
use crate::ops::write;
use chains::assembly::fold_assemblies;
use chains::bindings::{self, BindGraph, PluglessPolicy, ResolvedMapping, SurfaceBinding};
use chains::build::{BuildOptions, build_graph};
use chains::curve_placement::{self, Box2, Moved, PlaceEdge};
use chains::flow::{self, FlowEdge, LaneSource, NodeFlow};
use chains::geom::node_size;
use chains::graph::Graph;
use chains::module_row::{RankNode, RowOptions, Spaced, align_ranks, module_row, space_modules};
use chains::node::{Edge, EdgeInsert, Laid, Node};
use chains::panel::Panel;
use chains::row_seams::{SeamsAnswer, inserts_for, jack_ref, row_exit, row_seams};
use chains::stack_layout::{StackAnswer, stack_layout};
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

/// The graph to build, how, and which assemblies are open when it has any.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct BuildArgs {
    #[cfg_attr(feature = "ts", ts(type = "import(\"@studio/graph-model\").WebGraph"))]
    pub graph: Graph,
    #[serde(default)]
    pub opts: BuildOptions,
    /// Fold the graph's assemblies, these ones drawn open; absent leaves the graph as built.
    pub open: Option<Vec<String>>,
}

/// What a build answers: the cards, and the cables between them, unpositioned.
#[derive(Clone, Debug, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Built {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

pub fn graph_build(a: BuildArgs) -> Built {
    let (nodes, edges) = build_graph(&a.graph, &a.opts);
    let (nodes, edges) = match &a.open {
        Some(open) => fold_assemblies(&a.graph, nodes, edges, &open.iter().cloned().collect()),
        None => (nodes, edges),
    };
    Built { nodes, edges }
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct RowArgs {
    #[cfg_attr(feature = "ts", ts(type = "import(\"@studio/graph-model\").WebGraph"))]
    pub graph: Graph,
    pub modules: Vec<String>,
    #[serde(default)]
    pub opts: RowOptions,
}

pub fn graph_row(a: RowArgs) -> Laid {
    module_row(&a.graph, &a.modules, &a.opts)
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RankArgs {
    pub nodes: Vec<RankNode>,
    pub modules: Vec<String>,
}

pub fn graph_align(a: RankArgs) -> Vec<(String, f64)> {
    align_ranks(&a.nodes, &a.modules)
}

pub fn graph_space(a: RankArgs) -> Option<Spaced> {
    space_modules(&a.nodes, &a.modules)
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SeamsArgs {
    /// The row as laid before a card was lifted off its end, and the row shown.
    pub full: Laid,
    pub row: Laid,
    #[cfg_attr(feature = "ts", ts(type = "import(\"@studio/graph-model\").WebGraph"))]
    pub graph: Graph,
    pub branch: bool,
}

pub fn graph_seams(a: SeamsArgs) -> SeamsAnswer {
    let exit = row_exit(&a.full, &a.row, &a.graph);
    row_seams(&a.row, exit, a.branch)
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct InsertsArgs {
    pub nodes: Vec<Node>,
    pub pairs: Vec<(String, String)>,
}

pub fn graph_inserts(a: InsertsArgs) -> Vec<Option<EdgeInsert>> {
    inserts_for(&a.nodes, &a.pairs)
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct JackRefArgs {
    pub node: Node,
    pub port: Option<String>,
}

pub fn graph_jack_ref(a: JackRefArgs) -> String {
    jack_ref(&a.node, a.port.as_deref())
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(rename = "StackArgs"))]
#[serde(rename_all = "camelCase")]
pub struct StackArgsOp {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    #[cfg_attr(feature = "ts", ts(type = "import(\"@studio/graph-model\").WebGraph"))]
    pub graph: Graph,
    pub track_order: Vec<String>,
}

pub fn graph_stack(a: StackArgsOp) -> StackAnswer {
    stack_layout(&a.nodes, &a.edges, &a.graph, &a.track_order)
}

/// A panel and whether the card carries its bottom bar: what `graph.nodeSize` measures.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct NodeSizeArgs {
    #[cfg_attr(feature = "ts", ts(type = "import(\"@studio/graph-model\").PageModuleUI"))]
    pub ui: Option<Panel>,
    pub footer: Option<bool>,
}

/// A card's size.
#[derive(Clone, Debug, serde::Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Size {
    pub w: f64,
    pub h: f64,
}

pub fn graph_node_size(a: NodeSizeArgs) -> Size {
    let (w, h) = node_size(a.ui.as_ref(), a.footer == Some(true));
    Size { w, h }
}

/// The chains families' requests: `{"op": "widget.norm", "args": {…}}`, and the flow, lane, bindings and place families beside it.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "op", content = "args")]
// A request is decoded once and consumed at once, so its largest variant costs nothing held.
#[allow(clippy::large_enum_variant)]
pub enum ChainsRequest {
    #[serde(rename = "widget.norm")]
    WidgetNorm(WidgetValueArgs),
    #[serde(rename = "widget.denorm")]
    WidgetDenorm(WidgetValueArgs),
    #[serde(rename = "widget.pair")]
    WidgetPair(WidgetValueArgs),
    #[serde(rename = "widget.text")]
    WidgetText(WidgetTextArgs),
    #[serde(rename = "widget.typed")]
    WidgetTyped(WidgetTypedArgs),
    #[serde(rename = "widget.mix")]
    WidgetMix(WidgetOnlyArgs),
    #[serde(rename = "flow.compute")]
    FlowCompute(FlowArgs),
    #[serde(rename = "flow.seeds")]
    FlowSeeds(FlowSeedsArgs),
    #[serde(rename = "lane.chains")]
    LaneChains(LaneArgs),
    #[serde(rename = "lane.modules")]
    LaneModules(LaneArgs),
    #[serde(rename = "lane.inputs")]
    LaneInputs(LaneArgs),
    #[serde(rename = "bindings.resolve")]
    BindingsResolve(BindingsArgs),
    #[serde(rename = "bindings.unmapped")]
    BindingsUnmapped(UnmappedArgs),
    #[serde(rename = "place.targets")]
    PlaceTargets(PlaceTargetsArgs),
    #[serde(rename = "place.rowKeys")]
    PlaceRowKeys(PlaceArgs),
    #[serde(rename = "place.leftOf")]
    PlaceLeftOf(PlaceArgs),
    #[serde(rename = "place.underLeft")]
    PlaceUnderLeft(PlaceArgs),
    #[serde(rename = "place.withoutCurves")]
    PlaceWithoutCurves(WithoutCurvesArgs),
    #[serde(rename = "graph.build")]
    GraphBuild(BuildArgs),
    #[serde(rename = "graph.row")]
    GraphRow(RowArgs),
    #[serde(rename = "graph.align")]
    GraphAlign(RankArgs),
    #[serde(rename = "graph.space")]
    GraphSpace(RankArgs),
    #[serde(rename = "graph.seams")]
    GraphSeams(SeamsArgs),
    #[serde(rename = "graph.inserts")]
    GraphInserts(InsertsArgs),
    #[serde(rename = "graph.jackRef")]
    GraphJackRef(JackRefArgs),
    #[serde(rename = "graph.stack")]
    GraphStack(StackArgsOp),
    #[serde(rename = "graph.nodeSize")]
    GraphNodeSize(NodeSizeArgs),
}

pub fn dispatch(request: ChainsRequest, out: &mut Vec<u8>) -> Result<(), WasmError> {
    use ChainsRequest as R;
    match request {
        R::WidgetNorm(a) => write(out, &widget_norm(a)),
        R::WidgetDenorm(a) => write(out, &widget_denorm(a)),
        R::WidgetPair(a) => write(out, &widget_pair(a)),
        R::WidgetText(a) => write(out, &widget_text(a)),
        R::WidgetTyped(a) => write(out, &widget_typed(a)),
        R::WidgetMix(a) => write(out, &widget_is_mix(a)),
        R::FlowCompute(a) => write(out, &flow_compute(a)),
        R::FlowSeeds(a) => write(out, &flow_seeds(a)),
        R::LaneChains(a) => write(out, &subgraph_chains(a)),
        R::LaneModules(a) => write(out, &subgraph_modules(a)),
        R::LaneInputs(a) => write(out, &subgraph_inputs(a)),
        R::BindingsResolve(a) => write(out, &bindings_resolve(a)),
        R::BindingsUnmapped(a) => write(out, &bindings_unmapped(a)),
        R::PlaceTargets(a) => write(out, &place_targets(a)),
        R::PlaceRowKeys(a) => write(out, &place_row_keys(a)),
        R::PlaceLeftOf(a) => write(out, &place_left_of(a)),
        R::PlaceUnderLeft(a) => write(out, &place_under_left(a)),
        R::PlaceWithoutCurves(a) => write(out, &place_without_curves(a)),
        R::GraphBuild(a) => write(out, &graph_build(a)),
        R::GraphRow(a) => write(out, &graph_row(a)),
        R::GraphAlign(a) => write(out, &graph_align(a)),
        R::GraphSpace(a) => write(out, &graph_space(a)),
        R::GraphSeams(a) => write(out, &graph_seams(a)),
        R::GraphInserts(a) => write(out, &graph_inserts(a)),
        R::GraphJackRef(a) => write(out, &graph_jack_ref(a)),
        R::GraphStack(a) => write(out, &graph_stack(a)),
        R::GraphNodeSize(a) => write(out, &graph_node_size(a)),
    }
}
