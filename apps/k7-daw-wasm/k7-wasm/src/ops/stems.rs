// ops/stems.rs — the stems.* operations: the names a deck gives one stem's lane, platter and control, and which lane an id is.
// A reader would get wrong that the four stems are a constant (`STEMS`): only the names built from a deck's letter are asked here.

use crate::error::WasmError;
use crate::ops::write;
use serde::{Deserialize, Serialize};
use song_schema::{Stem, deck_platter, stem_control, stem_file, stem_lane, stem_lane_of, stem_platter};

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DeckStemArgs {
    pub deck: String,
    pub stem: Stem,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DeckArgs {
    pub deck: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct StemArgs {
    pub stem: Stem,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LaneOfArgs {
    pub id: String,
    pub decks: Vec<String>,
}

/// The deck and stem a lane id names.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct StemLane {
    pub deck: String,
    pub stem: Stem,
}

/// The stems family's requests: `{"op": "stems.lane", "args": {"deck": "deckA", "stem": "bass"}}`.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "op", content = "args")]
pub enum StemsRequest {
    #[serde(rename = "stems.lane")]
    Lane(DeckStemArgs),
    #[serde(rename = "stems.platter")]
    Platter(DeckStemArgs),
    #[serde(rename = "stems.deck_platter")]
    DeckPlatter(DeckArgs),
    #[serde(rename = "stems.control")]
    Control(DeckStemArgs),
    #[serde(rename = "stems.file")]
    File(StemArgs),
    #[serde(rename = "stems.lane_of")]
    LaneOf(LaneOfArgs),
}

pub fn dispatch(request: StemsRequest, out: &mut Vec<u8>) -> Result<(), WasmError> {
    use StemsRequest as R;
    match request {
        R::Lane(a) => write(out, &stem_lane(&a.deck, a.stem)),
        R::Platter(a) => write(out, &stem_platter(&a.deck, a.stem)),
        R::DeckPlatter(a) => write(out, &deck_platter(&a.deck)),
        R::Control(a) => write(out, &stem_control(&a.deck, a.stem)),
        R::File(a) => write(out, &stem_file(a.stem)),
        R::LaneOf(a) => {
            let decks: Vec<&str> = a.decks.iter().map(String::as_str).collect();
            write(
                out,
                &stem_lane_of(&a.id, &decks).map(|(deck, stem)| StemLane {
                    deck: deck.to_owned(),
                    stem,
                }),
            )
        }
    }
}
