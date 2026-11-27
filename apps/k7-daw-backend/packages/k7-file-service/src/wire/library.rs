// wire/library.rs — the origin's libraries, the frozen and cold chains, and the preferences a machine answers with.
// A reader would get wrong that these structs are not what the routes serialise: the routes write the JSON text, and `tests/all/wire.rs` holds it to these.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// One published version of a library: the lowest level that runs it, and what changed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct LibraryVersionInfo {
    pub version: u32,
    pub requires: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub notes: Option<String>,
}

/// One of the origin's libraries: the name its refs use, and whether the server holds its folder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct AccountLibrary {
    pub name: String,
    pub present: bool,
    /// Its published versions, oldest first.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub versions: Option<Vec<LibraryVersionInfo>>,
    /// Whether it has the draft its admins edit.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub draft: Option<bool>,
}

/// What giving a library its folder did.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum LibraryState {
    Seeded,
    Created,
    Current,
    Failed,
}

/// What giving a library its folder did, said as the hub would say it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct LibraryNote {
    pub name: String,
    pub state: LibraryState,
    pub note: String,
}

/// `GET` and `PUT /account/libraries`: the list, and what the call did to each.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct AccountLibrariesReply {
    pub libraries: Vec<AccountLibrary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub notes: Option<Vec<LibraryNote>>,
}

/// A model this machine can download, and where from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct ColdDownload {
    pub file: String,
    pub url: String,
}

/// One library chain whose steps are all offline.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct ColdChain {
    /// `@<library>/<path>`.
    #[serde(rename = "ref")]
    #[cfg_attr(feature = "ts", ts(rename = "ref"))]
    pub ref_: String,
    /// One of its offline steps writes an instrumental.
    pub instrumental: bool,
    /// Why THIS machine cannot run it yet (a model not downloaded), null when it can.
    pub unavailable: Option<String>,
    /// Models it needs that this machine can download.
    pub downloads: Vec<ColdDownload>,
}

/// The machine's two separating choices and the libraries' cold chains to make them from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ColdChainsReply {
    pub chains: Vec<ColdChain>,
    /// Null is "never told": this machine separates nothing.
    pub stems_chain: Option<String>,
    pub instrumental_chain: Option<String>,
}

/// One step of a frozen chain: a module, the engine it runs as when it has several, and that engine's values.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct FrozenChainStep {
    /// The step's id in the file, kept across saves so an unchanged step keeps its comments.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub id: Option<String>,
    /// The module it extends, `@<library>/<path>`.
    pub module: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub engine: Option<String>,
    #[cfg_attr(feature = "ts", ts(type = "Record<string, number | string>"))]
    pub params: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub bypass: Option<bool>,
}

/// The interface input the bench's IN is fed from when it opens on the chain.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct ChainInput {
    /// Channels, or a preferences label.
    #[cfg_attr(feature = "ts", ts(type = "string | number[]"))]
    pub channel: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(
        feature = "ts",
        ts(optional, type = "boolean | \"off\" | \"auto\" | \"on\"")
    )]
    pub monitor: Option<Value>,
}

/// A chain the home page's Frozen chains lists: the account's own (`@account/…`), or a library's cold one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FrozenChain {
    #[serde(rename = "ref")]
    #[cfg_attr(feature = "ts", ts(rename = "ref"))]
    pub ref_: String,
    /// Its path under `chains/`, what a person calls it.
    pub name: String,
    /// In the account's library, so this account edits it; a library's is read-only here.
    pub mine: bool,
    /// Every step is offline: it separates a file rather than treating a signal.
    pub cold: bool,
    pub steps: Vec<FrozenChainStep>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub input: Option<ChainInput>,
    /// Why it cannot be recorded through live, null when it can.
    pub live_refusal: Option<String>,
    /// Why the file does not read, when it does not; its steps are then empty.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub unreadable: Option<String>,
}

/// One chain of the library, and whether the bench can PLAY it live.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct LiveChain {
    /// Its path under `chains/`, what the bench's `chain` names.
    #[serde(rename = "ref")]
    #[cfg_attr(feature = "ts", ts(rename = "ref"))]
    pub ref_: String,
    /// Why it cannot run on a live input (a cold step, a clashing node name), null when it can.
    pub unavailable: Option<String>,
}

/// The interface's channel labels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct PreferenceChannels {
    #[serde(rename = "in")]
    #[cfg_attr(feature = "ts", ts(rename = "in"))]
    pub in_: String,
    pub out: String,
}

/// User preferences, formatted for display; `vim` is the only one the web writes back.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WebPreferences {
    pub audio_device: Option<String>,
    pub channels: PreferenceChannels,
    /// The editor's vim mode.
    pub vim: bool,
    /// The chain that separates a record into stems, null when this machine separates nothing.
    pub stems_chain: Option<String>,
    pub instrumental_chain: Option<String>,
}
