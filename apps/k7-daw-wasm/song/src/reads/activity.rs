// reads/activity.rs — the opacity levels a timeline event block wears along its width: quiet at half, the loudest bucket at full.
// A reader would get wrong that audio is max-pooled and MIDI summed: averaging an envelope would wash a hit out of a quiet bucket.

use super::model::RNote;
use crate::clips::js_number;

/// The floor: a silent stretch still shows the block at half strength.
const QUIET: f64 = 0.5;

/// `Math.round`: halves go up.
fn js_round(x: f64) -> f64 {
    (x + 0.5).floor()
}

/// Raw activity into alphas in `QUIET..=1`, normalised to the loudest bucket.
pub fn to_alphas(raw: &[f64]) -> Vec<f64> {
    let mut m = 0.0;
    for &v in raw {
        if v > m {
            m = v;
        }
    }
    raw.iter()
        .map(|&v| {
            if m > 0.0 {
                QUIET + (1.0 - QUIET) * (v / m)
            } else {
                QUIET
            }
        })
        .collect()
}

/// An envelope's peaks max-pooled into at most `buckets` alphas.
pub fn audio_activity(peak: &[f64], buckets: usize) -> Vec<f64> {
    let n = buckets.min(peak.len());
    if n == 0 {
        return Vec::new();
    }
    let mut out = vec![0.0; n];
    for (i, &p) in peak.iter().enumerate() {
        let b = (i * n / peak.len()).min(n - 1);
        if p > out[b] {
            out[b] = p;
        }
    }
    to_alphas(&out)
}

/// A sequence's activity: each bucket sums the velocities of the notes sounding in it, a looping clip tiling its window's notes.
pub fn midi_activity(notes: &[RNote], span_beats: f64, buckets: usize, loop_start: f64, loop_end: f64) -> Vec<f64> {
    if notes.is_empty() || !(span_beats > 0.0) || buckets == 0 {
        return Vec::new();
    }
    let loop_len = if loop_end > loop_start {
        loop_end - loop_start
    } else {
        0.0
    };
    let placed: Vec<(f64, f64, f64)> = if loop_len > 0.0 {
        let win: Vec<&RNote> = notes
            .iter()
            .filter(|n| n.time >= loop_start && n.time < loop_end)
            .collect();
        let mut out = Vec::new();
        let mut k = 0.0;
        while k * loop_len < span_beats {
            for n in &win {
                out.push((n.time - loop_start + k * loop_len, n.duration, n.vel.unwrap_or(1.0)));
            }
            k += 1.0;
        }
        out
    } else {
        notes
            .iter()
            .map(|n| (n.time, n.duration, n.vel.unwrap_or(1.0)))
            .collect()
    };
    let w = span_beats / buckets as f64;
    let last = buckets - 1;
    let mut raw = vec![0.0; buckets];
    for (time, duration, vel) in placed {
        if time >= span_beats {
            continue;
        }
        let from = (time / w).floor().max(0.0);
        // A zero-length note still sounds in the bucket it sits in.
        let to = (last as f64).min(from.max(((time + duration) / w).ceil() - 1.0));
        let mut b = from;
        while b <= to {
            raw[b as usize] += vel;
            b += 1.0;
        }
    }
    to_alphas(&raw)
}

/// The alphas as a CSS mask gradient: a mask rather than a painted gradient, so the block's fill stays what it was.
pub fn activity_mask(alphas: &[f64]) -> String {
    let Some(&first) = alphas.first() else {
        return String::new();
    };
    let one = |a: f64| format!("rgb(0 0 0 / {})", js_number(js_round(a * 1000.0) / 1000.0));
    if alphas.len() == 1 {
        return format!("linear-gradient({}, {})", one(first), one(first));
    }
    let n = alphas.len() as f64;
    let stops: Vec<String> = alphas
        .iter()
        .enumerate()
        .map(|(i, &a)| {
            format!(
                "{} {}%",
                one(a),
                js_number(js_round((i as f64 + 0.5) / n * 10000.0) / 100.0)
            )
        })
        .collect();
    format!("linear-gradient(to right, {})", stops.join(", "))
}
