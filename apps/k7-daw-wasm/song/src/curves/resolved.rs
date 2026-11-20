// curves/resolved.rs — a CurveFile against a time signature: the pilot's Curve, the conditions in beats, the beats it draws at.
// A reader would get wrong that the Curve inside has NO cut: the bake samples the whole shape and cuts after (cut_points).

use super::bake::step_beats;
use super::conditions::is_note_on_event;
use super::file::{CurveCondition, CurveFile};
use song_schema::{Curve, CurveSpec, Cut, Kind, Polarity, SchemaError, TimeSig, parse_position};

/// One condition with its timecodes parsed to song beats.
#[derive(Clone, Debug, PartialEq)]
pub enum Condition {
    Start {
        at: f64,
        retrigger: bool,
    },
    End {
        at: f64,
        retrigger: bool,
    },
    Between {
        start: f64,
        end: f64,
        retrigger: bool,
    },
    /// `note_on` is whether the event is the one a node announces.
    Event {
        note_on: bool,
        retrigger: bool,
    },
}

/// A curve file resolved once against the song's signature; pure, so a memo of it changes no answer.
#[derive(Clone, Debug)]
pub struct ResolvedCurve {
    pub(crate) curve: Curve,
    pub(crate) cut: Cut,
    pub(crate) bipolar: bool,
    pub(crate) conditions: Vec<Condition>,
    /// The beats a drawn curve is baked at, in the order the bake writes them (an on/off curve's stepped).
    pub(crate) drawn: Vec<f64>,
    /// The same beats sorted, when the bake order is not (an on/off curve's microbeats can cross).
    sorted: Option<Vec<f64>>,
}

fn finite_beat(tc: &str, sig: TimeSig) -> Result<f64, SchemaError> {
    let b = parse_position(tc, sig)?;
    if b.is_finite() {
        Ok(b)
    } else {
        Err(SchemaError::BadTimecode(format!("timecode '{tc}' is too large")))
    }
}

fn condition(c: &CurveCondition, sig: TimeSig) -> Result<Condition, SchemaError> {
    Ok(match c {
        CurveCondition::Start { at, retrigger } => Condition::Start {
            at: finite_beat(at, sig)?,
            retrigger: retrigger.unwrap_or(false),
        },
        CurveCondition::End { at, retrigger } => Condition::End {
            at: finite_beat(at, sig)?,
            retrigger: retrigger.unwrap_or(false),
        },
        CurveCondition::Between { start, end, retrigger } => Condition::Between {
            start: finite_beat(start, sig)?,
            end: finite_beat(end, sig)?,
            retrigger: retrigger.unwrap_or(false),
        },
        CurveCondition::Event { event, retrigger } => Condition::Event {
            note_on: is_note_on_event(event),
            retrigger: retrigger.unwrap_or(false),
        },
    })
}

impl ResolvedCurve {
    pub fn new(file: &CurveFile, sig: TimeSig) -> Result<ResolvedCurve, SchemaError> {
        // The pilot resolves the spec without its cut, so sample and output leave the cut to cut_points.
        let mut spec = file.spec.clone();
        let (cut, polarity) = match &mut spec {
            CurveSpec::Lfo(s) => (s.cut.take(), s.polarity),
            CurveSpec::Custom(s) | CurveSpec::Onoff(s) => (s.cut.take(), s.polarity),
            CurveSpec::Adsr(s) => (s.cut.take(), s.polarity),
        };
        let curve = Curve::resolve(&spec, sig)?;
        let conditions = file
            .conditions
            .iter()
            .flatten()
            .map(|c| condition(c, sig))
            .collect::<Result<Vec<_>, _>>()?;
        let (drawn, sorted) = match curve.kind() {
            Kind::Custom(points) => (points.as_slice().iter().map(|p| p.beat).collect(), None),
            Kind::OnOff(points) => {
                let stepped = step_beats(&points.as_slice().iter().map(|p| p.beat).collect::<Vec<_>>());
                let mut sorted = stepped.clone();
                sorted.sort_by(|a, b| (a + 0.0).total_cmp(&(b + 0.0)));
                (stepped, Some(sorted))
            }
            Kind::Lfo { .. } | Kind::Adsr(_) => (Vec::new(), None),
        };
        Ok(ResolvedCurve {
            curve,
            cut: cut.unwrap_or_default(),
            bipolar: polarity == Polarity::Bipolar,
            conditions,
            drawn,
            sorted,
        })
    }

    /// The beats a drawn curve has corners at, in time order.
    pub(crate) fn drawn_sorted(&self) -> &[f64] {
        self.sorted.as_deref().unwrap_or(&self.drawn)
    }

    pub fn curve(&self) -> &Curve {
        &self.curve
    }

    pub fn cut(&self) -> Cut {
        self.cut
    }

    pub fn bipolar(&self) -> bool {
        self.bipolar
    }

    pub fn conditions(&self) -> &[Condition] {
        &self.conditions
    }

    /// Whether the curve is an envelope: placed by its events, not by song time.
    pub fn is_envelope(&self) -> bool {
        matches!(self.curve.kind(), Kind::Adsr(_))
    }
}
