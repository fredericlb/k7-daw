// doc/highlight.rs — a lane HIGHLIGHTED from the mixer plays 10 dB up, its fader left where it is (decision 324).
// A reader would get wrong that the boost is the LAST mixer's output stage, on top of what it had, up to the stage's own +12 dB.

use super::PluginDoc;
use super::own_mix::OUTPUT_VOLUME_PARAM;
use crate::resolve::MIXER_PLUGIN;

/// How much louder a highlighted lane plays, dB.
const HIGHLIGHT_DB: f64 = 10.0;
/// The output stage's ceiling, the engine's own.
const STAGE_MAX_DB: f64 = 12.0;

/// A lane's plugins with its strip, the LAST mixer of the run, set to play `HIGHLIGHT_DB` louder; no mixer, nothing.
pub fn highlighted(plugins: &mut [PluginDoc]) {
    let Some(mixer) = plugins.iter_mut().rev().find(|p| p.plugin_type == MIXER_PLUGIN) else {
        return;
    };
    // `Number(x) || 0`: a NaN or an absent level counts as zero.
    let level = mixer
        .params
        .get(OUTPUT_VOLUME_PARAM)
        .copied()
        .filter(|v| !v.is_nan())
        .unwrap_or(0.0);
    mixer
        .params
        .insert(OUTPUT_VOLUME_PARAM.to_owned(), (level + HIGHLIGHT_DB).min(STAGE_MAX_DB));
}
