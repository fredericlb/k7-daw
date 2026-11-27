// files/preset_file.rs — a preset file and a module folder's `config.k7x`, decoded for the editor's check.
// A reader would get wrong that a rack pad's sample is only a name here: whether a library holds it is the compile's question.

use super::scalar::{MidiKey, NonEmpty};
use super::{Check, decode};
use crate::Finding;
use indexmap::IndexMap;
use k7x::PathSeg;
use serde::Deserialize;

/// One pad of a rack's kit, as a preset keeps it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PresetTap {
    pub note: MidiKey,
    pub sample: Option<NonEmpty>,
}

/// One preset: the values it sets, the engine they were taken on, the base they sit over, and a rack's kit.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PresetFile {
    pub engine: Option<NonEmpty>,
    pub base: Option<NonEmpty>,
    pub params: Option<IndexMap<String, f64>>,
    pub rack: Option<IndexMap<String, PresetTap>>,
}

fn key(name: &str) -> PathSeg {
    PathSeg::Key(name.to_owned())
}

impl Check for PresetFile {
    fn check(&self) -> Option<(Vec<PathSeg>, String)> {
        let empty =
            |field: &str, has: bool| has.then(|| (vec![key(field), key("")], "expected a non-empty key".to_owned()));
        empty("params", self.params.as_ref().is_some_and(|m| m.contains_key("")))
            .or_else(|| empty("rack", self.rack.as_ref().is_some_and(|m| m.contains_key(""))))
    }
}

pub fn decode_preset(file: &str, text: &str) -> Result<PresetFile, Finding> {
    decode(file, text)
}

/// A module folder's config: the namespace every module in it is addressed by.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleConfig {
    pub namespace: NonEmpty,
}

impl Check for ModuleConfig {}

pub fn decode_config(file: &str, text: &str) -> Result<ModuleConfig, Finding> {
    decode(file, text)
}
