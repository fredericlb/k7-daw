// grid.rs — laying a corrected tempo over what the analysis measured.
// A reader would get wrong whose it is: the correction belongs to the RECORD, so 87 BPM is 87 in every project that plays it.

use crate::vocab::DeckTempo;

/// The analysis with a correction laid over it: `natural` is what the record IS, and `target` follows unless a fader has moved.
pub fn corrected(tempo: &DeckTempo, bpm: Option<f64>) -> DeckTempo {
    let Some(bpm) = bpm else { return tempo.clone() };
    let followed = tempo.target == tempo.natural;
    DeckTempo {
        natural: bpm,
        // A fader already moved is a decision; only an untouched one follows the correction.
        target: if followed { bpm } else { tempo.target },
        ..tempo.clone()
    }
}
