// ops/text_desk.rs — the text.* readers and writers of the cue sheet, a playlist, the library pins, an assembly and a chain file.
// A reader would get wrong that a reader answers its issues beside an empty value: the page draws what is wrong, never throws.

use super::text::{TextAnswer, TextArgs, answer, failure, json_of};
use crate::error::WasmError;
use crate::ops::k7x_edit::to_value;
use k7x::edit::{Document, apply_sugared};
use serde::{Deserialize, Serialize};
use song::files::{ModuleIssue, finding_issue, validate_file};
use song_edit::error::Error;
use song_edit::library_pins::{Pin, parse_pins};
use song_edit::sugar::sugar_for;
use std::collections::BTreeMap;

// ---- the cue sheet and the desk's bindings -----------------------------------------------------

/// What `mix.k7x` says: each song's cue points by its ref, positional; empty with the issues when the file does not read.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct MixCues {
    pub markers: BTreeMap<String, Vec<Option<f64>>>,
    pub issues: Vec<ModuleIssue>,
}

pub(super) fn mix_cues_read(a: &TextArgs) -> MixCues {
    let verdict = validate_file("mix.k7x", &a.text);
    if !verdict.ok {
        return MixCues {
            markers: BTreeMap::new(),
            issues: verdict.issues,
        };
    }
    let mut markers = BTreeMap::new();
    if let Ok(file) = song::files::desk::decode_mix("mix.k7x", &a.text) {
        for cue in file.cues.into_iter().flatten() {
            if let Some(m) = cue.markers {
                markers.insert(cue.reference, m.into_iter().map(|n| n.0).collect());
            }
        }
    }
    MixCues {
        markers,
        issues: vec![],
    }
}

/// One edit of the cue sheet.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "op", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum MixOp {
    /// One song's cue points; a song left with none goes back to a bare ref.
    WithMarkers {
        #[serde(rename = "ref")]
        reference: String,
        markers: Vec<Option<f64>>,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct MixArgs {
    pub text: String,
    pub ops: Vec<MixOp>,
}

/// What `views/mix.k7x` binds: control id to what it drives, `null` for one not wired yet.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct MixViewRead {
    #[cfg_attr(feature = "ts", ts(type = "Record<string, import(\"./ViewBinding\").ViewBinding | null> | null"))]
    pub value: Option<song::files::desk::MixView>,
    pub issues: Vec<ModuleIssue>,
}

pub(super) fn mix_view_read(a: &TextArgs) -> MixViewRead {
    let verdict = validate_file("views/mix.k7x", &a.text);
    if !verdict.ok {
        return MixViewRead {
            value: None,
            issues: verdict.issues,
        };
    }
    MixViewRead {
        value: song::files::desk::decode_mix_view("views/mix.k7x", &a.text).ok(),
        issues: vec![],
    }
}

// ---- a playlist ----------------------------------------------------------------------------------

/// One edit of a playlist's `items`.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "op", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum PlaylistOp {
    Append {
        song: String,
        #[serde(default)]
        #[cfg_attr(feature = "ts", ts(optional))]
        variant: Option<String>,
    },
    Remove {
        index: f64,
    },
    Move {
        from: f64,
        to: f64,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PlaylistArgs {
    pub text: String,
    pub ops: Vec<PlaylistOp>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct NameArgs {
    pub name: String,
}

// ---- the library pins ----------------------------------------------------------------------------

/// What a library is pinned at: a published number, or `"draft"`.
#[derive(Clone, Debug, PartialEq)]
pub struct PinVersion(pub Pin);

impl Serialize for PinVersion {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match &self.0 {
            Pin::Number(n) => s.serialize_f64(*n),
            Pin::Draft => s.serialize_str("draft"),
        }
    }
}

impl<'de> Deserialize<'de> for PinVersion {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        match serde_json::Value::deserialize(d)? {
            serde_json::Value::Number(n) => Ok(PinVersion(Pin::Number(n.as_f64().unwrap_or(f64::NAN)))),
            serde_json::Value::String(s) if s == "draft" => Ok(PinVersion(Pin::Draft)),
            _ => Err(serde::de::Error::custom("a pin is a version number or \"draft\"")),
        }
    }
}

/// A workspace's pins, library name to version, in the file's order.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(type = "Record<string, number | \"draft\">"))]
pub struct LibraryPins(pub serde_json::Map<String, serde_json::Value>);

impl Serialize for LibraryPins {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(s)
    }
}

impl<'de> Deserialize<'de> for LibraryPins {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        serde_json::Map::deserialize(d).map(LibraryPins)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PinsArgs {
    pub pins: LibraryPins,
}

pub(super) fn pins_of(m: &serde_json::Map<String, serde_json::Value>) -> Vec<(String, Pin)> {
    m.iter()
        .filter_map(|(k, v)| match v {
            serde_json::Value::Number(n) => n.as_f64().map(|n| (k.clone(), Pin::Number(n))),
            serde_json::Value::String(s) if s == "draft" => Some((k.clone(), Pin::Draft)),
            _ => None,
        })
        .collect()
}

pub(super) fn pins_parsed(a: &TextArgs) -> LibraryPins {
    LibraryPins(
        parse_pins(&a.text)
            .into_iter()
            .map(|(k, p)| (k, serde_json::to_value(PinVersion(p)).unwrap_or(serde_json::Value::Null)))
            .collect(),
    )
}

// ---- an assembly template ------------------------------------------------------------------------

/// What `assembly.k7x` promotes: each param's name and the member param it writes.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AssemblyRead {
    #[cfg_attr(feature = "ts", ts(type = "Record<string, { node: string, param: string }> | null"))]
    pub params: Option<serde_json::Map<String, serde_json::Value>>,
    pub issues: Vec<ModuleIssue>,
}

pub(super) fn assembly_read(a: &TextArgs) -> AssemblyRead {
    match song::assemblies::file::decode_assembly_template("assembly.k7x", &a.text) {
        Ok(t) => AssemblyRead {
            params: t.params.map(|p| {
                p.into_iter()
                    .filter_map(|(k, p)| Some((k, serde_json::to_value(p).ok()?)))
                    .collect()
            }),
            issues: vec![],
        },
        Err(f) => AssemblyRead {
            params: None,
            issues: vec![finding_issue(&a.text, &f)],
        },
    }
}

/// One edit of an assembly template's `params`.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "op", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum AssemblyOp {
    SetParam { name: String, node: String, param: String },
    DropParam { name: String },
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AssemblyArgs {
    pub text: String,
    pub ops: Vec<AssemblyOp>,
}

// ---- a chain file ----------------------------------------------------------------------------------

/// One item of a chain file's steps, as a card draws it.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ChainItem {
    pub id: String,
    /// The module the item extends, when it names one.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub extends: Option<String>,
    pub bypass: bool,
}

/// What a `chains/<name>.k7x` holds: the items of its own steps in order, or the first thing wrong.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ChainRead {
    pub items: Vec<ChainItem>,
    pub issues: Vec<ModuleIssue>,
}

pub(super) fn chain_read(a: &TextArgs) -> ChainRead {
    let file = "chains/template.k7x";
    let verdict = validate_file(file, &a.text);
    if !verdict.ok {
        return ChainRead {
            items: vec![],
            issues: verdict.issues,
        };
    }
    let items = song::routing::decode_chain(file, &a.text, song::PathRule::HOST)
        .map(|c| {
            c.steps
                .iter()
                .filter(|s| !s.is_end())
                .flat_map(|s| s.items())
                .map(|item| ChainItem {
                    id: item.id().to_owned(),
                    extends: match item {
                        song::routing::Item::Node(n) => n.module.extends.clone(),
                        _ => None,
                    },
                    bypass: item.bypass(),
                })
                .collect()
        })
        .unwrap_or_default();
    ChainRead { items, issues: vec![] }
}

// ---- the short forms ------------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ExpandArgs {
    /// The workspace path the data was read at: its folder says which short forms it may write.
    pub path: String,
    #[cfg_attr(feature = "ts", ts(type = "unknown"))]
    pub data: serde_json::Value,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ApplyArgs {
    pub path: String,
    pub text: String,
    /// The expanded data the document is to hold once written.
    #[cfg_attr(feature = "ts", ts(type = "unknown"))]
    pub after: serde_json::Value,
}

pub(super) fn sugar_expand(a: &ExpandArgs) -> Result<serde_json::Value, WasmError> {
    let data = to_value(&a.data);
    match sugar_for(&a.path) {
        Some(s) => s
            .try_expand(&data)
            .map(|v| json_of(&v))
            .map_err(|e| WasmError::bad_request(e.to_string())),
        None => Ok(a.data.clone()),
    }
}

pub(super) fn sugar_apply(a: &ApplyArgs) -> TextAnswer {
    let mut doc = Document::new(&a.text);
    let sugar = sugar_for(&a.path);
    let done = apply_sugared(&mut doc, &to_value(&a.after), sugar.as_ref().map(|s| s as &dyn k7x::edit::Sugar));
    match done {
        Ok(()) => answer(&doc, vec![true], None),
        Err(e) => {
            let f = failure(doc.text(), 0, &Error::Edit(e));
            answer(&doc, vec![], Some(f))
        }
    }
}

