// song_tempo.rs — the song's one tempo, read off project.k7x for an edit that addresses an event by position.
// A reader would get wrong the default: 120, for a tempo that is absent, not a positive finite number, or a root that is not there.

use crate::error::Res;
use crate::fs::Fs;
use crate::project_root::load_project_root;
use crate::start_time::js_to_number;

/// `project.k7x`'s tempo, 120 when it states none.
///
/// # Errors
/// A project root that does not read.
pub fn song_tempo(fs: &dyn Fs, dir: &str) -> Res<f64> {
    let root = load_project_root(fs, dir, false)?;
    let t = root
        .project()
        .and_then(|p| p.get("tempo"))
        .filter(|v| !matches!(v, k7x::edit::Value::Null))
        .map_or(120.0, js_to_number);
    Ok(if t.is_finite() && t > 0.0 { t } else { 120.0 })
}
