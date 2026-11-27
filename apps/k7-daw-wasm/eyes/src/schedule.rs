// schedule.rs — which shots a layer shows at each time, as a table of spans the page looks up instead of working the rule out.
// A reader would get wrong that a span holds the SHAPE of a frame only: its blend's progress is a ramp the page evaluates at `t`.

use crate::js_min;
use crate::scene::SceneClip;
use serde::Serialize;

/// A value that climbs (or falls) from 0 to 1 across `len` seconds from `start`, held at the ends.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(rename = "ScheduleRamp"))]
pub struct Ramp {
    pub start: f64,
    pub len: f64,
    pub rising: bool,
}

fn clamp01(v: f64) -> f64 {
    if v < 0.0 {
        0.0
    } else if v > 1.0 {
        1.0
    } else {
        v
    }
}

impl Ramp {
    /// The ramp's value at `t`, in 0..1.
    pub fn at(&self, t: f64) -> f64 {
        let v = clamp01((t - self.start) / self.len);
        if self.rising { v } else { 1.0 - v }
    }
}

/// A shot inside its fade's STRETCH: which run, and how present the shot is: 1 whole, 0 gone.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(rename = "ScheduleStretch"))]
pub struct Stretch {
    pub chain: String,
    pub presence: Ramp,
}

/// The program between two pictures, and how far along it is.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(rename = "ScheduleBlend", optional_fields))]
pub struct Blend {
    pub name: String,
    pub progress: Ramp,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stretch_from: Option<Stretch>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stretch_to: Option<Stretch>,
}

/// One stretch of time over which a layer's frame keeps one shape: `at` to `until`, half-open, `until` absent for ever.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(rename = "ScheduleSpan", optional_fields))]
pub struct Span {
    pub at: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub until: Option<f64>,
    /// The shot on top (an index of `clips`): the one a still lane's visibility and a cut read.
    pub top: u32,
    /// The picture going OUT, absent for the transparent texel.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<u32>,
    /// The picture coming IN.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<u32>,
    /// Absent draws `to` plain, as a cut does.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blend: Option<Blend>,
}

/// A layer's schedule: where each shot ends (absent for open) and the spans in time order, with no gap filled.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(rename = "LayerSchedule"))]
pub struct LayerSchedule {
    #[cfg_attr(feature = "ts", ts(type = "Array<number | null>"))]
    pub ends: Vec<Option<f64>>,
    pub spans: Vec<Span>,
}

impl LayerSchedule {
    /// The span covering `t`, or none where the layer shows nothing.
    pub fn span_at(&self, t: f64) -> Option<&Span> {
        let at = self.spans.partition_point(|s| s.at <= t);
        let s = self.spans.get(at.checked_sub(1)?)?;
        s.until.is_none_or(|u| t < u).then_some(s)
    }
}

/// Where the placement at `i` stops: its own length, else the NEXT entry's start, else open.
fn end_of(clips: &[SceneClip], i: usize) -> f64 {
    let c = &clips[i];
    if c.length > 0.0 {
        return c.start + c.length;
    }
    clips.get(i + 1).map_or(f64::INFINITY, |n| n.start)
}

/// Every clip active at `t` in file order, half-open; a lane has two decoders, so the two on TOP win.
fn actives(clips: &[SceneClip], ends: &[f64], t: f64) -> Vec<usize> {
    let mut out: Vec<usize> = (0..clips.len())
        .filter(|&i| !(t < clips[i].start) && t < ends[i])
        .collect();
    if out.len() > 2 {
        out = out.split_off(out.len() - 2);
    }
    out
}

fn chain_of(f: &crate::scene::Fade) -> Option<&str> {
    f.chain.as_deref().filter(|c| !c.is_empty())
}

/// A shot inside its OWN fade-out's stretch at `t`, measured back from its end.
fn leaving(clips: &[SceneClip], ends: &[f64], i: usize, t: f64) -> Option<Stretch> {
    let fade = clips[i].fade_out.as_ref()?;
    let chain = chain_of(fade)?;
    if !ends[i].is_finite() {
        return None;
    }
    let len = js_min(fade.over, ends[i] - clips[i].start);
    let from = ends[i] - len;
    if !(len > 0.0) || t < from {
        return None;
    }
    let presence = Ramp {
        start: from,
        len,
        rising: false,
    };
    Some(Stretch {
        chain: chain.to_owned(),
        presence,
    })
}

/// The whole frame rule for one layer at `t`: which two pictures, the blend between them and each side's stretch.
fn span_for(clips: &[SceneClip], ends: &[f64], t: f64) -> Option<Span> {
    let all = actives(clips, ends, t);
    let over = *all.last()?;
    let under = (all.len() > 1).then(|| all[all.len() - 2]);
    // A fade is clamped to its own shot: a trim can leave one longer than what it runs over.
    let span = ends[over] - clips[over].start;
    let top = over as u32;
    let blank = |from, to, blend| Span {
        at: t,
        until: None,
        top,
        from,
        to,
        blend,
    };

    if let Some(fade) = &clips[over].fade_in {
        let len = js_min(fade.over, span);
        // `under` may be none, and that is a fade from NOTHING: the same branch.
        if len > 0.0 && t < clips[over].start + len {
            let ramp = Ramp {
                start: clips[over].start,
                len,
                rising: true,
            };
            let into = chain_of(fade).map(|c| Stretch {
                chain: c.to_owned(),
                presence: ramp.clone(),
            });
            let blend = Blend {
                name: fade.with_.clone(),
                progress: ramp,
                stretch_from: under.and_then(|u| leaving(clips, ends, u, t)),
                stretch_to: into,
            };
            return Some(blank(under.map(|u| u as u32), Some(top), Some(blend)));
        }
    }

    // An open-ended shot has no end to measure back from; the schema refuses that pair, this survives it.
    if let Some(fade) = &clips[over].fade_out
        && ends[over].is_finite()
    {
        let len = js_min(fade.over, span);
        let from = ends[over] - len;
        if len > 0.0 && t >= from {
            let blend = Blend {
                name: fade.with_.clone(),
                progress: Ramp {
                    start: from,
                    len,
                    rising: true,
                },
                stretch_from: leaving(clips, ends, over, t),
                stretch_to: None,
            };
            return Some(blank(Some(top), None, Some(blend)));
        }
    }
    Some(blank(None, Some(top), None))
}

/// Every time at which some clip's presence or some fade's window opens or closes.
fn breakpoints(clips: &[SceneClip], ends: &[f64]) -> Vec<f64> {
    let mut at = Vec::new();
    for (i, c) in clips.iter().enumerate() {
        at.push(c.start);
        at.push(ends[i]);
        let span = ends[i] - c.start;
        if let Some(f) = &c.fade_in {
            at.push(c.start + js_min(f.over, span));
        }
        if let Some(f) = &c.fade_out {
            at.push(ends[i] - js_min(f.over, span));
        }
    }
    at.retain(|t| t.is_finite());
    at.sort_by(f64::total_cmp);
    at.dedup();
    at
}

/// The schedule of a layer's clips, in file order.
pub fn schedule(clips: &[SceneClip]) -> LayerSchedule {
    let ends: Vec<f64> = (0..clips.len()).map(|i| end_of(clips, i)).collect();
    let cuts = breakpoints(clips, &ends);
    let mut spans: Vec<Span> = Vec::new();
    for (k, &b) in cuts.iter().enumerate() {
        // Every comparison the rule makes is against a breakpoint, so the shape at `b` is the shape until the next.
        let Some(mut s) = span_for(clips, &ends, b) else {
            continue;
        };
        s.until = cuts.get(k + 1).copied();
        match spans.last_mut() {
            Some(prev) if prev.until == Some(b) && same_shape(prev, &s) => prev.until = s.until,
            _ => spans.push(s),
        }
    }
    LayerSchedule {
        ends: ends.iter().map(|e| e.is_finite().then_some(*e)).collect(),
        spans,
    }
}

fn same_shape(a: &Span, b: &Span) -> bool {
    (a.top, a.from, a.to, &a.blend) == (b.top, b.from, b.to, &b.blend)
}
