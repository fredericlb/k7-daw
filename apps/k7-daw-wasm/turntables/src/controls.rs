// controls.rs — every control on the Mix desk, by a stable id: what the project's `views/mix.k7x` names to bind one to a module param.
// A reader would get wrong that the ids are ABSOLUTE (`deckA.tempo`, not a relative `deck.tempo`) and derived from `DECKS` (docs/turntables/controls.md).

use crate::vocab::{BPM_RANGE, DECKS, ECHO_RANGE, EQ_DB, FILTER_RANGE, LEVEL_DB, MIX_CENTRE, TRIM_DB};
use serde::{Deserialize, Serialize};
use song_schema::{STEM_NEUTRAL, STEM_RANGE, STEMS, Stem, stem_control};

/// What a control is: a fader moves over a range, a button is 0 or 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum ControlKind {
    Fader,
    Button,
}

/// One control on the desk; `range` is the point of the whole file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct MixControl {
    pub id: String,
    /// What it is called on screen and in a "bind to…" menu; French, like the rest of the desk.
    pub label: String,
    pub kind: ControlKind,
    /// Low and high ends, in the control's own units; a button is 0..1.
    pub range: [f64; 2],
    /// Where it sits with nothing done to it: what a reset returns to.
    pub home: f64,
    /// What kind of module this expects to drive: a hint for a scaffolded surface, never checked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wants: Option<String>,
}

/// What each stem is called on the desk: `other` is everything the three named ones are not.
fn stem_label(stem: Stem) -> &'static str {
    match stem {
        Stem::Drums => "Batterie",
        Stem::Bass => "Basse",
        Stem::Other => "Reste",
        Stem::Vocals => "Voix",
    }
}

fn control(
    id: String,
    label: String,
    kind: ControlKind,
    range: [f64; 2],
    home: f64,
    wants: Option<&str>,
) -> MixControl {
    MixControl {
        id,
        label,
        kind,
        range,
        home,
        wants: wants.map(str::to_owned),
    }
}

fn per_deck(deck: &str, letter: char) -> Vec<MixControl> {
    use ControlKind::{Button, Fader};
    let id = |suffix: &str| format!("{deck}.{suffix}");
    let label = |what: &str| format!("Deck {letter} {what}");
    let mut out = vec![
        // The module's own knob, so a binding cannot offer a range the panel refuses to draw.
        control(id("tempo"), label("tempo"), Fader, BPM_RANGE, 0.0, Some("tempo")),
        control(id("sync"), label("sync"), Button, [0.0, 1.0], 0.0, None),
    ];
    // The channel EQ, in decibels and not 0..1: flat lands mid-travel; one EQ module per deck, three bands on it.
    for (suffix, band) in [("eqLow", "Grave"), ("eqMid", "Mid"), ("eqHigh", "Aigu")] {
        out.push(control(id(suffix), label(band), Fader, EQ_DB, 0.0, Some("eq")));
    }
    // A button, though what it drives is a gain: pre-fade listen is on or off, silent at rest.
    out.push(control(
        id("cue"),
        label("headphones"),
        Button,
        [0.0, 1.0],
        0.0,
        Some("cue"),
    ));
    out.push(control(
        id("filter"),
        label("filter"),
        Fader,
        FILTER_RANGE,
        0.0,
        Some("filter"),
    ));
    // The gain staging, in decibels at both ends: the trim in front of the strip, the fader after it.
    out.push(control(id("trim"), label("trim"), Fader, TRIM_DB, 0.0, Some("trim")));
    out.push(control(
        id("level"),
        label("level"),
        Fader,
        LEVEL_DB,
        0.0,
        Some("level"),
    ));
    // Dry at rest: an effect must not be heard before somebody asks for it.
    out.push(control(id("echo"), label("echo"), Fader, ECHO_RANGE, 0.0, Some("echo")));
    // The stems: one knob per instrument, a proportion, each naming its own node; unity is the centre of the travel.
    for stem in STEMS {
        out.push(control(
            stem_control(deck, stem),
            label(stem_label(stem)),
            Fader,
            STEM_RANGE,
            STEM_NEUTRAL,
            Some("stem"),
        ));
    }
    out
}

/// Every control, derived from the decks so a third deck is not unbindable.
pub fn mix_controls() -> Vec<MixControl> {
    let mut out = vec![control(
        "crossfade".to_owned(),
        "Crossfade".to_owned(),
        ControlKind::Fader,
        [0.0, 1.0],
        MIX_CENTRE,
        Some("crossfade"),
    )];
    for (i, deck) in DECKS.iter().enumerate() {
        out.extend(per_deck(deck, char::from(b'A' + i as u8)));
    }
    out.push(control(
        "transport".to_owned(),
        "Play / stop".to_owned(),
        ControlKind::Button,
        [0.0, 1.0],
        0.0,
        None,
    ));
    out
}
