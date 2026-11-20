// curve.rs — what a RESOLVED curve's value is: wave, envelope, interpolation, then amplitude, polarity and cut.
// A reader would get wrong that a UNIT value is 0..1 before amplitude and polarity, and that times here are beats.

use crate::preview::Span;
use crate::timecode::TimeSig;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum Shape {
    Sine,
    Triangle,
    Saw,
    Square,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum Polarity {
    Unipolar,
    Bipolar,
}

/// Which half of the output a curve keeps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum Cut {
    #[default]
    None,
    Positive,
    Negative,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub beat: f64,
    pub value: f64,
}

/// Points in time order: the only constructor sorts, so a lookup may binary-search.
#[derive(Clone, Debug, PartialEq)]
pub struct Points(Vec<Point>);

impl Points {
    /// Stable by beat, so coincident points keep their order; `-0` and `0` are one beat.
    pub fn sorted(mut points: Vec<Point>) -> Points {
        points.sort_by(|a, b| (a.beat + 0.0).total_cmp(&(b.beat + 0.0)));
        Points(points)
    }

    pub fn as_slice(&self) -> &[Point] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Adsr {
    pub attack: f64,
    pub decay: f64,
    pub sustain: f64,
    pub release: f64,
    pub level: f64,
}

impl Adsr {
    /// The envelope's own length in ms: what one trigger occupies before it is over.
    pub fn ms(&self) -> f64 {
        self.attack + self.decay + self.sustain + self.release
    }

    /// The envelope's unit value `ms` after its trigger, flat 0 once over.
    pub fn unit(&self, ms: f64) -> f64 {
        if ms <= 0.0 {
            return 0.0;
        }
        if ms < self.attack {
            return ms / self.attack;
        }
        let d = ms - self.attack;
        if d < self.decay {
            return 1.0 - (1.0 - self.level) * (d / self.decay);
        }
        let h = d - self.decay;
        if h < self.sustain {
            return self.level;
        }
        let r = h - self.sustain;
        if r < self.release {
            return self.level * (1.0 - r / self.release);
        }
        0.0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    /// `duration` in beats, `None` for ever.
    Lfo {
        shape: Shape,
        frequency: f64,
        phase: f64,
        duration: Option<f64>,
    },
    Custom(Points),
    /// Points of 0 or 1, each HELD until the next.
    OnOff(Points),
    Adsr(Adsr),
}

/// A curve with its times in beats, its time signature and its picture's span fixed.
#[derive(Clone, Debug, PartialEq)]
pub struct Curve {
    pub(crate) polarity: Polarity,
    pub(crate) amplitude: f64,
    pub(crate) cut: Cut,
    pub(crate) kind: Kind,
    pub(crate) sig: TimeSig,
    pub(crate) span: Span,
}

/// An oscillator's unit value `cycles` into its run, 0..1.
fn lfo_wave(shape: Shape, cycles: f64) -> f64 {
    let f = cycles - cycles.floor();
    match shape {
        Shape::Triangle => {
            if f < 0.5 {
                2.0 * f
            } else {
                2.0 * (1.0 - f)
            }
        }
        Shape::Saw => f,
        Shape::Square => {
            if f < 0.5 {
                1.0
            } else {
                0.0
            }
        }
        Shape::Sine => ((2.0 * std::f64::consts::PI * cycles).sin() + 1.0) / 2.0,
    }
}

/// Linear between points, flat outside; coincident points take the later value.
fn points_unit(pts: &[Point], beat: f64) -> f64 {
    let (Some(first), Some(last)) = (pts.first(), pts.last()) else {
        return 0.0;
    };
    if beat <= first.beat {
        return first.value;
    }
    if beat >= last.beat {
        return last.value;
    }
    let i = pts.partition_point(|p| p.beat < beat).max(1);
    let (Some(a), Some(b)) = (pts.get(i - 1), pts.get(i)) else {
        return last.value;
    };
    let span = b.beat - a.beat;
    if span <= 0.0 {
        b.value
    } else {
        a.value + (b.value - a.value) * ((beat - a.beat) / span)
    }
}

/// Each point's value until the next, the first's before it.
fn held_unit(pts: &[Point], beat: f64) -> f64 {
    let Some(first) = pts.first() else {
        return 0.0;
    };
    match pts.partition_point(|p| p.beat <= beat) {
        0 => first.value,
        k => pts.get(k - 1).map_or(first.value, |p| p.value),
    }
}

/// An output with its cut applied.
pub fn cut_output(v: f64, cut: Cut) -> f64 {
    match cut {
        Cut::Positive => v.max(0.0),
        Cut::Negative => v.min(0.0),
        Cut::None => v,
    }
}

impl Curve {
    /// A curve over `kind`, its picture's span computed once.
    pub fn new(polarity: Polarity, amplitude: f64, cut: Cut, kind: Kind, sig: TimeSig) -> Curve {
        let span = crate::preview::span_of(&kind, sig);
        Curve {
            polarity,
            amplitude,
            cut,
            kind,
            sig,
            span,
        }
    }

    pub fn span(&self) -> Span {
        self.span
    }

    pub fn kind(&self) -> &Kind {
        &self.kind
    }

    pub fn sig(&self) -> TimeSig {
        self.sig
    }

    /// The unit value scaled by amplitude, unipolar 0..amp or bipolar -amp..amp, then cut.
    pub fn output(&self, unit: f64) -> f64 {
        let u = if self.polarity == Polarity::Bipolar {
            unit * 2.0 - 1.0
        } else {
            unit
        };
        cut_output(u * self.amplitude, self.cut)
    }

    /// The output at song beat `beat`; an ADSR has none there, only after a trigger, so its unit is 0.
    pub fn sample(&self, beat: f64) -> f64 {
        let unit = match &self.kind {
            Kind::Adsr(_) => 0.0,
            Kind::Lfo {
                shape,
                frequency,
                phase,
                ..
            } => lfo_wave(*shape, frequency * (beat / self.sig.bar()) + phase),
            Kind::OnOff(p) => held_unit(p.as_slice(), beat),
            Kind::Custom(p) => points_unit(p.as_slice(), beat),
        };
        self.output(unit)
    }
}
