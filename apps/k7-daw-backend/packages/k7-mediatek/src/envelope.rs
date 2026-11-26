//! envelope.rs — the JavaScript half of a sidecar's arithmetic: the band followers' coefficients, the
//! onsets (`detectTransients`), the tempo picked from the module's correlations (`detectBpm`).
//! What a reader gets wrong: `Math.exp` and `Math.log2` are V8's own (`k7_js::math`), never the platform's libm.

use k7_js::math::{exp, log2};
use k7_js::number::math_round;

use crate::sidecar::Envelope;

/// One bucket per 256 frames; the tempo's envelope keeps that hop whatever the length.
pub const FRAMES_PER_BUCKET: f64 = 256.0;
const BAND_LOW_HZ: f64 = 200.0;
const BAND_HIGH_HZ: f64 = 4000.0;
const TWO_PI: f64 = std::f64::consts::TAU;
const MIN_BPM: f64 = 60.0;
const MAX_BPM: f64 = 200.0;
const PRIOR_CENTRE_BPM: f64 = 120.0;
const PRIOR_OCTAVES: f64 = 1.0;

/// `followers(sampleRate)`: the two one-pole coefficients the module's measure is handed.
pub fn followers(sample_rate: f64) -> (f64, f64) {
    (
        1.0 - exp((-TWO_PI * BAND_LOW_HZ) / sample_rate),
        1.0 - exp((-TWO_PI * BAND_HIGH_HZ) / sample_rate),
    )
}

/// What the module's measure hands back, each array as the `Float32Array` it is in node.
#[derive(Clone, Debug, Default)]
pub struct Measured {
    pub peak: Vec<f32>,
    pub rms: Vec<f32>,
    pub low: Vec<f32>,
    pub mid: Vec<f32>,
    pub high: Vec<f32>,
    pub tempo: Vec<f32>,
}

/// `detectBpm`'s lag range for an onset envelope of `len` samples at `rate`, or None where it answers 0 at once.
pub fn bpm_lags(len: usize, rate: f64) -> Option<(usize, usize)> {
    if len < 16 || rate <= 0.0 {
        return None;
    }
    let min_lag = ((rate * 60.0) / MAX_BPM).floor();
    let max_lag = ((rate * 60.0) / MIN_BPM).ceil();
    if min_lag < 1.0 || max_lag <= min_lag || max_lag >= len as f64 {
        return None;
    }
    Some((min_lag as usize, max_lag as usize))
}

/// `detectBpm` over the correlations `table` holds for every lag of `[min_lag, max_lag]`; 0 is "none found".
pub fn detect_bpm(table: &[f64], min_lag: usize, max_lag: usize, rate: f64) -> f64 {
    let corr = |lag: usize| table[lag - min_lag];
    let prior = |lag: usize| {
        let bpm = (60.0 * rate) / lag as f64;
        let oct = log2(bpm / PRIOR_CENTRE_BPM) / PRIOR_OCTAVES;
        exp(-0.5 * oct * oct)
    };
    let mut best = 0.0;
    let mut best_lag = 0;
    for lag in min_lag..=max_lag {
        let score = corr(lag) * prior(lag);
        if score > best {
            best = score;
            best_lag = lag;
        }
    }
    if best_lag == 0 || best <= 0.0 {
        return 0.0;
    }
    let mut lag_f = best_lag as f64;
    if best_lag > min_lag && best_lag < max_lag {
        let ym1 = corr(best_lag - 1) * prior(best_lag - 1);
        let yp1 = corr(best_lag + 1) * prior(best_lag + 1);
        let denom = ym1 - 2.0 * best + yp1;
        if denom.abs() > 1e-12 {
            let shift = (0.5 * (ym1 - yp1)) / denom;
            if shift.abs() <= 1.0 {
                lag_f += shift;
            }
        }
    }
    let bpm = (60.0 * rate) / lag_f;
    if !(MIN_BPM..=MAX_BPM).contains(&bpm) {
        return 0.0;
    }
    math_round(bpm * 100.0) / 100.0
}

/// `detectTransients(peak, sensitivity)`: the buckets after a jump above the mean plus `sensitivity` deviations.
pub fn detect_transients(peak: &[f64], sensitivity: f64) -> Vec<usize> {
    if peak.len() < 3 {
        return Vec::new();
    }
    let diffs: Vec<f64> = peak.windows(2).map(|w| w[1] - w[0]).collect();
    let mut sum = 0.0;
    for d in &diffs {
        sum += d;
    }
    let mean = sum / diffs.len() as f64;
    let mut ssq = 0.0;
    for d in &diffs {
        let x = d - mean;
        ssq += x * x;
    }
    let std = (ssq / diffs.len() as f64).sqrt();
    let threshold = mean + sensitivity * std;
    const MIN_GAP: i64 = 3;
    let mut out = Vec::new();
    let mut last = -MIN_GAP;
    for (i, d) in diffs.iter().enumerate() {
        if *d > threshold && i as i64 - last >= MIN_GAP {
            out.push(i + 1);
            last = i as i64 + 1;
        }
    }
    out
}

/// The stream's facts a measure is stamped with.
#[derive(Clone, Copy, Debug)]
pub struct Facts {
    pub sample_rate: f64,
    pub channels: f64,
    pub frames: f64,
}

/// `envelopeFrom`: the sidecar's envelope from a measure, `bpm` already picked from the module's correlations.
pub fn envelope_from(
    m: &Measured,
    facts: Facts,
    fingerprint: &str,
    sensitivity: f64,
    bpm: f64,
) -> Envelope {
    let widen = |xs: &[f32]| xs.iter().map(|&x| f64::from(x)).collect::<Vec<f64>>();
    let peak = widen(&m.peak);
    let seconds = if facts.sample_rate > 0.0 {
        facts.frames / facts.sample_rate
    } else {
        0.0
    };
    let n = peak.len() as f64;
    let transients = detect_transients(&peak, sensitivity)
        .into_iter()
        .map(|i| {
            if n > 0.0 {
                (i as f64 / n) * seconds
            } else {
                0.0
            }
        })
        .collect();
    Envelope {
        fingerprint: fingerprint.to_string(),
        sample_rate: facts.sample_rate,
        channels: facts.channels,
        frames: facts.frames,
        seconds,
        peak,
        rms: widen(&m.rms),
        transients,
        sensitivity,
        bpm,
        low: Some(widen(&m.low)),
        mid: Some(widen(&m.mid)),
        high: Some(widen(&m.high)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_onsets_after_a_jump_and_none_in_silence() {
        assert!(detect_transients(&[0.0, 0.0], 2.0).is_empty());
        assert!(detect_transients(&[0.0; 50], 2.0).is_empty());
        let mut p = vec![0.0; 40];
        p[10] = 1.0;
        p[11] = 1.0;
        p[30] = 1.0;
        assert_eq!(detect_transients(&p, 2.0), vec![10, 30]);
    }

    #[test]
    fn answers_no_tempo_rather_than_a_guess() {
        assert_eq!(bpm_lags(10, 187.5), None);
        assert_eq!(bpm_lags(1000, 0.0), None);
        let (lo, hi) = bpm_lags(1000, 31.25).unwrap();
        assert_eq!((lo, hi), (9, 32));
        assert_eq!(detect_bpm(&vec![0.0; hi - lo + 1], lo, hi, 31.25), 0.0);
    }
}
