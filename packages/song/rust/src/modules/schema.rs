// modules/schema.rs — a module file decoded: `Module`, its `Alternative`, the plugin block, automations and engine sets.
// A reader would get wrong that one macro writes the fields both share: serde's `deny_unknown_fields` does not mix with `flatten`.

use super::Port;
use super::rack::Rack;
use super::scalars::{EngineKey, EngineName, LutPath, ModelUrl, ShaderName, StrudelPath, TapKey, Volume};
use super::ui::Ui;
use super::unions::{AlternativePick, Input, Keyed, Monitor, ParamValue};
use crate::files::scalar::{MidiKey, NonEmpty, Unit};
use crate::files::{Check, Finding, Placement, decode_with, from_map, keys, line_col, locate};
use indexmap::IndexMap;
use k7x::{MapNode, PathSeg};
use serde::de::{Deserializer, MapAccess, Visitor};
use serde::ser::{SerializeMap, Serializer};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleType {
    Plugin,
    Rack,
    VideoEffect,
    VideoTransition,
    Shader,
    Shape,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum IoKind {
    #[serde(rename = "out")]
    Out,
    #[serde(rename = "in")]
    In,
    #[serde(rename = "video_out")]
    VideoOut,
    #[serde(rename = "file")]
    File,
    #[serde(rename = "camera")]
    Camera,
    #[serde(rename = "midi")]
    Midi,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShapeKind {
    Square,
    Circle,
    Text,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Listed,
    Favorite,
    Hidden,
}

/// The plugin block: `type` and `name` are read, every other key is the engine's business and is kept.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plugin {
    pub plugin_type: Option<String>,
    pub name: Option<String>,
    pub extra: Map<String, Value>,
}

impl Serialize for Plugin {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(None)?;
        if let Some(t) = &self.plugin_type {
            map.serialize_entry("type", t)?;
        }
        if let Some(n) = &self.name {
            map.serialize_entry("name", n)?;
        }
        for (k, v) in &self.extra {
            map.serialize_entry(k, v)?;
        }
        map.end()
    }
}

struct PluginVisitor;

impl<'de> Visitor<'de> for PluginVisitor {
    type Value = Plugin;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("an object")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Plugin, A::Error> {
        let mut out = Plugin::default();
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "type" => out.plugin_type = Some(map.next_value::<String>()?),
                "name" => out.name = Some(map.next_value::<String>()?),
                _ => {
                    out.extra.insert(key, map.next_value::<Value>()?);
                }
            }
        }
        Ok(out)
    }
}

impl<'de> Deserialize<'de> for Plugin {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(PluginVisitor)
    }
}

/// A curve driving a named param; `trigger` names the module whose note-ons restart it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Automation {
    pub param: NonEmpty,
    pub curve: NonEmpty,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger: Option<NonEmpty>,
}

/// The values set for ONE engine.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineSet {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<IndexMap<String, ParamValue>>,
}

/// Writes the fields a module says about ITSELF into a struct, after the ones that struct owns.
macro_rules! module_struct {
    ($(#[$meta:meta])* $name:ident { $($own:tt)* }) => {
        $(#[$meta])*
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        #[serde(deny_unknown_fields)]
        pub struct $name {
            $($own)*
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub plugin: Option<Plugin>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub io: Option<IoKind>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub inputs: Option<Vec<Port>>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub outputs: Option<Vec<Port>>,
            #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
            pub module_type: Option<ModuleType>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub shader: Option<ShaderName>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub shape: Option<ShapeKind>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub font: Option<String>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub align: Option<TextAlign>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub background: Option<String>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub shadow: Option<bool>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub params: Option<IndexMap<String, ParamValue>>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub rack: Option<Rack>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub tap_outs: Option<Keyed<TapKey, NonEmpty>>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub kit: Option<Keyed<TapKey, MidiKey>>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub cold: Option<bool>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub model_url: Option<ModelUrl>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub channel: Option<Input>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub camera: Option<String>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub device: Option<String>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub frame: Option<[f64; 4]>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub source: Option<String>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub monitor: Option<Monitor>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub gain: Option<f64>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub mix: Option<Unit>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub input_volume: Option<Volume>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub output_volume: Option<Volume>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub base: Option<String>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub ui: Option<Ui>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub automations: Option<Vec<Automation>>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub content: Option<String>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub events: Option<Vec<Placement>>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub color: Option<String>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub disabled: Option<bool>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub tracks: Option<Vec<String>>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub strudel: Option<StrudelPath>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub video_source: Option<NonEmpty>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub lut: Option<LutPath>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub frozen: Option<NonEmpty>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub engine: Option<EngineName>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub engines: Option<Keyed<EngineKey, EngineSet>>,
        }
    };
}

module_struct! {
    /// One module file, as authored; a library module, a node and an engine are all this.
    Module {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub extends: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub alternatives: Option<Vec<Alternative>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub alternative: Option<AlternativePick>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub status: Option<Status>,
        /// Where `plugin` was written, for the one rule that is raised after the file reads.
        #[serde(skip)]
        pub plugin_at: Option<(u32, u32)>,
    }
}

module_struct! {
    /// One alternative: the same flat struct a module is, minus the alternatives.
    Alternative {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<NonEmpty>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub extends: Option<NonEmpty>,
    }
}

fn index(path: &mut Vec<PathSeg>, key: &str, i: usize) {
    path.push(PathSeg::Key(key.to_owned()));
    path.push(PathSeg::Index(i));
}

/// The first refinement a set of fields breaks: its ports' names, its panel's, then its events'.
fn field_check(
    base: &[PathSeg],
    ports: [(&str, Option<&Vec<Port>>); 2],
    rack: Option<&Rack>,
    ui: Option<&Ui>,
    events: Option<&Vec<Placement>>,
) -> Option<(Vec<PathSeg>, String)> {
    // Only a slice rack names a source: on a samples rack the key is no field of it.
    if let Some(r) = rack
        && r.rack_type == super::rack::RackType::Samples
        && r.source.is_some()
    {
        let mut at = base.to_vec();
        at.extend([PathSeg::Key("rack".to_owned()), PathSeg::Key("source".to_owned())]);
        return Some((
            at,
            "not a rack field: source — only a slice rack names its source".to_owned(),
        ));
    }
    for (key, list) in ports {
        for (i, port) in list.into_iter().flatten().enumerate() {
            if port.name.is_empty() {
                let mut at = base.to_vec();
                index(&mut at, key, i);
                at.push(PathSeg::Key("name".to_owned()));
                return Some((at, "expected a non-empty string".to_owned()));
            }
        }
    }
    if let Some(ui) = ui {
        let mut at = base.to_vec();
        at.push(PathSeg::Key("ui".to_owned()));
        if let Some(found) = ui.check(&at) {
            return Some(found);
        }
    }
    for (j, event) in events.into_iter().flatten().enumerate() {
        if let Some((rest, message)) = event.check() {
            let mut at = base.to_vec();
            index(&mut at, "events", j);
            at.extend(rest);
            return Some((at, message));
        }
    }
    None
}

impl Check for Module {
    fn check(&self) -> Option<(Vec<PathSeg>, String)> {
        let ports = [("inputs", self.inputs.as_ref()), ("outputs", self.outputs.as_ref())];
        if let Some(found) = field_check(&[], ports, self.rack.as_ref(), self.ui.as_ref(), self.events.as_ref()) {
            return Some(found);
        }
        for (i, alt) in self.alternatives.iter().flatten().enumerate() {
            let mut base = Vec::new();
            index(&mut base, "alternatives", i);
            let ports = [("inputs", alt.inputs.as_ref()), ("outputs", alt.outputs.as_ref())];
            if let Some(found) = field_check(&base, ports, alt.rack.as_ref(), alt.ui.as_ref(), alt.events.as_ref()) {
                return Some(found);
            }
        }
        None
    }
}

impl Module {
    /// What the read adds once the fields are in: the default type, and where the plugin block sits.
    fn finish(&mut self, root: &MapNode, text: &str) {
        if self.module_type.is_none() && self.io.is_none() && self.extends.is_none() {
            self.module_type = Some(ModuleType::Plugin);
        }
        if self.plugin.is_some() {
            self.plugin_at = locate(root, &keys(&["plugin"])).map(|o| line_col(text, o));
        }
    }
}

/// One module file's text decoded; an empty file is an empty module, and `type: plugin` is the default.
pub fn decode_module(file: &str, text: &str) -> Result<Module, Finding> {
    decode_with(file, text, |root| {
        from_map::<Module>(root).map(|mut m| {
            m.finish(root, text);
            m
        })
    })
}
