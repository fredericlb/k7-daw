//! audio.rs — the arithmetic a stem separation needs, with no model and no disk in it: how a long file is cut
//! into the fixed windows the model accepts, how the overlapping results fold back into one signal, and what a
//! 24-bit WAV looks like. What a reader gets wrong: the weight is per POSITION, counted once per window, not
//! once per stem (`docs/separation.md`).

/// HT-Demucs is trained at 44.1 kHz stereo, and the exported graph has it baked in.
pub const SAMPLE_RATE: u32 = 44_100;
pub const CHANNELS: usize = 2;
/// The model's input is a FIXED `[1, 2, 343980]`: 7.8 s, not a hint.
pub const SEGMENT: usize = 343_980;
/// A quarter of each window repeats the one before: Demucs' own default, and what hides the seam.
pub const HOP: usize = 257_985;

/// The four sources the model returns, IN THE ORDER it returns them.
pub const STEMS: [&str; 4] = ["drums", "bass", "other", "vocals"];
/// Every stem but the voice, summed.
pub const INSTRUMENTAL: &str = "instrumental";

/// One signal per channel.
pub type Channels = Vec<Vec<f32>>;

/// Where each window starts, the last pulled BACK so the tail is real audio.
pub fn window_starts(length: usize) -> Vec<usize> {
    if length <= SEGMENT {
        return vec![0];
    }
    let mut starts: Vec<usize> = (0..)
        .step_by(HOP)
        .take_while(|at| at + SEGMENT < length)
        .collect();
    starts.push(length - SEGMENT);
    starts
}

/// A triangular weight over one window, its ends nudged off zero.
pub fn triangle(n: usize) -> Vec<f32> {
    let half = (n as f64 - 1.0) / 2.0;
    (0..n)
        .map(|i| (1.0 / n as f64).max(1.0 - (i as f64 - half).abs() / half) as f32)
        .collect()
}

/// The accumulator that folds overlapping windows into one signal, per stem and channel.
pub struct OverlapAdd {
    length: usize,
    acc: Vec<Channels>,
    weight: Vec<f32>,
    win: Vec<f32>,
}

impl OverlapAdd {
    pub fn new(length: usize, stems: usize) -> OverlapAdd {
        OverlapAdd {
            length,
            acc: (0..stems)
                .map(|_| vec![vec![0.0; length]; CHANNELS])
                .collect(),
            weight: vec![0.0; length],
            win: triangle(SEGMENT),
        }
    }

    /// Fold one window's output in; `at` is where the window starts in the whole signal.
    pub fn add(&mut self, at: usize, stem: usize, channel: usize, samples: &[f32]) {
        let dst = &mut self.acc[stem][channel];
        let m = samples.len().min(self.length - at);
        for i in 0..m {
            dst[at + i] += samples[i] * self.win[i];
        }
    }

    /// Counted once per window, after its stems are all added: the weight is per POSITION.
    pub fn count_window(&mut self, at: usize) {
        let m = SEGMENT.min(self.length - at);
        for i in 0..m {
            self.weight[at + i] += self.win[i];
        }
    }

    /// One stem's channels, divided through.
    pub fn finish(&self, stem: usize) -> Channels {
        self.acc[stem]
            .iter()
            .map(|ch| {
                ch.iter()
                    .zip(&self.weight)
                    .map(|(&v, &w)| if w > 0.0 { v / w } else { 0.0 })
                    .collect()
            })
            .collect()
    }
}

/// The loudest sample across every channel: what says a stem was CLAMPED.
pub fn peak_of(chans: &[Vec<f32>]) -> f32 {
    chans
        .iter()
        .flatten()
        .fold(0.0, |peak, v| f32::max(peak, v.abs()))
}

/// A 24-bit PCM WAV, header and all: integer because the consumer is a deck.
pub fn wav_pcm24(chans: &[Vec<f32>], sample_rate: u32) -> Vec<u8> {
    const BYTES: u32 = 3;
    const FULL_SCALE: i32 = 0x7f_ffff;
    let frames = chans.first().map_or(0, Vec::len) as u32;
    let n = chans.len() as u32;
    let data = frames * n * BYTES;
    let mut buf = Vec::with_capacity(44 + data as usize);
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&(36 + data).to_le_bytes());
    buf.extend_from_slice(b"WAVEfmt ");
    buf.extend_from_slice(&16u32.to_le_bytes());
    buf.extend_from_slice(&1u16.to_le_bytes());
    buf.extend_from_slice(&(n as u16).to_le_bytes());
    buf.extend_from_slice(&sample_rate.to_le_bytes());
    buf.extend_from_slice(&(sample_rate * n * BYTES).to_le_bytes());
    buf.extend_from_slice(&((n * BYTES) as u16).to_le_bytes());
    buf.extend_from_slice(&24u16.to_le_bytes());
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&data.to_le_bytes());
    for i in 0..frames as usize {
        for ch in chans {
            let scaled = (f64::from(ch[i]) * f64::from(FULL_SCALE)).round();
            let s = scaled.clamp(-8_388_608.0, f64::from(FULL_SCALE)) as i32;
            buf.extend_from_slice(&s.to_le_bytes()[..3]);
        }
    }
    buf
}

/// One output's signal: a stem as the model gives it, or every stem but the voice summed.
pub fn output_channels(oa: &OverlapAdd, name: &str) -> Channels {
    let index = |stem: &str| STEMS.iter().position(|s| *s == stem).unwrap_or(STEMS.len());
    if name != INSTRUMENTAL {
        return oa.finish(index(name));
    }
    // Added one at a time rather than held four deep: a five-minute stem is 105 MB of floats.
    let mut sum: Option<Channels> = None;
    for stem in STEMS.iter().filter(|s| **s != "vocals") {
        let part = oa.finish(index(stem));
        match &mut sum {
            None => sum = Some(part),
            Some(sum) => {
                for (s, p) in sum.iter_mut().zip(&part) {
                    s.iter_mut().zip(p).for_each(|(s, p)| *s += p);
                }
            }
        }
    }
    sum.unwrap_or_default()
}
