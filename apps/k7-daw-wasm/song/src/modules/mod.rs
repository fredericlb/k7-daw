// modules/mod.rs — module files and the module tree: what a module is, and the vocabulary every root's tree makes.
// A reader would get wrong that a LoadedModule is the AUTHORED tree plus the loader's fields; nothing is resolved here (2c).

pub mod assets;
mod builtin;
mod chain_nodes;
pub mod decorate;
pub mod files;
pub mod keys;
mod merge;
pub mod names;
mod nodes;
pub mod panels;
pub mod preset;
mod rack;
mod scalars;
mod scan;
mod schema;
mod tree;
mod type_table;
pub mod types;
mod ui;
mod unions;
mod vocab;
pub mod weight;

pub use builtin::{
    BuiltinParam, BuiltinPlugin, builtin_param_automatable, builtin_plugin, builtin_plugin_for_ref, builtin_plugins,
};
pub use chain_nodes::{ChainNodes, VSTPRESETS_DIR, load_chain_nodes};
pub use merge::{merge_authored, merge_loaded};
pub use nodes::{NodesRead, load_nodes, plugin_name_findings};
pub use rack::{Rack, RackSound, RackType};
pub use scalars::EngineName;
pub use scan::{SCAN_CAP, Scan, ScannedPlugin, builtin_scan, engine_module_of, engine_modules, io_modules};
pub use schema::{Alternative, Automation, EngineSet, IoKind, Module, ModuleType, Plugin, decode_module};
pub use tree::{ModuleFolder, TreeRead, read_tree, read_tree_in};
pub use types::{MODULE_TYPES, TypeMeta};
pub use ui::{Curve, Layout, Tab, TabName, Ui, Widget, WidgetOption, WidgetType};
pub use unions::{AlternativePick, Input, Monitor, ParamValue};
pub use vocab::{
    Absent, MAX_ENTRIES, Vocabulary, base_module_refs, build_vocabulary, build_vocabulary_with, builtin_modules,
};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// What a module is at the coarsest level; the wire treats each differently.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum ModuleKind {
    Io,
    Plugin,
    Doc,
    Track,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum PortType {
    Audio,
    Midi,
    Video,
}

/// One named port a module declares for itself or takes from its type folder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct Port {
    pub name: String,
    #[serde(rename = "type")]
    #[cfg_attr(feature = "ts", ts(rename = "type"))]
    pub kind: PortType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub mandatory: Option<bool>,
}

/// A module as the tree hands it on: the authored fields (`inputs` and `outputs` resolved) plus the loader's.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct LoadedModule {
    #[serde(flatten)]
    pub authored: Module,
    /// From the type folder; a node that extends another has none, and takes its parent's.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<ModuleKind>,
    /// The type folder, or `io` / `docs` / `misc` for a built-in; a node has none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// The folder's name for a module, the file's base for a node.
    pub file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Where a node lives: `nodes`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub home: Option<String>,
    /// The chain file a chain project's node was written in, and the `id` it carries there.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chain: Option<String>,
    #[serde(rename = "nodeId", skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    /// Where the engine writes the plugin's state, relative to the project.
    #[serde(rename = "presetRel", skip_serializing_if = "Option::is_none")]
    pub preset_rel: Option<String>,
    #[serde(rename = "engineRefs", skip_serializing_if = "Option::is_none")]
    pub engine_refs: Option<IndexMap<String, String>>,
    #[serde(rename = "engineOf", skip_serializing_if = "Option::is_none")]
    pub engine_of: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generated: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub picture: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jsdsp: Option<String>,
    #[serde(rename = "folderAbs", skip_serializing_if = "Option::is_none")]
    pub folder_abs: Option<String>,
    /// The state file it loads: its chosen base, a node's own `.vstpreset`, or the module's.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
    /// The files its `model`, `onnx` and `ir` params name, resolved where it declared them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub onnx: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ir: Option<String>,
    /// The names of those params as authored, when it names any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assets: Option<IndexMap<String, String>>,
    /// The MIDI map stamped from the `controller.k7x` beside its folder.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cc: Option<IndexMap<String, panels::CcBinding>>,
}

impl LoadedModule {
    /// A built-in or catalogue module: no authored fields beyond those given.
    pub fn bare(kind: ModuleKind, category: &str, file: &str) -> LoadedModule {
        LoadedModule {
            authored: Module::default(),
            kind: Some(kind),
            category: Some(category.to_owned()),
            file: file.to_owned(),
            node: None,
            name: None,
            home: None,
            chain: None,
            node_id: None,
            preset_rel: None,
            engine_refs: None,
            engine_of: None,
            generated: None,
            picture: None,
            jsdsp: None,
            folder_abs: None,
            preset: None,
            model: None,
            onnx: None,
            ir: None,
            assets: None,
            cc: None,
        }
    }

    /// The one spelling of a node's file: `nodes/<file>.k7x`, `chains/<chain>.k7x` or `tracks/<file>.k7x`.
    pub fn own_label(&self) -> String {
        match (self.home.as_deref(), self.chain.as_deref()) {
            (Some("chains"), Some(chain)) => format!("chains/{chain}.k7x"),
            (Some("tracks"), _) => format!("tracks/{}.k7x", self.file),
            _ => format!("nodes/{}.k7x", self.file),
        }
    }

    pub fn inputs(&self) -> &[Port] {
        self.authored.inputs.as_deref().unwrap_or_default()
    }

    pub fn outputs(&self) -> &[Port] {
        self.authored.outputs.as_deref().unwrap_or_default()
    }
}
