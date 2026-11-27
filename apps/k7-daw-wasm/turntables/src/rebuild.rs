// rebuild.rs — has a deck's tempo moved far enough since the last send to be worth another document?
// A reader would get wrong a deck appearing or leaving: it is always worth a document, since only a present pair can be compared.

use crate::vocab::{DECKS, DeckTempos, LIVE_WARP_BPM};

/// Has any deck's tempo moved by `LIVE_WARP_BPM`, or its natural tempo changed at all?
pub fn worth_a_rebuild(sent: &DeckTempos, now: &DeckTempos) -> bool {
    DECKS.iter().any(|d| match (sent.get(*d), now.get(*d)) {
        (Some(a), Some(b)) => (a.target - b.target).abs() >= LIVE_WARP_BPM || a.natural != b.natural,
        (None, None) => false,
        _ => true,
    })
}
