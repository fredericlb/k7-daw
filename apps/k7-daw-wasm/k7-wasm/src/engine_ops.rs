// engine_ops.rs — the page's engine rules and the text fingerprint as JSON ops over song-schema's pure functions.
// A reader would get wrong that none of these is hot: each runs once per write, probe, pick or warning, so JSON is the right ABI.

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
