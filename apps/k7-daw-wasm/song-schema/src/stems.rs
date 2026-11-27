// stems.rs — the four sources a separation produces, and the names a deck gives their lanes, platters and controls.
// A reader would get wrong that the ORDER of STEMS is knowledge: the model's output tensor is `[1, 4, 2, N]` and nothing says which is which.

use serde::{Deserialize, Serialize};

/// A source a separation produces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum Stem {
    Drums,
    Bass,
    Other,
    Vocals,
}

impl Stem {
    /// The word a file spells it with.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Stem::Drums => "drums",
            Stem::Bass => "bass",
            Stem::Other => "other",
            Stem::Vocals => "vocals",
        }
    }

    fn cap(self) -> &'static str {
        match self {
            Stem::Drums => "Drums",
            Stem::Bass => "Bass",
            Stem::Other => "Other",
            Stem::Vocals => "Vocals",
        }
    }
}

/// The four sources, IN THE ORDER the model returns them.
pub const STEMS: [Stem; 4] = [Stem::Drums, Stem::Bass, Stem::Other, Stem::Vocals];

/// Where a separation's outputs go: `stems/` beside the file.
pub const STEMS_DIR: &str = "stems";

/// A stem knob's travel: 0 is the part gone, 1 is as recorded, 2 is twice as much.
pub const STEM_RANGE: [f64; 2] = [0.0, 2.0];

/// The middle of that travel, and the record as it was cut.
pub const STEM_NEUTRAL: f64 = 1.0;

/// One stem's file name, as the pass writes it: a WAV.
#[must_use]
pub fn stem_file(stem: Stem) -> String {
    format!("{}.wav", stem.word())
}

/// A deck's letter, from `deckA`.
fn letter_of(deck: &str) -> &str {
    deck.strip_prefix("deck").unwrap_or(deck)
}

/// The lane one stem plays on: `stemBassA`.
#[must_use]
pub fn stem_lane(deck: &str, stem: Stem) -> String {
    format!("stem{}{}", stem.cap(), letter_of(deck))
}

/// The platter that reads one stem's file: `platterBassA`.
#[must_use]
pub fn stem_platter(deck: &str, stem: Stem) -> String {
    format!("platter{}{}", stem.cap(), letter_of(deck))
}

/// The platter a deck's own record plays on: `platterA`.
#[must_use]
pub fn deck_platter(deck: &str) -> String {
    format!("platter{}", letter_of(deck))
}

/// The desk control that drives a stem's node: `deckA.stemBass`.
#[must_use]
pub fn stem_control(deck: &str, stem: Stem) -> String {
    format!("{deck}.stem{}", stem.cap())
}

/// Which deck and stem a lane id names, the reverse of `stem_lane`.
#[must_use]
pub fn stem_lane_of<'a>(id: &str, decks: &[&'a str]) -> Option<(&'a str, Stem)> {
    decks
        .iter()
        .flat_map(|d| STEMS.iter().map(move |s| (*d, *s)))
        .find(|(d, s)| stem_lane(d, *s) == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_deck_names_its_lanes_platters_and_controls() {
        assert_eq!(stem_lane("deckA", Stem::Bass), "stemBassA");
        assert_eq!(stem_platter("deckB", Stem::Vocals), "platterVocalsB");
        assert_eq!(deck_platter("deckA"), "platterA");
        assert_eq!(stem_control("deckA", Stem::Drums), "deckA.stemDrums");
        assert_eq!(stem_file(Stem::Other), "other.wav");
    }

    #[test]
    fn a_lane_id_names_its_deck_and_stem_or_none() {
        let decks = ["deckA", "deckB"];
        assert_eq!(stem_lane_of("stemOtherB", &decks), Some(("deckB", Stem::Other)));
        assert_eq!(stem_lane_of("stemOtherC", &decks), None);
        assert_eq!(stem_lane_of("platterA", &decks), None);
    }

    #[test]
    fn the_order_is_the_models() {
        let words: Vec<&str> = STEMS.iter().map(|s| s.word()).collect();
        assert_eq!(words, ["drums", "bass", "other", "vocals"]);
    }
}
