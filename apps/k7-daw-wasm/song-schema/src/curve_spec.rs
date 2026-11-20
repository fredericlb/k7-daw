// curve_spec.rs — a curve as a file holds it (times as timecodes), and its resolution into beats.
// A reader would get wrong that unknown fields (`conditions`) are ignored on purpose: the spec twin is a subset.

use crate::curve::{Adsr, Curve, Cut, Kind, Point, Points, Polarity, Shape};
use crate::error::SchemaError;
use crate::timecode::{TimeSig, parse_duration, parse_position};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PointSpec {
    pub time: String,
    pub value: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LfoSpec {
    pub polarity: Polarity,
    pub amplitude: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub cut: Option<Cut>,
    pub shape: Shape,
    /// Cycles per bar.
    pub frequency: f64,
    /// 0..1 of a cycle.
    pub phase: f64,
    /// A 0-based timecode; absent is for ever.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub duration: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PointsSpec {
    pub polarity: Polarity,
    pub amplitude: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub cut: Option<Cut>,
    pub points: Vec<PointSpec>,
}

/// The four lengths are in ms, `level` is 0..1.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AdsrSpec {
    pub polarity: Polarity,
    pub amplitude: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub cut: Option<Cut>,
    pub attack: f64,
    pub decay: f64,
    pub sustain: f64,
    pub release: f64,
    pub level: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum CurveSpec {
    Lfo(LfoSpec),
    Custom(PointsSpec),
    Onoff(PointsSpec),
    Adsr(AdsrSpec),
}

fn finite(what: &str, v: f64) -> Result<f64, SchemaError> {
    if v.is_finite() {
        Ok(v)
    } else {
        Err(SchemaError::BadCurve(format!("{what} must be a finite number")))
    }
}

/// A timecode in beats, refused where the digits are too large to be a time.
fn beats(tc: &str, sig: TimeSig, parse: fn(&str, TimeSig) -> Result<f64, SchemaError>) -> Result<f64, SchemaError> {
    let b = parse(tc, sig)?;
    if b.is_finite() {
        Ok(b)
    } else {
        Err(SchemaError::BadTimecode(format!("timecode '{tc}' is too large")))
    }
}

fn points(spec: &PointsSpec, sig: TimeSig) -> Result<Points, SchemaError> {
    let mut out = Vec::with_capacity(spec.points.len());
    for p in &spec.points {
        out.push(Point {
            beat: beats(&p.time, sig, parse_position)?,
            value: finite("a point's value", p.value)?,
        });
    }
    Ok(Points::sorted(out))
}

impl Curve {
    /// A spec resolved against a time signature: beats parsed, points sorted, the span computed.
    pub fn resolve(spec: &CurveSpec, sig: TimeSig) -> Result<Curve, SchemaError> {
        let (polarity, amplitude, cut, kind) = match spec {
            CurveSpec::Lfo(s) => {
                // An absent or empty duration, or one that is not above 0, runs for ever.
                let duration = match s.duration.as_deref() {
                    Some(tc) if !tc.is_empty() => Some(beats(tc, sig, parse_duration)?).filter(|d| *d > 0.0),
                    _ => None,
                };
                let kind = Kind::Lfo {
                    shape: s.shape,
                    frequency: finite("frequency", s.frequency)?,
                    phase: finite("phase", s.phase)?,
                    duration,
                };
                (s.polarity, s.amplitude, s.cut, kind)
            }
            CurveSpec::Custom(s) => (s.polarity, s.amplitude, s.cut, Kind::Custom(points(s, sig)?)),
            CurveSpec::Onoff(s) => (s.polarity, s.amplitude, s.cut, Kind::OnOff(points(s, sig)?)),
            CurveSpec::Adsr(s) => {
                let env = Adsr {
                    attack: finite("attack", s.attack)?,
                    decay: finite("decay", s.decay)?,
                    sustain: finite("sustain", s.sustain)?,
                    release: finite("release", s.release)?,
                    level: finite("level", s.level)?,
                };
                (s.polarity, s.amplitude, s.cut, Kind::Adsr(env))
            }
        };
        Ok(Curve::new(
            polarity,
            finite("amplitude", amplitude)?,
            cut.unwrap_or_default(),
            kind,
            sig,
        ))
    }
}
