// curve_draft.rs — the curve editor's state (`ui curve`) and everything it computes without a DOM: a draft from a curve or from nothing, the fields `curve set` writes.
// A reader would get wrong that a draft keeps EVERY field whichever shape is picked: switching shape loses nothing, and only `fields_of` drops what the shape lacks.

use crate::start_time::js_round;
use k7x::edit::js_trim;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use song_schema::{
    ADSR_DEFAULT_LEVEL, AdsrSpec, CurveSpec, Cut, LfoSpec, PointSpec, PointsSpec, Polarity, SchemaError, Shape,
    beats_to_position_on, is_timecode, parse_duration_on, parse_position_on,
};

/// What the shape select offers: the four oscillators, the envelope, the drawn curve, and a boolean's on/off.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum DraftShape {
    Sine,
    Triangle,
    Saw,
    Square,
    Adsr,
    Custom,
    Onoff,
}

/// One of WHEN the curve runs: from a moment, until one, between two, or on its node's event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum DraftWhen {
    Start,
    End,
    Between,
    Event,
}

/// What kind of param the curve drives: absent when none is in view, which offers every shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum ParamKind {
    Number,
    Boolean,
}

/// The shapes a param of this kind can follow: a number every curve but on/off, a boolean on/off alone.
#[must_use]
pub fn shapes_for(kind: Option<ParamKind>) -> Vec<DraftShape> {
    use DraftShape::{Adsr, Custom, Onoff, Saw, Sine, Square, Triangle};
    match kind {
        Some(ParamKind::Boolean) => vec![Onoff],
        Some(ParamKind::Number) => vec![Sine, Triangle, Saw, Square, Adsr, Custom],
        None => vec![Sine, Triangle, Saw, Square, Adsr, Custom, Onoff],
    }
}

/// Whether the shape is made of points drawn by hand: the custom curve and the on/off.
#[must_use]
pub fn is_drawn_shape(s: DraftShape) -> bool {
    matches!(s, DraftShape::Custom | DraftShape::Onoff)
}

/// One condition as the editor holds it: every moment kept, so switching its kind loses nothing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DraftCondition {
    pub when: DraftWhen,
    /// `start`'s and `between`'s opening, `end`'s close: one field per moment, so switching keeps them.
    pub start_at: String,
    pub end_at: String,
    pub event: String,
    pub retrigger: bool,
}

/// A new condition's fields, before the person picks anything.
#[must_use]
pub fn new_condition(event: &str) -> DraftCondition {
    DraftCondition {
        when: DraftWhen::Start,
        start_at: "1:1:1".to_owned(),
        end_at: "5:1:1".to_owned(),
        event: event.to_owned(),
        retrigger: false,
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DraftPoint {
    pub time: String,
    pub value: f64,
}

/// Every field the editor holds, whichever shape is picked.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CurveDraft {
    /// The curve runs while ANY of them holds; none is always.
    pub conditions: Vec<DraftCondition>,
    pub shape: DraftShape,
    pub polarity: Polarity,
    pub amplitude: f64,
    /// Which half of the output it keeps: written only when not `none`.
    pub cut: Cut,
    pub frequency: f64,
    /// An oscillator's length, a 0-based timecode: "0" runs for ever.
    pub duration: String,
    pub phase: f64,
    pub attack: f64,
    pub decay: f64,
    pub sustain: f64,
    pub release: f64,
    pub level: f64,
    pub points: Vec<DraftPoint>,
}

/// When a curve runs, as a file writes it; `retrigger` only when it is on.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Condition {
    Start {
        at: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        retrigger: Option<bool>,
    },
    End {
        at: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        retrigger: Option<bool>,
    },
    Between {
        start: String,
        end: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        retrigger: Option<bool>,
    },
    Event {
        event: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        retrigger: Option<bool>,
    },
}

/// A curve file as the page holds it: its spec and the conditions when there are any.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct SpecFile {
    #[serde(flatten)]
    pub spec: CurveSpec,
    #[serde(default)]
    pub conditions: Option<Vec<Condition>>,
}

fn draft_condition(c: &Condition) -> DraftCondition {
    let base = |retrigger: &Option<bool>| DraftCondition {
        retrigger: retrigger.unwrap_or(false),
        ..new_condition("Note On")
    };
    match c {
        Condition::Start { at, retrigger } => DraftCondition {
            when: DraftWhen::Start,
            start_at: at.clone(),
            ..base(retrigger)
        },
        Condition::End { at, retrigger } => DraftCondition {
            when: DraftWhen::End,
            end_at: at.clone(),
            ..base(retrigger)
        },
        Condition::Between { start, end, retrigger } => DraftCondition {
            when: DraftWhen::Between,
            start_at: start.clone(),
            end_at: end.clone(),
            ..base(retrigger)
        },
        Condition::Event { event, retrigger } => DraftCondition {
            when: DraftWhen::Event,
            event: event.clone(),
            ..base(retrigger)
        },
    }
}

/// The draft a curve opens on: its own fields over the defaults, or the defaults alone for a new one.
#[must_use]
pub fn draft_of(file: Option<&SpecFile>) -> CurveDraft {
    let base = CurveDraft {
        conditions: vec![],
        shape: DraftShape::Sine,
        polarity: Polarity::Unipolar,
        amplitude: 1.0,
        cut: Cut::None,
        frequency: 1.0,
        duration: "0".to_owned(),
        phase: 0.0,
        attack: 50.0,
        decay: 200.0,
        sustain: 300.0,
        release: 400.0,
        level: ADSR_DEFAULT_LEVEL,
        points: vec![
            DraftPoint {
                time: "1:1:1".to_owned(),
                value: 0.0,
            },
            DraftPoint {
                time: "3:1:1".to_owned(),
                value: 1.0,
            },
        ],
    };
    let Some(file) = file else { return base };
    let (polarity, amplitude, cut) = match &file.spec {
        CurveSpec::Lfo(s) => (s.polarity, s.amplitude, s.cut),
        CurveSpec::Custom(s) | CurveSpec::Onoff(s) => (s.polarity, s.amplitude, s.cut),
        CurveSpec::Adsr(s) => (s.polarity, s.amplitude, s.cut),
    };
    let common = CurveDraft {
        conditions: file.conditions.iter().flatten().map(draft_condition).collect(),
        polarity,
        amplitude,
        cut: cut.unwrap_or(Cut::None),
        ..base
    };
    match &file.spec {
        CurveSpec::Lfo(s) => CurveDraft {
            shape: match s.shape {
                Shape::Sine => DraftShape::Sine,
                Shape::Triangle => DraftShape::Triangle,
                Shape::Saw => DraftShape::Saw,
                Shape::Square => DraftShape::Square,
            },
            frequency: s.frequency,
            duration: s.duration.clone().unwrap_or_else(|| "0".to_owned()),
            phase: s.phase,
            ..common
        },
        CurveSpec::Custom(s) | CurveSpec::Onoff(s) => CurveDraft {
            shape: if matches!(file.spec, CurveSpec::Custom(_)) { DraftShape::Custom } else { DraftShape::Onoff },
            points: s.points.iter().map(|p| DraftPoint { time: p.time.clone(), value: p.value }).collect(),
            ..common
        },
        CurveSpec::Adsr(s) => CurveDraft {
            shape: DraftShape::Adsr,
            attack: s.attack,
            decay: s.decay,
            sustain: s.sustain,
            release: s.release,
            level: s.level,
            ..common
        },
    }
}

/// An oscillator's duration in beats, 0 for ever: and for a length that is not a timecode yet.
fn duration_beats(tc: &str, num: f64, den: f64) -> Result<f64, SchemaError> {
    let t = js_trim(tc);
    if is_timecode(t) { parse_duration_on(t, num, den) } else { Ok(0.0) }
}

/// The draft as the curve it describes: what the preview samples.
///
/// # Errors
/// A duration too large to be a time.
pub fn spec_of(d: &CurveDraft) -> Result<CurveSpec, SchemaError> {
    let cut = (d.cut != Cut::None).then_some(d.cut);
    let (polarity, amplitude) = (d.polarity, d.amplitude);
    let points = |round: bool| {
        d.points
            .iter()
            .map(|p| PointSpec {
                time: p.time.clone(),
                value: if round { f64::from(u8::from(p.value >= 0.5)) } else { p.value },
            })
            .collect::<Vec<_>>()
    };
    Ok(match d.shape {
        DraftShape::Custom => CurveSpec::Custom(PointsSpec {
            polarity,
            amplitude,
            cut,
            points: points(false),
        }),
        // A boolean has no depth and no sign, and its points are off or on whatever the draft held.
        DraftShape::Onoff => CurveSpec::Onoff(PointsSpec {
            polarity: Polarity::Unipolar,
            amplitude: 1.0,
            cut: None,
            points: points(true),
        }),
        DraftShape::Adsr => CurveSpec::Adsr(AdsrSpec {
            polarity,
            amplitude,
            cut,
            attack: d.attack,
            decay: d.decay,
            sustain: d.sustain,
            release: d.release,
            level: d.level,
        }),
        osc => CurveSpec::Lfo(LfoSpec {
            polarity,
            amplitude,
            cut,
            shape: match osc {
                DraftShape::Triangle => Shape::Triangle,
                DraftShape::Saw => Shape::Saw,
                DraftShape::Square => Shape::Square,
                _ => Shape::Sine,
            },
            frequency: d.frequency,
            phase: d.phase,
            // Only a finite one is carried: "0", or a length not yet typed whole, runs for ever.
            duration: (duration_beats(&d.duration, 4.0, 4.0)? > 0.0).then(|| js_trim(&d.duration).to_owned()),
        }),
    })
}

/// One draft condition as the file writes it; `retrigger` only when it is on.
fn condition_of(d: &DraftCondition) -> Condition {
    let retrigger = d.retrigger.then_some(true);
    let t = |s: &str| js_trim(s).to_owned();
    match d.when {
        DraftWhen::Start => Condition::Start {
            at: t(&d.start_at),
            retrigger,
        },
        DraftWhen::End => Condition::End {
            at: t(&d.end_at),
            retrigger,
        },
        DraftWhen::Between => Condition::Between {
            start: t(&d.start_at),
            end: t(&d.end_at),
            retrigger,
        },
        DraftWhen::Event => Condition::Event {
            event: t(&d.event),
            retrigger,
        },
    }
}

/// The fields `curve set` writes: the spec, the polarity only when not the default, the conditions when there are any.
///
/// # Errors
/// A duration too large to be a time.
pub fn fields_of(d: &CurveDraft) -> Result<Map<String, Value>, SchemaError> {
    let conditions: Vec<Value> = d.conditions.iter().filter_map(|c| serde_json::to_value(condition_of(c)).ok()).collect();
    let spec = spec_of(d)?;
    let mut fields = match serde_json::to_value(&spec) {
        Ok(Value::Object(m)) => m,
        _ => Map::new(),
    };
    // An on/off curve's file is its points and conditions, and nothing a boolean cannot have.
    if d.shape == DraftShape::Onoff {
        fields.retain(|k, _| matches!(k.as_str(), "type" | "points"));
    } else if let Some(polarity) = fields.shift_remove("polarity")
        && polarity == Value::String("bipolar".into())
    {
        fields.insert("polarity".into(), polarity);
    }
    if !conditions.is_empty() {
        fields.insert("conditions".into(), Value::Array(conditions));
    }
    Ok(fields)
}

/// Why one condition cannot be written, or None: a moment that is not a timecode, an event with no name.
#[must_use]
pub fn condition_problem(d: &DraftCondition) -> Option<String> {
    let moments: Vec<&str> = match d.when {
        DraftWhen::Start => vec![&d.start_at],
        DraftWhen::End => vec![&d.end_at],
        DraftWhen::Between => vec![&d.start_at, &d.end_at],
        DraftWhen::Event => vec![],
    };
    if let Some(bad) = moments.into_iter().find(|m| !is_timecode(js_trim(m))) {
        return Some(format!("\"{bad}\" is not a timecode — like 5:1:1"));
    }
    (d.when == DraftWhen::Event && js_trim(&d.event).is_empty()).then(|| "name the event".to_owned())
}

/// Why the draft cannot be saved yet, or None: the first of its conditions that cannot be written.
#[must_use]
pub fn conditions_problem(d: &CurveDraft) -> Option<String> {
    d.conditions.iter().find_map(condition_problem)
}

/// Why the duration cannot be saved, or None.
#[must_use]
pub fn duration_problem(d: &CurveDraft) -> Option<String> {
    if d.shape == DraftShape::Adsr || is_drawn_shape(d.shape) || is_timecode(js_trim(&d.duration)) {
        return None;
    }
    Some(format!("\"{}\" is not a length — like 2:0, or 0 for ever", d.duration))
}

/// How many bars the plot spans for a song-time curve: four, or past the last drawn point.
///
/// # Errors
/// A point whose time is not a timecode.
pub fn span_bars(d: &CurveDraft, num: f64, den: f64) -> Result<f64, SchemaError> {
    if !is_drawn_shape(d.shape) || d.points.is_empty() {
        return Ok(4.0);
    }
    let bar_beats = num * (4.0 / den);
    let mut last = f64::NEG_INFINITY;
    for p in &d.points {
        last = last.max(parse_position_on(&p.time, num, den)?);
    }
    Ok(4.0_f64.max((last / bar_beats + 0.001).ceil() + 1.0))
}

/// A sixteenth: where a drawn point snaps in time.
const SNAP_BEATS: f64 = 0.25;

/// A drawn point at plot position (`x` 0..1, `unit` value 0..1), snapped to a sixteenth, and with `binary` to off or on.
///
/// # Errors
/// A time signature with no timecode.
pub fn point_at(x: f64, unit: f64, span_beats: f64, num: f64, den: f64, binary: bool) -> Result<DraftPoint, SchemaError> {
    let beat = 0.0_f64.max(js_round((x * span_beats) / SNAP_BEATS) * SNAP_BEATS);
    let clamped = 1.0_f64.min(0.0_f64.max(unit));
    let value = if binary {
        f64::from(u8::from(clamped >= 0.5))
    } else {
        js_round(clamped * 1000.0) / 1000.0
    };
    Ok(DraftPoint {
        time: beats_to_position_on(beat, num, den)?,
        value,
    })
}

/// The points in time order, one per moment: the later write wins, as `curve point add` sets rather than duplicates.
///
/// # Errors
/// A point whose time is not a timecode.
pub fn with_point(
    points: &[DraftPoint],
    p: &DraftPoint,
    num: f64,
    den: f64,
    replacing: Option<&str>,
) -> Result<Vec<DraftPoint>, SchemaError> {
    // By BEAT, not by spelling: `2:1` and `2:1:1` are one moment.
    let beat_of = |time: &str| parse_position_on(time, num, den);
    let at = beat_of(&p.time)?;
    let gone = match replacing {
        Some(r) => beat_of(r)?,
        None => f64::NAN,
    };
    let mut kept: Vec<(f64, DraftPoint)> = Vec::new();
    for q in points {
        let b = beat_of(&q.time)?;
        // NaN (no point replaced) is not near anything, so every point passes this test.
        let is_gone = (b - gone).abs() < 1e-9;
        if (b - at).abs() > 1e-9 && !is_gone {
            kept.push((b, q.clone()));
        }
    }
    kept.push((at, p.clone()));
    kept.sort_by(|a, b| a.0.total_cmp(&b.0));
    Ok(kept.into_iter().map(|(_, q)| q).collect())
}
