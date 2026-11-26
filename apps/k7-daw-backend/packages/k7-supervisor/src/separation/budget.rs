//! budget.rs — what a cut asks of the machine's cores and memory, with no model and no device in it: a phone's
//! thread count, the memory a cut needs, and the sentence that refuses one.

use std::sync::atomic::{AtomicI32, Ordering};

/// A cut's CPU threads: half the cores on a phone, which has no render pool to spare; a computer keeps five cores back.
pub fn thread_count(cores: usize, phone: bool) -> usize {
    if phone {
        (cores / 2).max(1)
    } else {
        (cores / 2).min(cores.saturating_sub(5)).max(1)
    }
}

/// The weights held twice (the file and ORT's copy), the model's activations, and the song per channel: mix plus four stems and the weights.
pub fn bytes_needed(model_bytes: usize, frames: usize) -> usize {
    const ACTIVATIONS: usize = 400 << 20;
    const CHANNELS: usize = 2;
    const BUFFERS: usize = 6;
    const FLOAT: usize = 4;
    model_bytes * 2 + ACTIVATIONS + frames * CHANNELS * BUFFERS * FLOAT
}

/// None when `available` covers `needed` (and `floor`, a raised minimum for a test), else why the cut is refused.
pub fn memory_refusal(available: usize, needed: usize, floor: usize) -> Option<String> {
    let want = needed.max(floor);
    (available < want).then(|| {
        format!(
            "this device has {} MB of memory free for k7 and a cut needs {} MB: close other apps, or cut on a computer",
            available >> 20,
            want >> 20
        )
    })
}

static IN_FLIGHT: AtomicI32 = AtomicI32::new(0);

/// Cuts that are queued or running, which the iOS app reads to keep itself alive behind another.
pub fn cuts_in_flight() -> i32 {
    IN_FLIGHT.load(Ordering::Relaxed)
}

/// One cut counted from its creation until the guard drops.
pub(super) struct Counted;

impl Counted {
    pub(super) fn new() -> Counted {
        IN_FLIGHT.fetch_add(1, Ordering::Relaxed);
        Counted
    }
}

impl Drop for Counted {
    fn drop(&mut self) {
        IN_FLIGHT.fetch_sub(1, Ordering::Relaxed);
    }
}
