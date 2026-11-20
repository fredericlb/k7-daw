// assemblies/file.rs — the two files an assembly is: a library's `assembly.k7x` (plugs and promoted params) and a project's instance.
// A reader would get wrong that the decoders are strict and an empty file is a finding: both say what the file declares (273).
// An assembly is a DRAWING: nothing here reaches the compiler or the engine (docs/assemblies.md).

use crate::files::scalar::NonEmpty;
use crate::files::{Check, DeError, Finding, decode_with, from_map};
use crate::modules::PortType;
use indexmap::IndexMap;
use k7x::PathSeg;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// One PLUG: a port on the box, plus the member whose own port it stands for.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssemblyPlug {
    pub name: NonEmpty,
    #[serde(rename = "type")]
    pub kind: PortType,
    pub node: NonEmpty,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<NonEmpty>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mandatory: Option<bool>,
}

/// One PROMOTED param: a knob on the box's face, and the member param it writes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssemblyParam {
    pub node: NonEmpty,
    pub param: NonEmpty,
}

/// `assembly.k7x`: what the box IS, in the library.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssemblyTemplate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<NonEmpty>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inputs: Option<Vec<AssemblyPlug>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outputs: Option<Vec<AssemblyPlug>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<IndexMap<String, AssemblyParam>>,
}

/// `assemblies/<name>.k7x`: which of THIS project's nodes are that box.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssemblyFile {
    pub uses: NonEmpty,
    pub members: IndexMap<String, String>,
}

fn at(parts: &[&str], index: Option<usize>, tail: &[&str]) -> Vec<PathSeg> {
    let key = |s: &&str| PathSeg::Key((*s).to_owned());
    let mut out: Vec<PathSeg> = parts.iter().map(key).collect();
    out.extend(index.map(PathSeg::Index));
    out.extend(tail.iter().map(key));
    out
}

impl Check for AssemblyTemplate {
    fn check(&self) -> Option<(Vec<PathSeg>, String)> {
        // A plug name is an address, so two of them on one side is a cable with no reading.
        for (side, plugs) in [("inputs", &self.inputs), ("outputs", &self.outputs)] {
            let mut seen: HashSet<&str> = HashSet::new();
            for (i, p) in plugs.iter().flatten().enumerate() {
                if !seen.insert(p.name.0.as_str()) {
                    let message = format!(
                        "two {side} are called '{}' — a plug name is what a cable names",
                        p.name.0.chars().take(40).collect::<String>()
                    );
                    return Some((at(&[side], Some(i), &["name"]), message));
                }
            }
        }
        // A record's key is a name: an empty one is no name.
        if let Some(params) = &self.params
            && params.keys().any(String::is_empty)
        {
            return Some((at(&["params"], None, &[]), "expected a non-empty string".to_owned()));
        }
        None
    }
}

impl Check for AssemblyFile {
    fn check(&self) -> Option<(Vec<PathSeg>, String)> {
        let empty = |s: &String| s.is_empty();
        if self.members.iter().any(|(k, v)| empty(k) || empty(v)) {
            return Some((at(&["members"], None, &[]), "expected a non-empty string".to_owned()));
        }
        None
    }
}

const EMPTY_TEMPLATE: &str =
    "an assembly declares its plugs (`inputs` / `outputs`) and the params it promotes (`params`).";
const EMPTY_INSTANCE: &str =
    "an assembly instance says `uses` (a template ref) and `members` (template name -> node name).";

/// Decodes a file that must hold something, then its refinements; `empty` is what an empty file is told.
fn decode_filled<T: Check + serde::de::DeserializeOwned>(file: &str, text: &str, empty: &str) -> Result<T, Finding> {
    decode_with(file, text, |root| {
        if root.entries.is_empty() {
            return Err(DeError {
                message: empty.to_owned(),
                at: None,
            });
        }
        from_map::<T>(root)
    })
}

/// A project's `assemblies/<name>.k7x`.
pub fn decode_assembly(file: &str, text: &str) -> Result<AssemblyFile, Finding> {
    decode_filled(file, text, EMPTY_INSTANCE)
}

/// A library's `assembly.k7x`.
pub fn decode_assembly_template(file: &str, text: &str) -> Result<AssemblyTemplate, Finding> {
    decode_filled(file, text, EMPTY_TEMPLATE)
}
