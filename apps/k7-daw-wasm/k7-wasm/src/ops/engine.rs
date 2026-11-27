// ops/engine.rs — the engine.* and text.* operations: the page's engine rules and the text fingerprint over song-schema.
// A reader would get wrong that none of these is hot: each runs once per write, probe, pick or warning, so JSON is the right ABI.

use crate::error::WasmError;
use crate::ops::write;
use serde::{Deserialize, Serialize};
use song_schema::engine::{
    Candidate, CataloguePlugin, Choice, Compatibility, EngineLatency, MissingItem, PluginNeed, WarningKind,
};

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct HashArgs {
    pub text: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CompatArgs {
    pub needs: Vec<PluginNeed>,
    pub catalogue: Vec<CataloguePlugin>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct LatencyArgs {
    pub network_ms: f64,
    pub buffer_frames: f64,
    pub sample_rate: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct WarningArgs {
    pub message: String,
}

/// What a warning says: its kind, and the missing thing it reports when it reports one.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct WarningAnswer {
    pub kind: WarningKind,
    pub missing: Option<MissingItem>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ChooseArgs {
    pub candidates: Vec<Candidate>,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub remembered: Option<String>,
}

/// The engine `engine.best` takes, as an index into the candidates.
#[derive(Clone, Copy, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct BestAnswer {
    pub index: Option<usize>,
}

pub fn text_hash(a: HashArgs) -> String {
    song_schema::text_hash(&a.text)
}

pub fn compat(a: CompatArgs) -> Compatibility {
    song_schema::engine::compatibility(&a.needs, &a.catalogue)
}

pub fn latency(a: LatencyArgs) -> EngineLatency {
    song_schema::engine::engine_latency(a.network_ms, a.buffer_frames, a.sample_rate)
}

pub fn warning(a: WarningArgs) -> WarningAnswer {
    WarningAnswer {
        kind: song_schema::engine::warning_kind(&a.message),
        missing: song_schema::engine::missing_of(&a.message),
    }
}

pub fn choose(a: ChooseArgs) -> Choice {
    song_schema::engine::choose_engine(&a.candidates, a.remembered.as_deref())
}

pub fn best(a: ChooseArgs) -> BestAnswer {
    BestAnswer {
        index: song_schema::engine::best_engine(&a.candidates, a.remembered.as_deref()),
    }
}

/// The engine and text families' requests: `{"op": "engine.compat", "args": {…}}`.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "op", content = "args")]
pub enum EngineRequest {
    #[serde(rename = "text.hash")]
    TextHash(HashArgs),
    #[serde(rename = "engine.compat")]
    Compat(CompatArgs),
    #[serde(rename = "engine.latency")]
    Latency(LatencyArgs),
    #[serde(rename = "engine.warning")]
    Warning(WarningArgs),
    #[serde(rename = "engine.choose")]
    Choose(ChooseArgs),
    #[serde(rename = "engine.best")]
    Best(ChooseArgs),
}

pub fn dispatch(request: EngineRequest, out: &mut Vec<u8>) -> Result<(), WasmError> {
    match request {
        EngineRequest::TextHash(a) => write(out, &text_hash(a)),
        EngineRequest::Compat(a) => write(out, &compat(a)),
        EngineRequest::Latency(a) => write(out, &latency(a)),
        EngineRequest::Warning(a) => write(out, &warning(a)),
        EngineRequest::Choose(a) => write(out, &choose(a)),
        EngineRequest::Best(a) => write(out, &best(a)),
    }
}
