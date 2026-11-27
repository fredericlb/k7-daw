// ops/clipboard.rs — the clipboard.* operations: what a rectangle of notes or events copies and where a paste lands.
// A reader would get wrong that a refused paste is an empty list, not an error: what falls off the keyboard or the lanes is dropped and counted.

use crate::error::WasmError;
use crate::ops::write;
use serde::Deserialize;
use song_edit::clipboard::{
    Clipboard, LaneEvents, RollNote, copy_events, copy_notes, dropped_count, notes_in_region, paste_events, paste_notes,
};

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ClipCopyNotesArgs {
    pub notes: Vec<RollNote>,
    pub sequence: String,
    pub lo_key: f64,
    pub hi_key: f64,
    pub from: f64,
    pub to: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ClipPasteNotesArgs {
    pub clip: Clipboard,
    pub top_key: f64,
    pub at: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ClipCopyEventsArgs {
    pub rows: Vec<LaneEvents>,
    pub from: f64,
    pub to: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ClipPasteEventsArgs {
    pub clip: Clipboard,
    pub lanes: Vec<String>,
    pub at: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ClipDroppedArgs {
    pub kept: usize,
    pub clip: Clipboard,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ClipRegionArgs {
    /// Each note's `[key, time]`.
    pub notes: Vec<(f64, f64)>,
    pub lo_key: f64,
    pub hi_key: f64,
    pub from: f64,
    pub to: f64,
}

/// The clipboard family's requests: `{"op": "clipboard.copy_notes", "args": {…}}`.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "op", content = "args")]
pub enum ClipboardRequest {
    #[serde(rename = "clipboard.copy_notes")]
    CopyNotes(ClipCopyNotesArgs),
    #[serde(rename = "clipboard.paste_notes")]
    PasteNotes(ClipPasteNotesArgs),
    #[serde(rename = "clipboard.copy_events")]
    CopyEvents(ClipCopyEventsArgs),
    #[serde(rename = "clipboard.paste_events")]
    PasteEvents(ClipPasteEventsArgs),
    #[serde(rename = "clipboard.dropped")]
    Dropped(ClipDroppedArgs),
    #[serde(rename = "clipboard.in_region")]
    InRegion(ClipRegionArgs),
}

pub fn dispatch(request: ClipboardRequest, out: &mut Vec<u8>) -> Result<(), WasmError> {
    use ClipboardRequest as R;
    match request {
        R::CopyNotes(a) => write(
            out,
            &copy_notes(&a.notes, &a.sequence, a.lo_key, a.hi_key, a.from, a.to),
        ),
        R::PasteNotes(a) => write(out, &paste_notes(&a.clip, a.top_key, a.at)),
        R::CopyEvents(a) => write(out, &copy_events(&a.rows, a.from, a.to)),
        R::PasteEvents(a) => write(out, &paste_events(&a.clip, &a.lanes, a.at)),
        R::Dropped(a) => write(out, &dropped_count(a.kept, &a.clip)),
        R::InRegion(a) => write(out, &notes_in_region(&a.notes, a.lo_key, a.hi_key, a.from, a.to)),
    }
}
