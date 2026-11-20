// routing/types.rs — the routing's values: the segment graph, the derived chains, a run's steps and outs.
// A reader would get wrong that `Segments` is the ONE routing value: a graph file decodes into it, a chain project builds it.

use indexmap::IndexMap;
use serde::Serialize;

/// Where a node sits on the graph, in world pixels.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

/// A run fed by one tap of a rack node rather than by a link.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TapFeed {
    pub node: String,
    pub tap: String,
    pub to: String,
}

/// `graph.k7x` as its keys spell it; every field is absent when the file had none.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Segments {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modules: Option<IndexMap<String, Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bypassed: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bus_only: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tap_feeds: Option<Vec<TapFeed>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub positions: Option<IndexMap<String, Position>>,
}

impl Segments {
    /// The names `bypassed` lists; none when the file has no such list.
    pub fn bypassed_of(&self) -> impl Iterator<Item = &str> {
        self.bypassed.iter().flatten().map(String::as_str)
    }

    /// The names `bus_only` lists.
    pub fn bus_only_of(&self) -> impl Iterator<Item = &str> {
        self.bus_only.iter().flatten().map(String::as_str)
    }

    /// The tap feeds the file states.
    pub fn tap_feeds_of(&self) -> &[TapFeed] {
        self.tap_feeds.as_deref().unwrap_or_default()
    }
}

/// One module a run passes through: the sources wired to its named ports, and whether it is bypassed.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct UseStep {
    pub module: String,
    #[serde(rename = "in", skip_serializing_if = "Option::is_none")]
    pub inputs: Option<Vec<String>>,
    /// Only ever `false`: a bypassed module.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

/// Where a run ends: the destination, its gain and the output it left from.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Out {
    pub to: String,
    pub gain: f64,
    #[serde(rename = "fromPort", skip_serializing_if = "Option::is_none")]
    pub from_port: Option<String>,
}

/// A maximal linear run of the segment graph.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Chain {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(rename = "fromPort", skip_serializing_if = "Option::is_none")]
    pub from_port: Option<String>,
    #[serde(rename = "use")]
    pub use_steps: Vec<UseStep>,
    pub out: Vec<Out>,
}
