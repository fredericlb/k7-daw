// ops/draft.rs — the draft.* operations: the curve editor's draft from a curve, the fields `curve set` writes, a drawn point placed.
// A reader would get wrong that a refusal here is an error the page catches: a half-typed time makes the draft's picture empty, not a crash.

use super::curve::{PreviewArgs, curve_preview};
use crate::error::WasmError;
use crate::ops::write;
use serde::Deserialize;
use song_edit::curve_draft::{
    CurveDraft, DraftCondition, DraftPoint, ParamKind, SpecFile, condition_problem, conditions_problem, draft_of,
    duration_problem, fields_of, new_condition, point_at, shapes_for, span_bars, spec_of, with_point,
};
use song_schema::SchemaError;

fn bad(e: SchemaError) -> WasmError {
    e.into()
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DraftOfArgs {
    /// The curve file the editor opens on; none for a new curve.
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional, type = "unknown"))]
    pub spec: Option<SpecFile>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DraftArgs {
    pub draft: CurveDraft,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DraftConditionArgs {
    pub condition: DraftCondition,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DraftEventArgs {
    pub event: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DraftKindArgs {
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub kind: Option<ParamKind>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DraftMeterArgs {
    pub draft: CurveDraft,
    pub num: f64,
    pub den: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DraftPointAtArgs {
    pub x: f64,
    pub unit: f64,
    pub span_beats: f64,
    pub num: f64,
    pub den: f64,
    #[serde(default)]
    pub binary: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DraftWithPointArgs {
    pub points: Vec<DraftPoint>,
    pub point: DraftPoint,
    pub num: f64,
    pub den: f64,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub replacing: Option<String>,
}

/// The draft family's requests: `{"op": "draft.of", "args": {…}}`.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "op", content = "args")]
pub enum DraftRequest {
    #[serde(rename = "draft.of")]
    Of(DraftOfArgs),
    #[serde(rename = "draft.fields")]
    Fields(DraftArgs),
    #[serde(rename = "draft.preview")]
    Preview(DraftMeterArgs),
    #[serde(rename = "draft.conditions_problem")]
    ConditionsProblem(DraftArgs),
    #[serde(rename = "draft.condition_problem")]
    ConditionProblem(DraftConditionArgs),
    #[serde(rename = "draft.duration_problem")]
    DurationProblem(DraftArgs),
    #[serde(rename = "draft.span_bars")]
    SpanBars(DraftMeterArgs),
    #[serde(rename = "draft.new_condition")]
    NewCondition(DraftEventArgs),
    #[serde(rename = "draft.shapes_for")]
    ShapesFor(DraftKindArgs),
    #[serde(rename = "draft.point_at")]
    PointAt(DraftPointAtArgs),
    #[serde(rename = "draft.with_point")]
    WithPoint(DraftWithPointArgs),
}

pub fn dispatch(request: DraftRequest, out: &mut Vec<u8>) -> Result<(), WasmError> {
    use DraftRequest as R;
    match request {
        R::Of(a) => write(out, &draft_of(a.spec.as_ref())),
        R::Fields(a) => write(out, &fields_of(&a.draft).map_err(bad)?),
        R::Preview(a) => {
            let spec = spec_of(&a.draft).map_err(bad)?;
            let picture = curve_preview(PreviewArgs {
                spec,
                num: a.num as u32,
                den: a.den as u32,
                n: None,
                playhead: None,
            })?;
            write(out, &picture)
        }
        R::ConditionsProblem(a) => write(out, &conditions_problem(&a.draft)),
        R::ConditionProblem(a) => write(out, &condition_problem(&a.condition)),
        R::DurationProblem(a) => write(out, &duration_problem(&a.draft)),
        R::SpanBars(a) => write(out, &span_bars(&a.draft, a.num, a.den).map_err(bad)?),
        R::NewCondition(a) => write(out, &new_condition(&a.event)),
        R::ShapesFor(a) => write(out, &shapes_for(a.kind)),
        R::PointAt(a) => write(
            out,
            &point_at(a.x, a.unit, a.span_beats, a.num, a.den, a.binary).map_err(bad)?,
        ),
        R::WithPoint(a) => write(
            out,
            &with_point(&a.points, &a.point, a.num, a.den, a.replacing.as_deref()).map_err(bad)?,
        ),
    }
}
