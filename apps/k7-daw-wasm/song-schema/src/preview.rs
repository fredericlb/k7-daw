// preview.rs — a curve's picture, the one the editor and the graph's card both draw, answered in one value.
// An envelope has no song time: its picture is over its own length in ms, and it has no span, beat lines or playhead.

use crate::curve::{Curve, Kind};
use crate::timecode::TimeSig;
use serde::Serialize;

/// How much of the next cycle an endless oscillator's picture shows, faded.
const NEXT_CYCLE: f64 = 0.25;

/// The pictured room an envelope's release is given past its own length.
const ADSR_ROOM: f64 = 1.1;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct XY {
    pub x: f64,
    pub y: f64,
}

/// What the picture spans in beats and how much of it is solid.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Span {
    pub beats: f64,
    pub solid: f64,
}

/// One vertical line per metrical beat, `bar` on every bar's first.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct BeatLine {
    pub x: f64,
    pub bar: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CurvePreview {
    pub span: Span,
    pub solid: Vec<XY>,
    pub faded: Vec<XY>,
    pub beats: Vec<BeatLine>,
    pub playhead: Option<XY>,
}

/// The span of a curve of this kind: a drawn curve its bars, an endless oscillator a cycle and a bit.
pub(crate) fn span_of(kind: &Kind, sig: TimeSig) -> Span {
    let bar = sig.bar();
    match kind {
        Kind::Adsr(_) => Span { beats: 0.0, solid: 0.0 },
        Kind::Custom(points) | Kind::OnOff(points) => {
            let last = points.as_slice().iter().map(|p| p.beat).fold(0.0, f64::max);
            let bars = ((last / bar + 0.001).ceil() + 1.0).max(4.0);
            Span {
                beats: bars * bar,
                solid: bars * bar,
            }
        }
        Kind::Lfo {
            frequency, duration, ..
        } => match duration {
            Some(d) if *d > 0.0 => Span { beats: *d, solid: *d },
            _ => {
                // A non-positive frequency is drawn as a frequency of 1, where the TypeScript drew infinity.
                let cycle = bar / if *frequency > 0.0 { *frequency } else { 1.0 };
                Span {
                    beats: cycle * (1.0 + NEXT_CYCLE),
                    solid: cycle,
                }
            }
        },
    }
}

/// `n`+1 samples, x 0..1 across the span, y the output.
fn samples(c: &Curve, n: usize) -> Vec<XY> {
    let n = n.max(1);
    let at = |i: usize| i as f64 / n as f64;
    if let Kind::Adsr(env) = c.kind() {
        let total = env.ms().max(1.0) * ADSR_ROOM;
        return (0..=n)
            .map(|i| XY {
                x: at(i),
                y: c.output(env.unit(at(i) * total)),
            })
            .collect();
    }
    let beats = c.span().beats;
    (0..=n)
        .map(|i| XY {
            x: at(i),
            y: c.sample(at(i) * beats),
        })
        .collect()
}

/// The solid span and the faded rest, both through an exact sample at the boundary.
fn parts(c: &Curve, n: usize) -> (Vec<XY>, Vec<XY>) {
    let pts = samples(c, n);
    let Span { beats, solid } = c.span();
    let cut = if beats != 0.0 { solid / beats } else { 1.0 };
    if cut >= 1.0 {
        return (pts, Vec::new());
    }
    let edge = XY {
        x: cut,
        y: c.sample(solid),
    };
    let mut s: Vec<XY> = pts.iter().copied().filter(|p| p.x < cut - 1e-9).collect();
    s.push(edge);
    let mut f = vec![edge];
    f.extend(pts.iter().copied().filter(|p| p.x > cut + 1e-9));
    (s, f)
}

/// Beat lines allowed per sample: past it the lines thin out to whole bars, so a huge span stays small.
const LINES_PER_SAMPLE: f64 = 4.0;

/// The sample count the cap never falls below: a coarse picture of a long curve keeps its grid.
const MIN_LINE_SAMPLES: usize = 160;

/// One line per metrical beat across the span, thinned to at most four per sample; none for an envelope.
fn beat_lines(c: &Curve, n: usize) -> Vec<BeatLine> {
    let beats = c.span().beats;
    if beats == 0.0 {
        return Vec::new();
    }
    let sig = c.sig();
    let beat_len = sig.beat();
    let count = (beats / beat_len + 1e-6).floor();
    let num = u64::from(sig.num());
    // Every stride-th beat, a whole number of bars, so a bar line stays a bar line.
    let stride = if count <= LINES_PER_SAMPLE * n.max(MIN_LINE_SAMPLES) as f64 {
        1
    } else {
        ((count / (LINES_PER_SAMPLE * n.max(MIN_LINE_SAMPLES) as f64)).ceil() as u64).div_ceil(num) * num
    };
    (0..=(count as u64) / stride)
        .map(|k| BeatLine {
            x: ((k * stride) as f64 * beat_len) / beats,
            bar: (k * stride) % num == 0,
        })
        .collect()
}

/// Where song beat `beat` falls on the picture; an endless oscillator wraps, a finite curve holds its end.
pub fn playhead(c: &Curve, beat: f64) -> Option<XY> {
    let Span { beats, solid } = c.span();
    if beats == 0.0 || beat < 0.0 {
        return None;
    }
    let endless = matches!(c.kind(), Kind::Lfo { .. }) && solid < beats;
    let at = if endless { beat % solid } else { beat.min(solid) };
    Some(XY {
        x: at / beats,
        y: c.sample(at),
    })
}

/// The whole picture in `n` samples, with the playhead at song beat `at` when asked.
pub fn preview(c: &Curve, n: usize, at: Option<f64>) -> CurvePreview {
    let (solid, faded) = parts(c, n);
    CurvePreview {
        span: c.span(),
        solid,
        faded,
        beats: beat_lines(c, n),
        playhead: at.and_then(|b| playhead(c, b)),
    }
}
