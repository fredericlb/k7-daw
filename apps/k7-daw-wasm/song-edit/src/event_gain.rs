// event_gain.rs — one audio event's own trim, `gain:` in dB on its placement, ±6, to a tenth of a dB, deleted at 0 (unity).
// A reader would get wrong the range: it is a balance between takes, not a fader.

use crate::error::Res;
use crate::fs::Fs;
use crate::song_tempo::song_tempo;
use crate::start_time::js_round;
use crate::track_edit::edit_track_struct;
use crate::track_file::{find_placement_at, set_placement_field};
use k7x::edit::Value;

/// The trim an event may carry, either way.
pub const EVENT_GAIN_MAX_DB: f64 = 6.0;

/// `db` as the file keeps it: clamped, to a tenth of a dB, and None for unity.
#[must_use]
pub fn event_gain_value(db: f64) -> Option<f64> {
    let c = (-EVENT_GAIN_MAX_DB).max(EVENT_GAIN_MAX_DB.min(db));
    let r = js_round(c * 10.0) / 10.0;
    (r != 0.0).then_some(r)
}

/// Writes the trim of the placement at `from_beats`; false when no event sits there.
///
/// # Errors
/// A lane with no file, a project root that does not read, or the write's.
pub fn set_track_placement_gain(
    fs: &mut dyn Fs,
    dir: &str,
    id: &str,
    from_beats: f64,
    meter: (f64, f64),
    db: f64,
) -> Res<bool> {
    let mut hit = false;
    let tempo = song_tempo(fs, dir)?;
    edit_track_struct(fs, dir, id, "set an event's gain", |doc| {
        let Some(i) = find_placement_at(doc, from_beats, meter.0, meter.1, tempo)? else {
            return Ok(());
        };
        hit = true;
        set_placement_field(doc, i, "gain", event_gain_value(db).map(Value::Num))
    })?;
    Ok(hit)
}
