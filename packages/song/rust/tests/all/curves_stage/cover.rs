// curves_stage/cover.rs — what the sweep COVERED, counted by effect: a cut that changed a point, a corner that was cut short.
// A reader would get wrong that a count reads a field: a curve with `cut` or `duration` counts only when the bake differs.

use song::curves::{BakedCurve, CurveCondition, CurveFile, CurveRun, EnvelopeFire, ResolvedCurve};
use song_schema::{CurveSpec, Cut, Kind, TimeSig};

#[derive(Default)]
pub struct Counts {
    pub golden: usize,
    pub negative: usize,
    pub negative_ts: usize,
    pub swept: usize,
    pub wired: usize,
    pub wired_bindings: usize,
    pub kinds: [usize; 4],
    pub conditions: [usize; 4],
    pub retrigger: usize,
    pub duration: usize,
    pub cut_positive: usize,
    pub cut_negative: usize,
    pub many_runs: usize,
    pub cut_short: usize,
    pub widened: usize,
    pub below_one: usize,
    pub four_fields: usize,
    pub deep_fields: usize,
    pub fractional_tempo: usize,
    pub off_grid: usize,
    pub huge_song: usize,
    pub fine_sig: usize,
    pub spent_max: usize,
    pub spent_lane: usize,
    pub skipped_threw: usize,
}

/// One swept case with everything the counts read.
pub struct Case<'a> {
    pub file: &'a CurveFile,
    pub rc: &'a ResolvedCurve,
    pub sig: TimeSig,
    pub tempo: f64,
    pub song: f64,
    pub triggers: &'a [f64],
    pub runs: &'a [CurveRun],
    pub fires: &'a [EnvelopeFire],
    pub baked: &'a BakedCurve,
    /// The same curve baked with no cut.
    pub uncut: &'a BakedCurve,
}

/// The same curve with no cut: the bake of it is what the cut is measured against.
pub fn without_cut(file: &CurveFile) -> CurveFile {
    let mut f = file.clone();
    match &mut f.spec {
        CurveSpec::Lfo(s) => s.cut = None,
        CurveSpec::Custom(s) => s.cut = None,
        CurveSpec::Adsr(s) => s.cut = None,
        CurveSpec::Onoff(_) => {}
    }
    f
}

fn timecodes(file: &CurveFile) -> Vec<&str> {
    let mut out: Vec<&str> = match &file.spec {
        CurveSpec::Custom(p) | CurveSpec::Onoff(p) => p.points.iter().map(|p| p.time.as_str()).collect(),
        _ => Vec::new(),
    };
    for c in file.conditions.iter().flatten() {
        match c {
            CurveCondition::Start { at, .. } | CurveCondition::End { at, .. } => out.push(at),
            CurveCondition::Between { start, end, .. } => out.extend([start.as_str(), end.as_str()]),
            CurveCondition::Event { .. } => {}
        }
    }
    out
}

/// Whether an envelope run by `fires` has a corner cut short: the next firing or the window's end comes before its own end.
fn cut_short(fires: &[EnvelopeFire], len: f64) -> bool {
    let mut live: Vec<EnvelopeFire> = fires
        .iter()
        .copied()
        .filter(|f| f.stop.is_none_or(|s| f.at < s))
        .collect();
    live.sort_by(|a, b| a.at.total_cmp(&b.at));
    let mut kept: Vec<EnvelopeFire> = Vec::new();
    let mut free = f64::NEG_INFINITY;
    for f in live {
        if f.retrigger || f.at >= free {
            free = (f.at + len).min(f.stop.unwrap_or(f64::INFINITY));
            kept.push(f);
        }
    }
    kept.iter().enumerate().any(|(i, f)| {
        let next = kept
            .get(i + 1)
            .map_or(f64::INFINITY, |n| n.at)
            .min(f.stop.unwrap_or(f64::INFINITY));
        f.at + len > next
    })
}

/// Whether the oscillator's grid was widened to its cap: the stretch it runs over holds more than 2,048 steps.
fn widened(c: &Case) -> bool {
    let Kind::Lfo { duration, .. } = c.rc.curve().kind() else {
        return false;
    };
    let bar = c.sig.bar();
    let step = bar / 24.0;
    let span: f64 = if c.file.conditions.is_none() && duration.is_none() {
        c.song.max(bar * 4.0)
    } else {
        c.runs
            .iter()
            .map(|r| (r.to.min(r.origin + duration.unwrap_or(f64::INFINITY)) - r.from).max(0.0))
            .sum()
    };
    span / step > 2048.0
}

pub fn tally(n: &mut Counts, c: &Case) {
    let cut = match &c.file.spec {
        CurveSpec::Lfo(s) => {
            n.kinds[0] += 1;
            s.cut
        }
        CurveSpec::Custom(s) => {
            n.kinds[1] += 1;
            s.cut
        }
        CurveSpec::Onoff(_) => {
            n.kinds[2] += 1;
            None
        }
        CurveSpec::Adsr(s) => {
            n.kinds[3] += 1;
            s.cut
        }
    };
    // A cut counts when the points it leaves differ from the same curve's uncut ones.
    if c.baked.points != c.uncut.points {
        n.cut_positive += usize::from(cut == Some(Cut::Positive));
        n.cut_negative += usize::from(cut == Some(Cut::Negative));
    }
    for cond in c.file.conditions.iter().flatten() {
        let (k, re) = match cond {
            CurveCondition::Start { retrigger, .. } => (0, retrigger),
            CurveCondition::End { retrigger, .. } => (1, retrigger),
            CurveCondition::Between { retrigger, .. } => (2, retrigger),
            CurveCondition::Event { retrigger, .. } => (3, retrigger),
        };
        n.conditions[k] += 1;
        n.retrigger += usize::from(*re == Some(true));
    }
    n.many_runs += usize::from(c.runs.len() > 1);
    // A duration counts when it ended a run before its condition did.
    if let Kind::Lfo { duration: Some(d), .. } = c.rc.curve().kind() {
        n.duration += usize::from(c.runs.iter().any(|r| r.origin + d < r.to));
    }
    n.widened += usize::from(widened(c));
    if let CurveSpec::Adsr(env) = &c.file.spec {
        let ms = env.attack + env.decay + env.sustain + env.release;
        n.cut_short += usize::from(cut_short(c.fires, (ms / 1000.0) * (c.tempo / 60.0)));
    }
    let tcs = timecodes(c.file);
    let head = |t: &&str| t.split('.').next().unwrap_or("").split(':').count();
    n.below_one += usize::from(
        tcs.iter()
            .any(|t| t.split('.').next().is_some_and(|h| h.split(':').any(|f| f == "0"))),
    );
    n.four_fields += usize::from(tcs.iter().any(|t| head(t) == 4));
    n.deep_fields += usize::from(tcs.iter().any(|t| head(t) > 4));
    n.fractional_tempo += usize::from(c.tempo.fract() != 0.0);
    n.off_grid += usize::from(c.triggers.iter().any(|t| (t * 8.0).fract() != 0.0));
    n.huge_song += usize::from(c.song >= 1e5);
    n.fine_sig += usize::from(c.sig.den() >= 64);
}
