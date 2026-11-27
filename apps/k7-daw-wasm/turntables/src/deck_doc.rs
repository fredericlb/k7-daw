// deck_doc.rs — the two decks as ONE document the engine plays: a track each, a level each, the crossfade, the click.
// A reader would get wrong `on_platter`: it REMOVES `Seek`, because the engine reads a changed seek as a needle-drop (docs/turntables/mix.md).

use crate::deck::{deck_gain, gain_to_db, level_id, platter_key, platter_rate, plugin_id, stem_source};
use crate::metronome::metronome_track;
#[cfg(feature = "ts")]
use crate::vocab::DeckTempo;
use crate::vocab::{DECKS, DOC_TEMPO, DeckLoad, DeckLoads, DeckTempos, MIX_CENTRE, PLATTER_TYPE};
use indexmap::IndexMap;
use serde::Deserialize;
use song::doc::{PluginDoc, ProjectDoc, TrackDoc};
use song::modules::Plugin;
use song::reads::blanks;
use song_schema::stem_lane_of;
#[cfg(feature = "ts")]
use std::collections::HashMap;

/// The `plugin:` block of a module file, as far as the engine's name for it is concerned.
#[derive(Clone, Debug, Default, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct ModulePlugin {
    #[serde(default, rename = "type")]
    #[cfg_attr(feature = "ts", ts(rename = "type"))]
    pub kind: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
}

/// A module the deck runs: its node name, the plugin behind it and whether it is on.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct DeckModuleDoc {
    pub name: String,
    #[serde(default)]
    pub plugin: Option<ModulePlugin>,
    pub enabled: bool,
}

/// Which decks are turning: a platter carries its own transport, so this is per deck.
pub type DeckPlaying = IndexMap<String, bool>;

/// Everything a deck document is built from.
#[derive(Clone, Debug, Default, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct DeckSet {
    #[cfg_attr(feature = "ts", ts(as = "HashMap<String, Option<DeckLoad>>"))]
    pub loads: DeckLoads,
    #[serde(default)]
    pub mix: Option<f64>,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(as = "HashMap<String, Vec<DeckModuleDoc>>"))]
    pub modules: IndexMap<String, Vec<DeckModuleDoc>>,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(as = "HashMap<String, DeckTempo>"))]
    pub tempos: DeckTempos,
    /// Which deck's beat the click follows, or null for no click.
    #[serde(default)]
    pub click: Option<String>,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(as = "HashMap<String, bool>"))]
    pub playing: DeckPlaying,
}

fn is_playing(playing: &DeckPlaying, deck: &str) -> bool {
    playing.get(deck).copied().unwrap_or(false)
}

/// Put a record on a DECLARED platter: the file, and the two numbers a re-send must not lose.
fn on_platter(plugin: &PluginDoc, source: String, set: &DeckSet, deck: &str) -> PluginDoc {
    let mut p = plugin.clone();
    p.deck_file = Some(source);
    // Dropped rather than overwritten: a param the document omits is one the engine does not touch.
    p.params.shift_remove("Seek");
    let tempo = set.tempos.get(deck);
    p.params
        .insert("Rate".to_owned(), platter_rate(tempo, is_playing(&set.playing, deck)));
    // No hand here: a document is what the deck does when nobody is touching it.
    p.params.insert("Key".to_owned(), platter_key(tempo, false));
    p
}

/// The COMPILED project with the decks' songs filled in: everything else is left alone.
pub fn with_decks(compiled: ProjectDoc, set: &DeckSet) -> ProjectDoc {
    let click = set.click.as_deref();
    let mut tracks = compiled.tracks.clone();
    tracks.push(metronome_track(click.and_then(|c| set.tempos.get(c)), click.is_some()));
    let tracks = tracks
        .into_iter()
        .map(|mut track| {
            // A deck's lane and its stem lanes are filled from the same load, and placed by one computation.
            let stem = stem_lane_of(&track.id, &DECKS);
            let deck = stem.map_or(track.id.as_str(), |(d, _)| d).to_owned();
            let Some(load) = set.loads.get(&deck).and_then(Option::as_ref) else {
                return track;
            };
            let separated = load.stems == Some(true);
            // An unseparated song leaves its stem lanes empty, and plays off the deck's own lane instead.
            if stem.is_some() && !separated {
                return track;
            }
            track.clips = Vec::new();
            // A separated one is the other way round: the four PARTS play, so the record itself does not.
            if stem.is_none() && separated {
                return track;
            }
            // Whatever plays, plays off a platter the PROJECT declares; all this writes is the record and two live numbers.
            track.plugins = track
                .plugins
                .iter()
                .map(|p| {
                    if p.plugin_type != PLATTER_TYPE {
                        return p.clone();
                    }
                    let source = match stem {
                        Some((_, s)) => stem_source(&load.path, s),
                        None => load.path.clone(),
                    };
                    on_platter(p, source, set, &deck)
                })
                .collect();
            track
        })
        .collect();
    ProjectDoc { tracks, ..compiled }
}

/// The stand-in's platter, BUILT rather than patched: it declares no nodes, so there is none to patch.
fn stand_in_platter(track: &str, load: &DeckLoad, set: &DeckSet) -> PluginDoc {
    let tempo = set.tempos.get(track);
    let mut p = PluginDoc::new(&format!("{track}:platter"));
    p.plugin_type = PLATTER_TYPE.to_owned();
    p.name = "platter".to_owned();
    p.role = "instrument".to_owned();
    p.deck_file = Some(load.path.clone());
    p.params
        .insert("Rate".to_owned(), platter_rate(tempo, is_playing(&set.playing, track)));
    p.params.insert("Gain".to_owned(), 1.0);
    p.params.insert("Key".to_owned(), platter_key(tempo, false));
    p
}

/// A document built from NOTHING, for the seconds before the compile lands: an imitation.
pub fn deck_doc(set: &DeckSet) -> ProjectDoc {
    let mix = set.mix.unwrap_or(MIX_CENTRE);
    let click = set.click.as_deref();
    let mut tracks: Vec<TrackDoc> = DECKS
        .iter()
        .map(|deck| {
            let load = set.loads.get(*deck).and_then(Option::as_ref);
            let mut track = TrackDoc::new(deck);
            track.name = load.map_or_else(|| (*deck).to_owned(), |l| l.title.clone());
            // The platter first when there is one: it is the source; then the effects, the level last.
            if let Some(load) = load {
                track.plugins.push(stand_in_platter(deck, load, set));
            }
            for m in set.modules.get(*deck).into_iter().flatten() {
                let plugin = Plugin {
                    plugin_type: m.plugin.as_ref().and_then(|p| p.kind.clone()),
                    name: m.plugin.as_ref().and_then(|p| p.name.clone()),
                    ..Default::default()
                };
                let (kind, name) = song::console::engine_plugin(Some(&plugin), &m.name, None);
                let mut p = PluginDoc::new(&plugin_id(deck, &m.name));
                p.plugin_type = kind;
                p.name = name;
                p.enabled = m.enabled;
                track.plugins.push(p);
            }
            let mut level = PluginDoc::new(&level_id(deck));
            level.plugin_type = song::resolve::MIXER_PLUGIN.to_owned();
            level.name = format!("{deck} level");
            level
                .params
                .insert("Volume".to_owned(), gain_to_db(deck_gain(deck, mix)));
            track.plugins.push(level);
            track
        })
        .collect();
    // Appended, like `with_decks` does it: the decks keep their places in the track list.
    tracks.push(metronome_track(click.and_then(|c| set.tempos.get(c)), click.is_some()));
    let mut doc = blanks().doc;
    doc.name = "m9".to_owned();
    doc.tempo = DOC_TEMPO;
    doc.tracks = tracks;
    doc
}

/// What to send: the compiled project with the decks filled in when there is one, the stand-in when there is not.
pub fn document(compiled: Option<ProjectDoc>, set: &DeckSet) -> ProjectDoc {
    match compiled {
        Some(c) => with_decks(c, set),
        None => deck_doc(set),
    }
}
