// deck.rs — the small rules around the deck document: the crossfade law, what a platter plays at, the faders after a gesture.
// A reader would get wrong `platter_rate`'s direction: target OVER natural, so a 120 song asked for 60 runs at half speed.

use crate::vocab::{DECKS, DeckLoad, DeckTempo, DeckTempos, SCRATCH_MS, ratio_of};
use indexmap::IndexMap;
use serde::Serialize;
use song::reads::{VOLUME_DB_MAX, VOLUME_DB_MIN};
use song_schema::{STEMS, STEMS_DIR, Stem, deck_platter, stem_file, stem_lane, stem_platter};

/// How long to hold this scratch: 0 goes now. A RELEASE (no rate) never waits, since it hands the record back.
pub fn scratch_delay(since_last: f64, rate: Option<f64>, every: Option<f64>) -> f64 {
    if rate.is_none() {
        return 0.0;
    }
    0.0_f64.max(every.unwrap_or(SCRATCH_MS) - since_last)
}

/// A deck's linear gain at crossfade `mix`: equal GAIN, the same law the project's module writes.
pub fn deck_gain(deck: &str, mix: f64) -> f64 {
    let clamped = 1.0_f64.min(0.0_f64.max(mix));
    if deck == "deckA" { 1.0 - clamped } else { clamped }
}

/// A linear gain as the decibels a strip's `Volume` takes: silence is the floor, not minus infinity.
pub fn gain_to_db(gain: f64) -> f64 {
    if gain <= 0.0 {
        return VOLUME_DB_MIN;
    }
    VOLUME_DB_MIN.max(VOLUME_DB_MAX.min(20.0 * gain.log10()))
}

/// The id of the level at the end of a deck's chain: the thing the crossfader moves.
pub fn level_id(deck: &str) -> String {
    format!("{deck}:level")
}

/// One deck's level at a crossfade position, as `set-plugin-param` takes it.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LevelDb {
    pub id: String,
    pub db: f64,
}

/// Every deck's level at crossfade `mix`, in the order the decks are named.
pub fn crossfade_levels(mix: f64) -> Vec<LevelDb> {
    DECKS
        .iter()
        .map(|d| LevelDb {
            id: level_id(d),
            db: gain_to_db(deck_gain(d, mix)),
        })
        .collect()
}

/// `stems/` beside a record: string work, since the path is the engine machine's.
fn stems_folder(master: &str) -> String {
    let cut = master.rfind(['/', '\\']);
    let head = cut.map_or("", |i| &master[..=i]);
    format!("{head}{STEMS_DIR}/")
}

/// One stem's file beside its record.
pub fn stem_source(master: &str, stem: Stem) -> String {
    format!("{}{}", stems_folder(master), stem_file(stem))
}

/// A plugin's engine address: the chain it heads, then the module.
pub fn plugin_id(track: &str, module: &str) -> String {
    format!("{track}:{module}")
}

/// Every platter a deck drives: four when the record is separated, else its own; none when it holds nothing.
pub fn platters_of(deck: &str, load: Option<&DeckLoad>) -> Vec<String> {
    let Some(load) = load else { return Vec::new() };
    if load.stems == Some(true) {
        STEMS
            .iter()
            .map(|s| plugin_id(&stem_lane(deck, *s), &stem_platter(deck, *s)))
            .collect()
    } else {
        vec![plugin_id(deck, &deck_platter(deck))]
    }
}

/// How fast a platter runs as a multiple of the record's own clock; 0 when stopped.
pub fn platter_rate(tempo: Option<&DeckTempo>, playing: bool) -> f64 {
    if !playing {
        return 0.0;
    }
    // Target over natural, and not the other way round: the inverse is the stretch.
    ratio_of(tempo.map_or(0.0, |t| t.target), tempo.map_or(0.0, |t| t.natural))
}

/// What a platter's `Key` should be: 1 holds the pitch, 0 lets it follow the speed; a hand on the record wins.
pub fn platter_key(tempo: Option<&DeckTempo>, holding: bool) -> f64 {
    if !holding && tempo.is_some_and(|t| t.keylock == Some(true)) {
        1.0
    } else {
        0.0
    }
}

/// The faders after one has been moved: the target, and nothing else.
pub fn nudged_tempos(faders: &DeckTempos, deck: &str, bpm: f64) -> DeckTempos {
    let mut out = faders.clone();
    if let Some(t) = out.get_mut(deck) {
        t.target = bpm;
    }
    out
}

/// The keylock switch belongs to the DECK, not the record on it, so it is merged in rather than stored per song.
pub fn with_keylock(tempos: &DeckTempos, on: &IndexMap<String, bool>) -> DeckTempos {
    let mut out = tempos.clone();
    for deck in DECKS {
        if let Some(t) = out.get_mut(deck) {
            t.keylock = Some(on.get(deck).copied().unwrap_or(false));
        }
    }
    out
}

/// What the decks PLAY at: the slave takes the master's tempo.
pub fn synced_tempos(tempos: &DeckTempos, master: Option<&str>) -> DeckTempos {
    let Some(master) = master else { return tempos.clone() };
    let Some(lead) = tempos.get(master).filter(|l| l.target > 0.0) else {
        return tempos.clone();
    };
    let target = lead.target;
    let mut out = tempos.clone();
    for deck in DECKS {
        // A deck with no natural tempo cannot be pulled onto one: there is no ratio to drive its platter with.
        if deck == master {
            continue;
        }
        if let Some(t) = out.get_mut(deck).filter(|t| t.natural > 0.0) {
            t.target = target;
        }
    }
    out
}
