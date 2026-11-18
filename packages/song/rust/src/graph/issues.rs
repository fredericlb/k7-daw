// graph/issues.rs — validate()'s warnings as codes with their names, and the English sentence of each, verbatim.
// A reader would get wrong that a name quoted here is cut to NAME_CUT: a closed code set may not grow, so a hostile name may not either (410).

use super::NAME_CUT;
use indexmap::IndexMap;
use serde::Serialize;

macro_rules! codes {
    ($($variant:ident => $name:literal),+ $(,)?) => {
        /// What `validate` can say of a graph: `GraphWarningCode` of `@studio/graph-model`, spelled as there.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
        #[cfg_attr(feature = "ts", derive(ts_rs::TS))]
        pub enum WarningCode {
            $(#[serde(rename = $name)] #[cfg_attr(feature = "ts", ts(rename = $name))] $variant),+
        }

        impl WarningCode {
            pub const ALL: &'static [WarningCode] = &[$(WarningCode::$variant),+];

            pub fn name(self) -> &'static str {
                match self { $(WarningCode::$variant => $name),+ }
            }
        }
    };
}

codes! {
    ChainFromUnknown => "chainFromUnknown",
    ChainMixedLane => "chainMixedLane",
    ChainStepUnknown => "chainStepUnknown",
    ChainExtendsUnknown => "chainExtendsUnknown",
    ChainTransitionWired => "chainTransitionWired",
    OutIsInput => "outIsInput",
    OutIsCamera => "outIsCamera",
    OutIsMidiDevice => "outIsMidiDevice",
    OutIsPictureLane => "outIsPictureLane",
    OutIsTrack => "outIsTrack",
    OutUnknown => "outUnknown",
    OutNegativeGain => "outNegativeGain",
    OutBadPort => "outBadPort",
    OutRenderPort => "outRenderPort",
    OutRenderSoundNotMix => "outRenderSoundNotMix",
    ChainDeadEnd => "chainDeadEnd",
    TrackClipMissing => "trackClipMissing",
    ModuleAssetMissing => "moduleAssetMissing",
    InputNoChannel => "inputNoChannel",
    InputTrackDisabled => "inputTrackDisabled",
    InputTrackMissing => "inputTrackMissing",
    InputTrackGroup => "inputTrackGroup",
    InputTrackStrudel => "inputTrackStrudel",
    TrackTwoInputs => "trackTwoInputs",
    ModuleChannelNotInput => "moduleChannelNotInput",
    GroupContainsItself => "groupContainsItself",
    GroupTrackMissing => "groupTrackMissing",
    TrackTwoGroups => "trackTwoGroups",
    AssemblyNameTaken => "assemblyNameTaken",
    AssemblyMemberMissing => "assemblyMemberMissing",
    NodeTwoAssemblies => "nodeTwoAssemblies",
    ChainNoOut => "chainNoOut",
    ChainCycle => "chainCycle",
}

/// One warning as data: its code, the names in it, and the sentence a log prints.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct GraphIssue {
    pub code: WarningCode,
    #[cfg_attr(feature = "ts", ts(type = "Record<string, string>"))]
    pub params: IndexMap<&'static str, String>,
    pub text: String,
}

impl GraphIssue {
    /// `code|name=value|…` in the order the params were given: what a test and the gate compare.
    pub fn key(&self) -> String {
        let mut out = self.code.name().to_owned();
        for (k, v) in &self.params {
            out.push('|');
            out.push_str(k);
            out.push('=');
            out.push_str(v);
        }
        out
    }
}

/// `s` cut to `NAME_CUT` characters (410).
pub fn cut(s: &str) -> String {
    s.chars().take(NAME_CUT).collect()
}

fn p<'a>(params: &'a IndexMap<&'static str, String>, key: &str) -> &'a str {
    params.get(key).map_or("", String::as_str)
}

fn sentence(code: WarningCode, q: &IndexMap<&'static str, String>) -> String {
    use WarningCode as C;
    let g = |k: &str| p(q, k);
    match code {
        C::ChainFromUnknown => format!("chain '{}': from '{}' is not a track or chain", g("chain"), g("from")),
        C::ChainMixedLane => format!(
            "chain '{}': track '{}' holds audio AND sequences, and '{}' takes {} — all of it goes through it. Split the lane: '{}#audio:' and '{}#midi:'",
            g("chain"),
            g("track"),
            g("module"),
            g("wants"),
            g("track"),
            g("track")
        ),
        C::ChainStepUnknown => format!(
            "chain '{}': step module '{}' is not a module or module node{}",
            g("chain"),
            g("module"),
            g("hint")
        ),
        C::ChainExtendsUnknown => format!(
            "chain '{}': module '{}' extends unknown module '{}'",
            g("chain"),
            g("module"),
            g("base")
        ),
        C::ChainTransitionWired => format!(
            "chain '{}': '{}' is a transition, and a transition is not wired — a cut names it (`fadeIn({{ over: 1, with: \"{}\" }})` on a shot of a picture lane)",
            g("chain"),
            g("module"),
            g("module")
        ),
        C::OutIsInput => format!(
            "chain '{}': out '{}' is a live input — signal comes FROM it, not to it",
            g("chain"),
            g("out")
        ),
        C::OutIsCamera => format!(
            "chain '{}': out '{}' is a camera — picture comes FROM it, not to it",
            g("chain"),
            g("out")
        ),
        C::OutIsMidiDevice => format!(
            "chain '{}': out '{}' is a MIDI device — notes come FROM it, not to it",
            g("chain"),
            g("out")
        ),
        C::OutIsPictureLane => format!(
            "chain '{}': out '{}' is a picture lane — only a camera's or a shape's picture lands on one, or a run of shaders nothing feeds (a fade's `chain:`)",
            g("chain"),
            g("out")
        ),
        C::OutIsTrack => format!(
            "chain '{}': out '{}' is a track — only a live input's or a MIDI device's signal lands on a lane",
            g("chain"),
            g("out")
        ),
        C::OutUnknown => format!("chain '{}': out '{}' is not a chain or io sink", g("chain"), g("out")),
        C::OutNegativeGain => format!("chain '{}': out '{}' has a negative gain", g("chain"), g("out")),
        C::OutBadPort => {
            let outputs = if g("outputs").is_empty() { "none" } else { g("outputs") };
            format!(
                "chain '{}': out '{}' leaves '{}' through '{}', which is not one of its outputs ({outputs})",
                g("chain"),
                g("out"),
                g("source"),
                g("port")
            )
        }
        C::OutRenderPort => format!(
            "chain '{}': out '{}' is no input of RENDER — it takes the picture on 'in' and the sound on 'audio'",
            g("chain"),
            g("out")
        ),
        C::OutRenderSoundNotMix => format!(
            "chain '{}': out '{}' — only the mix can be the video's sound for now (MIX: [\"RENDER#audio\"]); this one is not in the export",
            g("chain"),
            g("out")
        ),
        C::ChainDeadEnd => format!("chain '{}': has no out (dead end)", g("chain")),
        C::TrackClipMissing => format!("track '{}': clip/sequence '{}' not found", g("track"), g("ref")),
        C::ModuleAssetMissing => format!(
            "module '{}': {} '{}' is not in this module's assets/",
            g("module"),
            g("param"),
            g("ref")
        ),
        C::InputNoChannel => format!("input '{}': no channel — it feeds nothing", g("input")),
        C::InputTrackDisabled => format!(
            "input '{}': track '{}' is disabled — it records nothing",
            g("input"),
            g("track")
        ),
        C::InputTrackMissing => format!("input '{}': track '{}' not found", g("input"), g("track")),
        C::InputTrackGroup => format!(
            "input '{}': track '{}' is a group — it holds no material",
            g("input"),
            g("track")
        ),
        C::InputTrackStrudel => format!(
            "input '{}': track '{}' is a strudel lane — its pattern is the source",
            g("input"),
            g("track")
        ),
        C::TrackTwoInputs => format!(
            "track '{}': fed by two inputs ('{}', '{}') — only one can win",
            g("track"),
            g("first"),
            g("second")
        ),
        C::ModuleChannelNotInput => format!(
            "module '{}': channel is for an io \"in\" module, but '{}' is not one",
            g("module"),
            g("module")
        ),
        C::GroupContainsItself => format!("group '{}': contains itself", g("group")),
        C::GroupTrackMissing => format!("group '{}': track '{}' not found", g("group"), g("track")),
        C::TrackTwoGroups => format!(
            "track '{}': in two groups ('{}', '{}')",
            g("track"),
            g("first"),
            g("second")
        ),
        C::AssemblyNameTaken => format!(
            "assembly '{}': a node of this project is already called that",
            g("assembly")
        ),
        C::AssemblyMemberMissing => format!(
            "assembly '{}': member '{}' names '{}', which this project has not got",
            g("assembly"),
            g("member"),
            g("node")
        ),
        C::NodeTwoAssemblies => format!(
            "node '{}': in two assemblies ('{}', '{}')",
            g("node"),
            g("first"),
            g("second")
        ),
        C::ChainNoOut => format!("chain '{}': nothing reaches OUT", g("chain")),
        C::ChainCycle => format!("chain '{}': is part of a cycle", g("chain")),
    }
}

/// One warning, its sentence built from the same names the code carries; every name is already cut by the caller.
pub fn issue<const N: usize>(code: WarningCode, params: [(&'static str, String); N]) -> GraphIssue {
    let params: IndexMap<&'static str, String> = params.into_iter().collect();
    let text = sentence(code, &params);
    GraphIssue { code, params, text }
}
