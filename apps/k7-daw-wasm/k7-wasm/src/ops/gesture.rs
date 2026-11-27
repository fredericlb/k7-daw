// ops/gesture.rs — the gesture.* operations: the gestures' calls that answer a structure, as JSON ops over the gesture crate: the ones that run once per release, click or scroll.
// A reader would get wrong that the hot calls are not here: those are ../typed.rs, numbers in and numbers out.

use gesture::clip_warp::{SourceMap, WarpPoint, warp_line};
use gesture::roll_drag::{GroupMove, GroupNote, plan_group_move};
use gesture::ruler::{RulerTick, TickSpec, ruler_ticks};
use gesture::tools::{
    ClipTarget, FadeClip, FadePair, ShotTarget, Tool, ToolAct, crossfade_pair, picture_tool_line, tool_line,
};
use crate::error::WasmError;
use crate::ops::write;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct PlanArgs {
    pub group: Vec<GroupNote>,
    pub others: Vec<GroupNote>,
    pub d_beats: f64,
    pub d_keys: f64,
}

pub fn plan_move(a: PlanArgs) -> GroupMove {
    plan_group_move(&a.group, &a.others, a.d_beats, a.d_keys)
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct WarpArgs {
    pub clip: String,
    pub at_sec: f64,
    pub to_sec: f64,
    pub map: SourceMap,
    pub tempo: f64,
    #[serde(default)]
    pub marks: Vec<WarpPoint>,
}

pub fn warp(a: WarpArgs) -> String {
    warp_line(&a.clip, a.at_sec, a.to_sec, &a.map, a.tempo, &a.marks)
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ToolArgs {
    pub tool: Tool,
    pub target: ClipTarget,
}

pub fn clip_tool(a: ToolArgs) -> Option<ToolAct> {
    tool_line(a.tool, &a.target)
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PictureToolArgs {
    pub tool: Tool,
    pub target: ShotTarget,
}

pub fn shot_tool(a: PictureToolArgs) -> Option<ToolAct> {
    picture_tool_line(a.tool, &a.target)
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CrossfadeArgs {
    pub clips: Vec<FadeClip>,
    pub pressed: FadeClip,
    pub at: f64,
}

pub fn crossfade(a: CrossfadeArgs) -> Option<FadePair> {
    crossfade_pair(&a.clips, &a.pressed, a.at)
}

pub fn ticks(a: TickSpec) -> Vec<RulerTick> {
    ruler_ticks(&a)
}

/// The gesture family's requests: `{"op": "gesture.warp_line", "args": {…}}`.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "op", content = "args")]
pub enum GestureRequest {
    #[serde(rename = "gesture.plan_group_move")]
    PlanGroupMove(PlanArgs),
    #[serde(rename = "gesture.warp_line")]
    WarpLine(WarpArgs),
    #[serde(rename = "gesture.tool_line")]
    ToolLine(ToolArgs),
    #[serde(rename = "gesture.picture_tool_line")]
    PictureToolLine(PictureToolArgs),
    #[serde(rename = "gesture.crossfade_pair")]
    CrossfadePair(CrossfadeArgs),
    #[serde(rename = "gesture.ruler_ticks")]
    RulerTicks(TickSpec),
}

pub fn dispatch(request: GestureRequest, out: &mut Vec<u8>) -> Result<(), WasmError> {
    match request {
        GestureRequest::PlanGroupMove(a) => write(out, &plan_move(a)),
        GestureRequest::WarpLine(a) => write(out, &warp(a)),
        GestureRequest::ToolLine(a) => write(out, &clip_tool(a)),
        GestureRequest::PictureToolLine(a) => write(out, &shot_tool(a)),
        GestureRequest::CrossfadePair(a) => write(out, &crossfade(a)),
        GestureRequest::RulerTicks(a) => write(out, &ticks(a)),
    }
}
