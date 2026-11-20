// curves/bake.rs — a curve baked to the [beat, value] breakpoints the engine plays; after curveBake.ts and cutPoints.
// A reader would get wrong that an envelope is placed by its EVENTS (bake_adsr) and every other curve by song time (bake_curve).

use super::conditions::{CurveRun, EnvelopeFire, curve_runs};
use super::resolved::ResolvedCurve;
use super::{Budget, Over};
use serde::Serialize;
use song_schema::{Cut, Kind, cut_output, round_beat};

/// Breakpoints per bar for an oscillator; the engine interpolates.
const LFO_BAKE_PER_BAR: f64 = 24.0;

/// A cap, so a long song cannot explode the point count.
const LFO_BAKE_MAX_POINTS: f64 = 2048.0;

/// A microbeat: under a sample at any tempo, and the point grid's own step.
const EPS: f64 = 1e-6;

/// A curve baked to engine breakpoints; `points` are [beat, output] in the curve's own range.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct BakedCurve {
    pub bipolar: bool,
    pub points: Vec<[f64; 2]>,
}

fn by_beat(a: f64, b: f64) -> std::cmp::Ordering {
    (a + 0.0).total_cmp(&(b + 0.0))
}

/// Breakpoints with a cut applied: a point added where a segment crosses 0, so the engine's line is cut there.
pub fn cut_points(points: &[[f64; 2]], cut: Cut, budget: &mut Budget) -> Result<Vec<[f64; 2]>, Over> {
    let mut out: Vec<[f64; 2]> = Vec::new();
    for (i, [b, v]) in points.iter().copied().enumerate() {
        budget.spend(1)?;
        if let Some([pb, pv]) = i.checked_sub(1).and_then(|p| points.get(p)).copied()
            && pv * v < 0.0
            && b > pb
        {
            let at = pb + ((b - pb) * -pv) / (v - pv);
            out.push([round_beat(at), 0.0]);
        }
        out.push([b, round_beat(cut_output(v, cut))]);
    }
    Ok(out)
}

/// An on/off curve's corners, each preceded by a microbeat holding the value before it: the engine steps, never ramps.
pub(crate) fn step_beats(corners: &[f64]) -> Vec<f64> {
    let mut sorted = corners.to_vec();
    sorted.sort_by(|a, b| by_beat(*a, *b));
    let mut out = Vec::with_capacity(sorted.len().saturating_mul(2));
    for (i, b) in sorted.iter().enumerate() {
        if i > 0 {
            out.push(b - EPS);
        }
        out.push(*b);
    }
    out
}

/// The flat answer for an envelope asked for song-time breakpoints: silence, the bottom when bipolar.
fn flat(bipolar: bool) -> BakedCurve {
    BakedCurve {
        bipolar,
        points: vec![[0.0, if bipolar { -1.0 } else { 0.0 }]],
    }
}

/// Bake over the song: a drawn curve verbatim, an oscillator on a grid, any under its conditions; the cut after.
pub fn bake_curve(
    c: &ResolvedCurve,
    song_beats: f64,
    triggers: &[f64],
    budget: &mut Budget,
) -> Result<BakedCurve, Over> {
    // Cut AFTER the shape is baked, as an envelope is: see cut_points.
    let whole = bake_whole(c, song_beats, triggers, budget)?;
    if c.cut == Cut::None {
        return Ok(whole);
    }
    Ok(BakedCurve {
        points: cut_points(&whole.points, c.cut, budget)?,
        ..whole
    })
}

fn lfo_duration(c: &ResolvedCurve) -> Option<f64> {
    match c.curve.kind() {
        Kind::Lfo { duration, .. } => *duration,
        _ => None,
    }
}

fn bake_whole(c: &ResolvedCurve, song_beats: f64, triggers: &[f64], budget: &mut Budget) -> Result<BakedCurve, Over> {
    if c.is_envelope() {
        // No triggers here: bake_adsr is the entry point that has them, and flat keeps this total.
        return Ok(flat(c.bipolar));
    }
    if !c.conditions.is_empty() || lfo_duration(c).is_some() {
        return bake_conditioned(c, song_beats, triggers, budget);
    }
    let mut points: Vec<[f64; 2]> = Vec::new();
    match c.curve.kind() {
        Kind::Custom(_) | Kind::OnOff(_) => {
            budget.spend(c.drawn.len())?;
            points.reserve(c.drawn.len());
            for b in &c.drawn {
                points.push([round_beat(*b), round_beat(c.curve.sample(*b))]);
            }
        }
        _ => {
            let bar = c.curve.sig().bar();
            let end = song_beats.max(bar * 4.0);
            let mut step = bar / LFO_BAKE_PER_BAR;
            if end / step > LFO_BAKE_MAX_POINTS {
                step = end / LFO_BAKE_MAX_POINTS;
            }
            // The accumulation is kept as the TypeScript's: a step that stops moving ends in the budget.
            let mut b = 0.0;
            while b <= end + step * 0.5 {
                budget.spend(1)?;
                points.push([round_beat(b), round_beat(c.curve.sample(b))]);
                b += step;
            }
        }
    }
    Ok(BakedCurve {
        bipolar: c.bipolar,
        points,
    })
}

/// A song-time curve under its conditions and its duration: sampled over each run, its own time counted from the run's origin.
fn bake_conditioned(
    c: &ResolvedCurve,
    song_beats: f64,
    triggers: &[f64],
    budget: &mut Budget,
) -> Result<BakedCurve, Over> {
    let bar = c.curve.sig().bar();
    let end = song_beats.max(bar * 4.0);
    // A duration ends each run that long after its own start; past it the engine holds where it stopped.
    let dur = lfo_duration(c).unwrap_or(f64::INFINITY);
    let runs: Vec<CurveRun> = curve_runs(c, end, triggers, budget)?
        .into_iter()
        .map(|r| CurveRun {
            to: r.to.min(r.origin + dur),
            ..r
        })
        .filter(|r| r.to > r.from)
        .collect();
    let span: f64 = runs.iter().map(|r| r.to - r.from).sum();
    let mut step = bar / LFO_BAKE_PER_BAR;
    if span / step > LFO_BAKE_MAX_POINTS {
        step = span / LFO_BAKE_MAX_POINTS;
    }
    let is_lfo = matches!(c.curve.kind(), Kind::Lfo { .. });
    let drawn = c.drawn_sorted();
    let mut out: Vec<[f64; 2]> = Vec::new();
    for run in &runs {
        budget.spend(1)?;
        // A drawn curve's corners where they fall in the run, an oscillator's grid; both ends of the run always.
        let mut at: Vec<f64> = vec![run.from, run.to];
        if is_lfo {
            // Short of the end by a microbeat: a step landing a hair under it would round onto it and be nudged past.
            let mut b = run.from + step;
            while b < run.to - EPS {
                budget.spend(1)?;
                at.push(b);
                b += step;
            }
        } else {
            let first = drawn.partition_point(|b| b + run.origin <= run.from);
            for b in drawn.iter().skip(first).take_while(|b| **b + run.origin < run.to) {
                budget.spend(1)?;
                at.push(b + run.origin);
            }
        }
        at.sort_by(|a, b| by_beat(*a, *b));
        at.dedup();
        for b in at {
            budget.spend(1)?;
            // A retriggered run starts where the last ended: nudged a microbeat on, so the jump is a step, not a slope.
            let beat = match out.last() {
                Some(last) if b <= last[0] => last[0] + EPS,
                _ => b,
            };
            out.push([round_beat(beat), round_beat(c.curve.sample(b - run.origin))]);
        }
    }
    Ok(BakedCurve {
        bipolar: c.bipolar,
        points: out,
    })
}

/// The firings kept: a retriggering one always, one that may not cut only when no envelope still runs.
fn spaced(sorted: &[EnvelopeFire], len: f64) -> Vec<EnvelopeFire> {
    let mut out = Vec::new();
    let mut free = f64::NEG_INFINITY;
    for f in sorted {
        if f.retrigger || f.at >= free {
            out.push(*f);
            free = (f.at + len).min(f.stop.unwrap_or(f64::INFINITY));
        }
    }
    out
}

/// One corner of an envelope: a value on a beat, nudged past a coincident one, dropped when it repeats it.
fn put(out: &mut Vec<[f64; 2]>, c: &ResolvedCurve, beat: f64, unit: f64, budget: &mut Budget) -> Result<(), Over> {
    let v6 = round_beat(c.curve.output(unit));
    let last = out.last().copied();
    // A corner on the beat one already sits at is NUDGED past it, never dropped.
    let coincident = last.is_some_and(|l| beat <= l[0] + EPS / 2.0);
    // Unless it repeats the value, which says nothing the point before it did not.
    if coincident && last.is_some_and(|l| l[1] == v6) {
        return Ok(());
    }
    budget.spend(1)?;
    let at = match last {
        Some(l) if coincident => l[0] + EPS,
        _ => beat,
    };
    out.push([round_beat(at), v6]);
    Ok(())
}

/// An envelope at each of `fires`, five corners apiece, an early firing CUTTING it; the cut after.
pub fn bake_adsr(
    c: &ResolvedCurve,
    fires: &[EnvelopeFire],
    tempo: f64,
    budget: &mut Budget,
) -> Result<BakedCurve, Over> {
    // Cut AFTER the shape is baked, so a corner crossing 0 gets its crossing rather than a slope to the clamp.
    let whole = bake_adsr_whole(c, fires, tempo, budget)?;
    if c.cut == Cut::None {
        return Ok(whole);
    }
    Ok(BakedCurve {
        points: cut_points(&whole.points, c.cut, budget)?,
        ..whole
    })
}

fn bake_adsr_whole(
    c: &ResolvedCurve,
    fires: &[EnvelopeFire],
    tempo: f64,
    budget: &mut Budget,
) -> Result<BakedCurve, Over> {
    let Kind::Adsr(env) = c.curve.kind() else {
        return Ok(BakedCurve {
            bipolar: c.bipolar,
            points: Vec::new(),
        });
    };
    let to_beats = |ms: f64| (ms / 1000.0) * (tempo / 60.0);
    let to_ms = |b: f64| (b * 60000.0) / tempo;
    let (a, d, s, r) = (
        to_beats(env.attack),
        to_beats(env.decay),
        to_beats(env.sustain),
        to_beats(env.release),
    );
    budget.spend(fires.len())?;
    let mut sorted: Vec<EnvelopeFire> = fires
        .iter()
        .copied()
        .filter(|f| f.stop.is_none_or(|s| f.at < s))
        .collect();
    sorted.sort_by(|x, y| by_beat(x.at, y.at));
    let at = spaced(&sorted, a + d + s + r);
    let mut out: Vec<[f64; 2]> = Vec::new();
    for (i, fire) in at.iter().enumerate() {
        let t = fire.at;
        let next = at
            .get(i + 1)
            .map_or(f64::INFINITY, |n| n.at)
            .min(fire.stop.unwrap_or(f64::INFINITY));
        let corners = [
            (t, 0.0),
            (t + a, 1.0),
            (t + a + d, env.level),
            (t + a + d + s, env.level),
            (t + a + d + s + r, 0.0),
        ];
        for (b, u) in corners {
            // Strictly past, not at: one ENDING where the next starts has nothing left to cut.
            if b > next {
                // Cut short at the value the shape had actually reached, so the line into it is its own.
                put(&mut out, c, next - EPS, env.unit(to_ms(next - t)), budget)?;
                break;
            }
            put(&mut out, c, b, u, budget)?;
        }
    }
    Ok(BakedCurve {
        bipolar: c.bipolar,
        points: out,
    })
}
