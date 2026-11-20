//! region.rs — a file's envelope sliced to the region a clip plays, and the bar and beat grid over it (2420, 2423): `waveform.ts`
//! from `regionWaveform` on, with no I/O. Numbers are f64 throughout and every `Math.*` keeps JavaScript's NaN and rounding.
//! What a reader gets wrong: `env.frames | 0` is ToInt32, so a file of 2^31 frames or more reads as empty, as the TypeScript does.

use song::clips::{ClipDoc, js_number};

use super::MAX_GRID_LINES;
use super::warp::{Marker, WarpMap};

/// A sidecar's envelope: the arrays and the numbers the slicing reads; the fingerprint and the sensitivity are the answer's.
#[derive(Clone, Copy, Debug)]
pub struct Envelope<'a> {
    pub sample_rate: f64,
    pub frames: f64,
    pub seconds: f64,
    pub peak: &'a [f64],
    pub rms: &'a [f64],
    pub bands: Option<[&'a [f64]; 3]>,
    pub transients: &'a [f64],
}

/// A warp marker in display seconds: where its source is drawn, where its beat lands, and the file second it pins.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WarpMark {
    pub orig: f64,
    pub target: f64,
    pub src: f64,
}

/// The numbers `to_display` is, for a caller going back across a wire.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SourceMap {
    pub in_point: f64,
    pub stretch: f64,
    pub zero_beat: f64,
}

#[derive(Clone, Debug)]
pub struct Region {
    pub peak: Vec<f64>,
    pub rms: Vec<f64>,
    pub bands: Option<[Vec<f64>; 3]>,
    pub seconds: f64,
    pub start_beat: f64,
    pub clip_beats: f64,
    pub bar_beats: f64,
    pub tempo: f64,
    pub warp_marks: Vec<WarpMark>,
    pub transients: Vec<f64>,
    pub source_map: SourceMap,
    sample_rate: f64,
    map: Option<WarpMap>,
    beat_to_disp: f64,
}

/// One bar or beat line, `t` in seconds from the region's start; `label` only on a bar.
#[derive(Clone, Debug, PartialEq)]
pub struct GridLine {
    pub t: f64,
    pub bar: bool,
    pub label: Option<String>,
}

/// A grid of more than `MAX_GRID_LINES` lines, which the route answers `'<clip>' is too long to draw` (2423).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TooLong;

// `Math.max` and `Math.min` answer NaN when either side is.
pub(super) fn jmax(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.max(b)
    }
}

pub(super) fn jmin(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.min(b)
    }
}

/// `Math.round`: halves go up, so -2.5 is -2.
fn jround(x: f64) -> f64 {
    if !x.is_finite() {
        return x;
    }
    let f = x.floor();
    if x - f >= 0.5 { f + 1.0 } else { f }
}

/// `x | 0`: ToInt32, the value modulo 2^32 read as signed; a non-finite number is 0.
fn to_int32(x: f64) -> f64 {
    if !x.is_finite() {
        return 0.0;
    }
    let m = x.trunc().rem_euclid(4_294_967_296.0);
    if m >= 2_147_483_648.0 {
        m - 4_294_967_296.0
    } else {
        m
    }
}

/// `a || b` on numbers: zero and NaN are falsy.
fn or(a: f64, b: f64) -> f64 {
    if a == 0.0 || a.is_nan() { b } else { a }
}

/// How many timeline seconds one source second occupies, 1 unwarped (`warpStretch`).
fn warp_stretch(markers: Option<&[Marker]>, tempo: f64) -> f64 {
    let Some(m) = markers.filter(|m| m.len() >= 2 && tempo > 0.0) else {
        return 1.0;
    };
    let beat_span = m[m.len() - 1].0 - m[0].0;
    let src_span = m[m.len() - 1].1 - m[0].1;
    if src_span.is_nan() || src_span <= 0.0 || beat_span.is_nan() || beat_span <= 0.0 {
        return 1.0;
    }
    (beat_span * 60.0) / tempo / src_span
}

/// `regionWaveform`: the envelope resampled to the `buckets` columns of the clip's region (2420).
pub fn region_waveform(
    env: &Envelope<'_>,
    clip: &ClipDoc,
    tempo: f64,
    num: f64,
    den: f64,
    buckets: usize,
) -> Region {
    let sr = or(env.sample_rate, 48000.0);
    let total_frames = jmax(0.0, to_int32(env.frames));
    let file_seconds = or(env.seconds, total_frames / sr);
    let tempo = or(tempo, 120.0);
    let bar_beats = jmax(1.0, or(num, 4.0) * (4.0 / or(den, 4.0)));
    let offset_sec = jmax(0.0, or(clip.offset, 0.0));
    let in_point = jmax(0.0, clip.source_from.unwrap_or(0.0) + offset_sec);
    let seconds = if clip.length > 0.0 {
        (clip.length * 60.0) / tempo
    } else {
        jmax(0.0, file_seconds - in_point)
    };
    let clip_beats = if clip.length > 0.0 {
        clip.length
    } else {
        (seconds * tempo) / 60.0
    };
    let markers: Option<&[Marker]> = clip.warp.as_ref().map(|w| w.markers.as_slice());
    let stretch = warp_stretch(markers, tempo);
    let map = markers.filter(|m| m.len() >= 2).map(WarpMap::new);
    let beat_to_disp = 60.0 / tempo;
    let zero_beat = map.as_ref().map_or(0.0, |m| m.beat_at_source(in_point));
    let mut region = Region {
        peak: Vec::new(),
        rms: Vec::new(),
        bands: None,
        seconds,
        start_beat: clip.start,
        clip_beats,
        bar_beats,
        tempo,
        warp_marks: Vec::new(),
        transients: Vec::new(),
        source_map: SourceMap {
            in_point,
            stretch,
            zero_beat,
        },
        sample_rate: sr,
        map,
        beat_to_disp,
    };
    let env_n = env.peak.len();
    let start_frame = jmin(total_frames, (in_point * sr).floor());
    let end_sec = match clip.source_to {
        Some(to) if to != 0.0 && !to.is_nan() && to > in_point => jmin(to, file_seconds),
        _ => file_seconds,
    };
    let want_frames = jmax(1.0, jround((seconds / stretch) * sr));
    // A wanted count that is NaN (a NaN in-point, or a degenerate warp) is the TypeScript's `RangeError`; one silent column stands for it (2428).
    let n = jmax(1.0, jmin(buckets as f64, want_frames));
    let n = if n.is_nan() { 1 } else { n as usize };
    let bands_on = env.bands.filter(|b| b.iter().all(|a| a.len() == env_n));
    let mut peak = vec![0.0; n];
    let mut rms = vec![0.0; n];
    let mut bands = bands_on.map(|_| [vec![0.0; n], vec![0.0; n], vec![0.0; n]]);
    let disp_per = seconds / n as f64;
    let frames_per_env = if env_n > 0 {
        total_frames / env_n as f64
    } else {
        1.0
    };
    let end_frame = jmin(total_frames, (end_sec * sr).floor());
    for b in 0..n {
        let f0 = jmax(
            start_frame,
            (region.to_source(b as f64 * disp_per) * sr).floor(),
        );
        let f1 = jmin(
            end_frame,
            (region.to_source((b + 1) as f64 * disp_per) * sr).floor(),
        );
        if f1 <= f0 || env_n == 0 {
            continue;
        }
        let e0 = (f0 / frames_per_env).floor();
        let e1 = jmax(e0 + 1.0, jmin(env_n as f64, (f1 / frames_per_env).ceil()));
        let (mut mx, mut sq, mut cnt) = (0.0_f64, 0.0_f64, 0.0_f64);
        let mut e = e0;
        while e < e1 && e < env_n as f64 {
            let i = e as usize;
            if env.peak[i] > mx {
                mx = env.peak[i];
            }
            let r = env.rms.get(i).copied().unwrap_or(f64::NAN);
            sq += r * r;
            cnt += 1.0;
            if let (Some(out), Some(src)) = (bands.as_mut(), bands_on) {
                for k in 0..3 {
                    if src[k][i] > out[k][b] {
                        out[k][b] = src[k][i];
                    }
                }
            }
            e += 1.0;
        }
        peak[b] = mx;
        rms[b] = if cnt != 0.0 { (sq / cnt).sqrt() } else { 0.0 };
    }
    region.peak = peak;
    region.rms = rms;
    region.bands = bands;
    region.warp_marks = markers
        .unwrap_or(&[])
        .iter()
        .map(|&(beat, src)| WarpMark {
            orig: region.to_display(src),
            target: (beat - zero_beat) * beat_to_disp,
            src,
        })
        .filter(|mk| {
            mk.orig.is_finite()
                && mk.target.is_finite()
                && ((mk.orig >= -1e-6 && mk.orig <= seconds + 1e-6)
                    || (mk.target >= -1e-6 && mk.target <= seconds + 1e-6))
        })
        .collect();
    region.transients = region.display_transients(env.transients);
    region
}

impl Region {
    /// A file second as a position on the drawn region: the one mapping everything uses.
    pub fn to_display(&self, file_sec: f64) -> f64 {
        match &self.map {
            Some(m) => (m.beat_at_source(file_sec) - self.source_map.zero_beat) * self.beat_to_disp,
            None => (file_sec - self.source_map.in_point) * self.source_map.stretch,
        }
    }

    /// A display second back to the file second it shows, which is what resamples the envelope.
    fn to_source(&self, disp: f64) -> f64 {
        match &self.map {
            Some(m) => m.source_at_beat(self.source_map.zero_beat + disp / self.beat_to_disp),
            None => disp / or(self.source_map.stretch, 1.0) + self.source_map.in_point,
        }
    }

    /// File-second onsets as display seconds, those inside the region: the answer's `transients` and `detected`.
    pub fn display_transients(&self, file: &[f64]) -> Vec<f64> {
        file.iter()
            .map(|&t| self.to_display(t))
            .filter(|&t| t >= -1e-6 && t <= self.seconds + 1e-6)
            .collect()
    }

    /// The sidecar's sample rate, 48000 when it carries none.
    pub fn sample_rate(&self) -> f64 {
        self.sample_rate
    }
}

/// `beatGrid`: bar and beat lines aligned to the arrangement; more than `MAX_GRID_LINES` of them is refused (2423).
pub fn beat_grid(r: &Region) -> Result<Vec<GridLine>, TooLong> {
    let mut lines = Vec::new();
    let last = r.start_beat + r.clip_beats;
    let mut beat = (r.start_beat - 1e-9).ceil();
    while beat <= last + 1e-9 {
        if lines.len() >= MAX_GRID_LINES {
            return Err(TooLong);
        }
        let bar = ((beat % r.bar_beats) + r.bar_beats) % r.bar_beats == 0.0;
        lines.push(GridLine {
            t: ((beat - r.start_beat) * 60.0) / r.tempo,
            bar,
            label: bar.then(|| js_number((beat / r.bar_beats).floor() + 1.0)),
        });
        beat += 1.0;
    }
    Ok(lines)
}
