// doc/own_mix.rs — what a module hands to the engine's own stage: its blend, the level into it and the level out of it (decision 314).
// A reader would get wrong that a module with TWO inputs keeps its `Mix`: a crossfade's `Mix` blends A against B, and is no dry/wet.

use crate::files::scalar::js_space;
use crate::modules::{LoadedModule, ModuleKind};

/// The intrinsic a module's engine-side dry/wet is bound and sent by.
pub const MIX_PARAM: &str = "@mix";
/// The two levels on the same engine stage, in dB: into an audio module, and out of anything with audio out.
pub const INPUT_VOLUME_PARAM: &str = "@input-volume";
pub const OUTPUT_VOLUME_PARAM: &str = "@output-volume";

/// What the decision reads off a resolved module.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BlendShape<'a> {
    pub kind: Option<ModuleKind>,
    pub category: Option<&'a str>,
    pub inputs: usize,
}

impl<'a> BlendShape<'a> {
    pub fn of(m: &'a LoadedModule) -> BlendShape<'a> {
        BlendShape {
            kind: m.kind,
            category: m.category.as_deref(),
            inputs: m.authored.inputs.as_ref().map_or(0, Vec::len),
        }
    }

    /// `(m.kind ?? "plugin") === "plugin"`: a module with no kind is read as a plugin.
    fn is_plugin(&self) -> bool {
        self.kind.is_none_or(|k| k == ModuleKind::Plugin)
    }
}

/// Which side a plugin's own level param is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LevelSide {
    Input,
    Output,
}

/// Which way a plugin's own blend param is held when the engine owns the blend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MixSide {
    Wet,
    Dry,
}

/// Does the engine own this module's blend? An effect (`audio/`) with at most one declared input.
pub fn owns_blend(m: Option<BlendShape<'_>>) -> bool {
    m.is_some_and(|m| m.is_plugin() && m.category == Some("audio") && m.inputs <= 1)
}

/// Does the engine set the level INTO this module? Any audio module, whatever its inputs.
pub fn takes_input_volume(m: Option<BlendShape<'_>>) -> bool {
    m.is_some_and(|m| m.is_plugin() && m.category == Some("audio"))
}

/// …and OUT of it? Anything a plugin makes audio with: an effect or an instrument.
pub fn takes_output_volume(m: Option<BlendShape<'_>>) -> bool {
    m.is_some_and(|m| m.is_plugin() && matches!(m.category, Some("audio" | "instruments")))
}

/// `^(stem|short)(\s+(tail))?$` read case-insensitively, over ASCII letters alone, as the regex without `u` reads it.
fn level_name(name: &str, stems: [&str; 2]) -> bool {
    const TAILS: [&str; 6] = ["volume", "vol", "gain", "level", "lvl", "trim"];
    stems.iter().any(|stem| {
        let (Some(head), Some(rest)) = (name.get(..stem.len()), name.get(stem.len()..)) else {
            return false;
        };
        let tail = rest.trim_start_matches(js_space);
        head.eq_ignore_ascii_case(stem)
            && (rest.is_empty() || (tail.len() < rest.len() && TAILS.iter().any(|t| tail.eq_ignore_ascii_case(t))))
    })
}

/// Which LEVEL a plugin's own param is, by its WHOLE name: "Output", "Input Gain", "Out Level".
pub fn own_level_side(name: &str) -> Option<LevelSide> {
    if level_name(name, ["input", "in"]) {
        Some(LevelSide::Input)
    } else if level_name(name, ["output", "out"]) {
        Some(LevelSide::Output)
    } else {
        None
    }
}

/// Whether some whole run of ASCII word characters (`\b`'s word) is one of `words`, ignoring case.
fn has_word_run(name: &str, words: &[&str]) -> bool {
    name.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .any(|run| words.iter().any(|w| run.eq_ignore_ascii_case(w)))
}

/// Which way a plugin's OWN blend param is held when the engine owns the blend: all wet, or no dry.
pub fn own_mix_side(name: &str) -> Option<MixSide> {
    if name.starts_with('@') {
        None
    } else if has_word_run(name, &["wet", "mix"]) {
        Some(MixSide::Wet)
    } else if has_word_run(name, &["dry"]) {
        Some(MixSide::Dry)
    } else {
        None
    }
}

/// Every param of `names` the engine's stage HOLDS on this module: the ones a module may not set.
pub fn held_params<'a>(m: Option<BlendShape<'_>>, names: impl IntoIterator<Item = &'a str>) -> Vec<&'a str> {
    names
        .into_iter()
        .filter(|n| match own_level_side(n) {
            Some(LevelSide::Input) => takes_input_volume(m),
            Some(LevelSide::Output) => takes_output_volume(m),
            None => owns_blend(m) && own_mix_side(n).is_some(),
        })
        .collect()
}
