// vocab.rs — what a deck IS and the ends of every knob the desk draws over it: names, what a deck holds, what it plays at.
// A reader would get wrong that these constants are written once: consts.generated.ts prints them from here for the page.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// The two decks, named as the project names them (`graph.k7x`).
pub const DECKS: [&str; 2] = ["deckA", "deckB"];

/// The crossfader's middle: both decks at once, where a set starts.
pub const MIX_CENTRE: f64 = 0.5;

/// The tempo the timeline runs at, fixed: each deck warps its own clip.
pub const DOC_TEMPO: f64 = 120.0;

/// How far a tempo bends either way, ±16 %.
pub const PITCH_RANGE: f64 = 0.16;

/// The module a deck's tempo belongs to, and the params the faders are.
pub const TEMPO_MODULE: &str = "@k7-core/cold/tempo";
pub const TEMPO_PARAM: &str = "Target BPM";
pub const TEMPO_SOURCE_PARAM: &str = "Source BPM";

/// The ends of the tempo fader, the module's own knob.
pub const BPM_RANGE: [f64; 2] = [10.0, 300.0];
/// The ends of an EQ knob, in dB.
pub const EQ_DB: [f64; 2] = [-20.0, 20.0];
/// The ends of a deck's filter knob: signed, the neutral is the middle.
pub const FILTER_RANGE: [f64; 2] = [-100.0, 100.0];
/// The ends of the input trim, in dB.
pub const TRIM_DB: [f64; 2] = [-12.0, 12.0];
/// The ends of the channel fader, in dB; the bottom is silence.
pub const LEVEL_DB: [f64; 2] = [-60.0, 6.0];
/// The echo's amount, dry to fully wet.
pub const ECHO_RANGE: [f64; 2] = [0.0, 1.0];

/// How fast a record runs backwards under the censor: a platter rate.
pub const CENSOR_RATE: f64 = -1.0;

/// The three bands a deck's EQ strip draws, low to high.
pub const EQ_BANDS: [&str; 3] = ["eqLow", "eqMid", "eqHigh"];

/// The engine's platter type, kept for the stand-in document only.
pub const PLATTER_TYPE: &str = "studio_turntable";

/// The track the click lives on.
pub const METRONOME_TRACK: &str = "metronome";

/// The metre everything assumes and nowhere measures.
pub const BEATS_PER_BAR: u32 = 4;

/// How often a scratch may reach the engine, in milliseconds.
pub const SCRATCH_MS: f64 = 16.0;

/// How far a tempo must move for a drag to send again.
pub const LIVE_WARP_BPM: f64 = 0.05;

/// The lengths a loop button offers, in beats.
pub const LOOP_BEATS: [f64; 3] = [4.0, 8.0, 16.0];
/// What halving and doubling may reach.
pub const LOOP_MIN_BEATS: f64 = 1.0 / 8.0;
pub const LOOP_MAX_BEATS: f64 = 32.0;
/// What a beat jump moves by before anybody has chosen a length.
pub const DEFAULT_LOOP_BEATS: f64 = 4.0;

/// What a deck is holding: a file the engine can open, and who to say it is.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DeckLoad {
    /// Absolute, on the engine's machine.
    pub path: String,
    pub title: String,
    #[serde(default)]
    pub artist: Option<String>,
    /// The record's workspace path.
    pub id: String,
    /// Whether `stems/` holds this record's four files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub stems: Option<bool>,
    /// A tempo corrected this session, a fact about the record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub bpm: Option<f64>,
    /// `C# minor`, typed by a person.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub key: Option<String>,
}

/// What one deck is playing at: the song's own tempo, its length and where the fader is.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DeckTempo {
    /// The engine's estimate for this song, or 0 when it found none.
    pub natural: f64,
    /// The song's length in seconds.
    pub seconds: f64,
    /// What it should play at.
    pub target: f64,
    /// Where this song's beat grid starts, in source seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub phase: Option<f64>,
    /// Hold the pitch while the tempo moves?
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub keylock: Option<bool>,
}

/// What a deck is holding, by deck id; a deck with nothing on it is null.
pub type DeckLoads = IndexMap<String, Option<DeckLoad>>;
/// What each deck plays at; a deck with no tempo is absent.
pub type DeckTempos = IndexMap<String, DeckTempo>;

/// The stretch, as a multiplier of the file's own clock: 1 when either end is unknown.
pub fn ratio_of(natural: f64, target: f64) -> f64 {
    if natural > 0.0 && target > 0.0 {
        natural / target
    } else {
        1.0
    }
}
