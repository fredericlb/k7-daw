//! audio_policy.rs — which interface the engine should be on: the user's list in THEIR order, best first,
//! and the first entry actually plugged in wins. No default and no fallback: a fallback was only ever the
//! second line of this list. Pure, so the rules are tested without an engine
//! (`apps/k7-daw-backend/packages/k7-supervisor/docs/supervisorPolicy.md`, "AudioPolicy").

use crate::protocol::AudioDevice;

/// Control characters as separators, so a device name can never forge one.
const US: char = '\u{1f}';
const RS: char = '\u{1e}';

/// The set of visible interfaces as one string: a switch that did not take is retried when it moves.
pub fn signature(devices: &[AudioDevice]) -> String {
    let mut names: Vec<&str> = devices.iter().map(|d| d.name.as_str()).collect();
    names.sort_unstable();
    names.join(&US.to_string())
}

/// Everything the audio rows depend on, `current` and the ranking included.
pub fn menu_signature(devices: &[AudioDevice], priority: &[String]) -> String {
    let mut rows: Vec<String> = devices
        .iter()
        .map(|d| {
            format!(
                "{}|{}|{}|{}",
                d.name, d.device_type, d.current, d.current_input
            )
        })
        .collect();
    rows.sort_unstable();
    // The ranking IN ORDER, unsorted: a reorder that moved no membership is the whole point of it.
    format!(
        "{}{RS}{}",
        rows.join(&US.to_string()),
        priority.join(&US.to_string())
    )
}

/// The best-ranked interface that is actually there.
pub fn best_present<'a>(devices: &[AudioDevice], priority: &'a [String]) -> Option<&'a str> {
    priority
        .iter()
        .find(|n| devices.iter().any(|d| &d.name == *n))
        .map(String::as_str)
}

/// The device to switch to, or nothing when the engine is where it belongs or nothing ranked is there.
pub fn desired_switch<'a>(devices: &[AudioDevice], priority: &'a [String]) -> Option<&'a str> {
    // The engine answered with nothing: don't thrash.
    if devices.is_empty() {
        return None;
    }
    // Nothing ranked is plugged in: a device we did not choose beats none.
    let want = best_present(devices, priority)?;
    match devices.iter().find(|d| d.current) {
        Some(cur) if cur.name == want => None,
        _ => Some(want),
    }
}
