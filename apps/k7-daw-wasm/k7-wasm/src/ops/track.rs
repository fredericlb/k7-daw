// ops/track.rs — the track.* operations: what a track's name declares, read off its `[position.][prefix.]label`.
// A reader would get wrong that the order of lanes is not here: a lane order is computed where the document is built, and the page reads it.

use crate::error::WasmError;
use crate::ops::write;
use serde::{Deserialize, Serialize};
use song::doc::{track_kind, track_label};

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TrackNameArgs {
    pub name: String,
}

/// The instrument a track name declares: its code, its family and the label a row's tooltip shows.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TrackKind {
    pub prefix: String,
    pub family: String,
    pub label: String,
}

/// The track family's requests: `{"op": "track.label", "args": {"name": "3.ge.lead"}}`.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "op", content = "args")]
pub enum TrackRequest {
    /// The name with its position and instrument stripped; the whole name when nothing would remain.
    #[serde(rename = "track.label")]
    Label(TrackNameArgs),
    /// The family and label a name declares, or null when it declares none.
    #[serde(rename = "track.kind")]
    Kind(TrackNameArgs),
}

pub fn dispatch(request: TrackRequest, out: &mut Vec<u8>) -> Result<(), WasmError> {
    match request {
        TrackRequest::Label(a) => write(out, track_label(&a.name)),
        TrackRequest::Kind(a) => write(
            out,
            &track_kind(&a.name).map(|(prefix, family, label)| TrackKind {
                prefix: prefix.to_owned(),
                family: family.name().to_owned(),
                label: label.to_owned(),
            }),
        ),
    }
}
