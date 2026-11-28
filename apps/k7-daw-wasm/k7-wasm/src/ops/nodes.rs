// ops/nodes.rs — the nodes.* operations: a selection of cards and what it links, a node's shown name, a group's edge, a row's pinned mixer.
// A reader would get wrong that the rules are in the `chains` crate: this file is the wire (names and shapes) over them.

use crate::error::WasmError;
use crate::ops::write;
use chains::anchor_right::split_trailing_mixer;
use chains::group_edge::group_edge_insets;
use chains::node::Laid;
use chains::panel::Widgets;
use chains::select::{DragBox, SelectNode, linked_nodes, nodes_in_box, toggle_selection};
use chains::shown_name::{ShownStep, shown_names};
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct ToggleArgs {
    pub current: Vec<String>,
    pub primary: Option<String>,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LinkedArgs {
    pub nodes: Vec<SelectNode>,
    pub selection: Vec<String>,
    pub of: SelectNode,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(rename = "InBoxArgs"))]
pub struct BoxArgs {
    pub nodes: Vec<SelectNode>,
    #[serde(rename = "box")]
    #[cfg_attr(feature = "ts", ts(rename = "box"))]
    pub rect: DragBox,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ShownArgs {
    pub steps: Vec<ShownStep>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct GroupEdgeArgs {
    #[cfg_attr(
        feature = "ts",
        ts(type = "Record<string, import(\"@studio/graph-model\").PageWidget>")
    )]
    pub widgets: Widgets,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(rename = "SplitMixerArgs"))]
pub struct SplitArgs {
    pub row: Laid,
}

/// The nodes family's requests: `{"op": "nodes.linked", "args": {…}}`.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "op", content = "args")]
// A request is decoded once and consumed at once, so its largest variant costs nothing held.
#[allow(clippy::large_enum_variant)]
pub enum NodesRequest {
    #[serde(rename = "nodes.toggleSelection")]
    ToggleSelection(ToggleArgs),
    #[serde(rename = "nodes.linked")]
    Linked(LinkedArgs),
    #[serde(rename = "nodes.inBox")]
    InBox(BoxArgs),
    #[serde(rename = "nodes.shownNames")]
    ShownNames(ShownArgs),
    #[serde(rename = "nodes.groupEdge")]
    GroupEdge(GroupEdgeArgs),
    #[serde(rename = "nodes.splitMixer")]
    SplitMixer(SplitArgs),
}

pub fn dispatch(request: NodesRequest, out: &mut Vec<u8>) -> Result<(), WasmError> {
    use NodesRequest as R;
    match request {
        R::ToggleSelection(a) => write(out, &toggle_selection(&a.current, a.primary.as_deref(), &a.name)),
        R::Linked(a) => write(out, &linked_nodes(&a.nodes, &a.selection, &a.of)),
        R::InBox(a) => write(out, &nodes_in_box(&a.nodes, a.rect)),
        R::ShownNames(a) => write(out, &shown_names(&a.steps)),
        R::GroupEdge(a) => write(out, &group_edge_insets(&a.widgets)),
        R::SplitMixer(a) => write(out, &split_trailing_mixer(&a.row)),
    }
}
