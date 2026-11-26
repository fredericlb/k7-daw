//! decode.rs — a mix decoded into the shape the model wants, and a stem taken back to the record's rate, through
//! the media library's own decoder and resampler (the `waveform` crate): no ffmpeg. Blocks land straight in the
//! per-channel buffers, so no interleaved copy of the whole song is ever held beside them. Android has no
//! separation: both calls fail with a sentence saying so.

use std::path::Path;

use super::audio::Channels;
#[cfg(not(target_os = "android"))]
use super::audio::{CHANNELS, SAMPLE_RATE};

pub struct Decoded {
    /// `CHANNELS` channels at `SAMPLE_RATE`, whatever the file holds.
    pub channels: Channels,
    /// The file's own rate, which the stems are written back at.
    pub source_rate: u32,
}

#[cfg(not(target_os = "android"))]
mod real {
    use super::*;
    use waveform::decode::Decoder;
    use waveform::resample::Resample;

    /// Frames asked of the decoder per block: about a second of a 44.1 kHz file.
    const BLOCK: usize = 44_100;

    /// `frames` from `from` channels to `to`, as the media library's encoder maps them: kept, the first two averaged, one doubled, or the first two kept.
    pub(crate) fn map_channels(frames: &[f32], from: usize, to: usize, out: &mut Vec<f32>) {
        out.clear();
        if from == to {
            out.extend_from_slice(frames);
            return;
        }
        for frame in frames.chunks_exact(from) {
            let pair = &frame[..from.min(2)];
            let mono = pair.iter().sum::<f32>() / pair.len() as f32;
            match to {
                1 => out.push(mono),
                _ if from == 1 => out.extend(std::iter::repeat_n(mono, to)),
                _ => {
                    out.extend(frame.iter().take(to));
                    out.extend(std::iter::repeat_n(0.0, to.saturating_sub(from)));
                }
            }
        }
    }

    /// Interleaved `frames` appended to per-channel buffers.
    fn append(out: &mut Channels, frames: &[f32]) {
        let n = out.len();
        for (c, ch) in out.iter_mut().enumerate() {
            ch.extend(frames.iter().skip(c).step_by(n));
        }
    }

    pub fn decode_for_model(path: &Path) -> Result<Decoded, String> {
        let shown = path.display();
        let file = std::fs::File::open(path).map_err(|e| format!("cannot open {shown}: {e}"))?;
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let mut decoder =
            Decoder::open(Box::new(file), ext).map_err(|e| format!("{shown}: {e}"))?;
        let source_rate = decoder.sample_rate;
        let mut resample = (source_rate != SAMPLE_RATE)
            .then(|| Resample::new(source_rate, SAMPLE_RATE, CHANNELS))
            .transpose()?;
        let mut channels: Channels = vec![Vec::new(); CHANNELS];
        let (mut raw, mut mapped, mut out) = (Vec::new(), Vec::new(), Vec::new());
        loop {
            raw.clear();
            let ended = decoder.next(&mut raw, BLOCK)? == 0;
            out.clear();
            if ended {
                if let Some(r) = &mut resample {
                    r.finish(&mut out)?;
                }
            } else {
                map_channels(&raw, decoder.channels, CHANNELS, &mut mapped);
                match &mut resample {
                    None => std::mem::swap(&mut out, &mut mapped),
                    Some(r) => r.push(&mapped, &mut out)?,
                }
            }
            append(&mut channels, &out);
            if ended {
                break;
            }
        }
        channels.iter_mut().for_each(Vec::shrink_to_fit);
        Ok(Decoded {
            channels,
            source_rate,
        })
    }

    pub fn resample(chans: &Channels, from: u32, to: u32) -> Result<Channels, String> {
        const BLOCK: usize = 65_536;
        let n = chans.len();
        let frames = chans.first().map_or(0, Vec::len);
        let mut r = Resample::new(from, to, n)?;
        let mut out: Channels =
            vec![
                Vec::with_capacity((frames as f64 * f64::from(to) / f64::from(from)) as usize + 1);
                n
            ];
        let (mut block, mut got) = (Vec::new(), Vec::new());
        for at in (0..frames).step_by(BLOCK) {
            let len = BLOCK.min(frames - at);
            block.clear();
            for i in at..at + len {
                block.extend(chans.iter().map(|ch| ch[i]));
            }
            got.clear();
            r.push(&block, &mut got)?;
            append(&mut out, &got);
        }
        got.clear();
        r.finish(&mut got)?;
        append(&mut out, &got);
        Ok(out)
    }
}

#[cfg(not(target_os = "android"))]
pub use real::{decode_for_model, resample};

/// Decodes `path`, failing HARD: there is no lesser result to fall back to.
#[cfg(target_os = "android")]
pub fn decode_for_model(_path: &Path) -> Result<Decoded, String> {
    Err("this build has no stem separation: no audio decoder is built for this platform".into())
}

/// `chans` from `from` Hz to `to` Hz.
#[cfg(target_os = "android")]
pub fn resample(_chans: &Channels, _from: u32, _to: u32) -> Result<Channels, String> {
    Err("this build has no stem separation: no resampler is built for this platform".into())
}
