// metronome.rs — the desk's CLICK, as a track in the deck document: a clip warped onto whichever deck leads.
// A reader would get wrong that it is not the engine's metronome: that one clicks on the DOCUMENT's grid, the music's beat is elsewhere.

use crate::vocab::{BEATS_PER_BAR, DECKS, DOC_TEMPO, DeckLoads, DeckTempo, DeckTempos, METRONOME_TRACK, ratio_of};
use song::clips::{ClipDoc, ClipKind, NoteDoc};
use song::doc::{PluginDoc, TrackDoc};
use song::reads::blanks;

/// How loud the click sits under the music, in dB on its own strip.
const METRONOME_DB: f64 = -6.0;

/// Which deck's beat the click follows, or None: a click needs ONE beat, and a measured one.
pub fn metronome_deck(loads: &DeckLoads, tempos: &DeckTempos, master: Option<&str>) -> Option<String> {
    let held = |d: &str| loads.get(d).is_some_and(Option::is_some);
    let on: Vec<&str> = DECKS.iter().copied().filter(|d| held(d)).collect();
    let deck = if on.len() == 1 {
        Some(on[0])
    } else {
        master.filter(|m| held(m))
    }?;
    let t = tempos.get(deck)?;
    (t.natural > 0.0 && t.target > 0.0 && t.phase.is_some()).then(|| deck.to_owned())
}

/// The click as a TRACK, never removed from the document; its bar is there only when `on` and a tempo are.
pub fn metronome_track(tempo: Option<&DeckTempo>, on: bool) -> TrackDoc {
    let mut track = TrackDoc::new(METRONOME_TRACK);
    track.name = "metronome".to_owned();
    track.clips = tempo.filter(|_| on).map(click_bar).into_iter().collect();
    let mut synth = PluginDoc::new(&format!("{METRONOME_TRACK}:synth"));
    synth.plugin_type = "4osc".to_owned();
    synth.name = "click".to_owned();
    // A click is an envelope, not a waveform: instant on, gone in thirty milliseconds.
    for (name, value) in [
        ("Amp Attack", 0.001),
        ("Amp Decay", 0.03),
        ("Amp Sustain", 0.0),
        ("Amp Release", 0.03),
        ("Filter Amount", 0.0),
    ] {
        synth.params.insert(name.to_owned(), value);
    }
    synth.role = "instrument".to_owned();
    let mut level = PluginDoc::new(&format!("{METRONOME_TRACK}:level"));
    level.plugin_type = song::resolve::MIXER_PLUGIN.to_owned();
    level.name = "metronome level".to_owned();
    level.params.insert("Volume".to_owned(), METRONOME_DB);
    track.plugins = vec![synth, level];
    track
}

/// One looping bar of clicks at the deck's spacing, from the transport's zero: no phase.
fn click_bar(tempo: &DeckTempo) -> ClipDoc {
    let to_beats = |seconds: f64| (seconds * DOC_TEMPO) / 60.0;
    // Document beats are a unit of TIME here: one beat of the deck lasts 60/target seconds.
    let beat = DOC_TEMPO / tempo.target;
    // As long as the record PLAYS for, so the click stops rather than outliving it.
    let span = beat.max(to_beats(tempo.seconds * ratio_of(tempo.natural, tempo.target)));
    let mut clip = blanks().clip;
    clip.id = format!("{METRONOME_TRACK}-clip");
    clip.kind = ClipKind::Midi;
    clip.name = "click".to_owned();
    clip.start = 0.0;
    clip.length = span;
    // Four notes and a loop rather than one per beat: the clip is rebuilt on every step of a drag.
    clip.loop_start = 0.0;
    clip.loop_end = f64::from(BEATS_PER_BAR) * beat;
    clip.notes = (0..BEATS_PER_BAR)
        .map(|i| NoteDoc {
            time: f64::from(i) * beat,
            duration: beat / 4.0,
            // The bar's first beat sounds different: that IS the feature.
            key: if i == 0 { 96 } else { 89 },
            vel: if i == 0 { 1.0 } else { 0.7 },
            channel: 1,
            rel: 0.0,
            repeat: None,
            skip: None,
        })
        .collect();
    clip
}
