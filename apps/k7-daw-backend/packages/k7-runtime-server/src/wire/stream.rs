//! stream.rs — the audio broadcast's binary frame, "LSTR": a 32-byte header, then interleaved samples as
//! little-endian float32 or int16. The reader is engine-protocol's `stream.ts`; both ends agree on bytes.
//! What a reader gets wrong: `captureMs` is the engine's steady clock since its boot, a difference and not a date.

/// "LSTR" read as a little-endian u32, so a binary frame from anything else is refused.
pub const MAGIC: u32 = 0x4c53_5452;
pub const VERSION: u8 = 1;
pub const HEADER_BYTES: usize = 32;
/// Interleaved float32, the default.
pub const CODEC_PCM_F32: u8 = 0;
/// Interleaved int16 at half the bytes, for a listener that asked `codec: "pcm16"`; 1 is kept for Opus.
pub const CODEC_PCM_S16: u8 = 2;
/// The broadcast is always a stereo pair (EngineHost::broadcastChannels).
pub const CHANNELS: u8 = 2;

/// One drained run of the master tap: `frames * CHANNELS` samples, interleaved.
pub struct Chunk<'a> {
    pub samples: &'a [f32],
    pub frames: usize,
    pub sample_rate: f64,
}

/// The frame's bytes; an int16 sample is clipped to full scale and rounded half away from zero, as `lround` did.
pub fn encode(chunk: &Chunk<'_>, seq: u32, capture_ms: f64, codec: u8) -> Vec<u8> {
    let count = chunk.frames * CHANNELS as usize;
    let samples = &chunk.samples[..count.min(chunk.samples.len())];
    let width = if codec == CODEC_PCM_S16 { 2 } else { 4 };
    let mut out = Vec::with_capacity(HEADER_BYTES + samples.len() * width);
    out.extend_from_slice(&MAGIC.to_le_bytes());
    out.push(VERSION);
    out.push(CHANNELS);
    out.extend_from_slice(&[0, 0]);
    out.extend_from_slice(&(chunk.sample_rate as u32).to_le_bytes());
    out.extend_from_slice(&(chunk.frames as u32).to_le_bytes());
    out.extend_from_slice(&seq.to_le_bytes());
    out.push(codec);
    out.extend_from_slice(&[0, 0, 0]);
    out.extend_from_slice(&capture_ms.to_le_bytes());
    if codec == CODEC_PCM_S16 {
        for &x in samples {
            let v = (x.clamp(-1.0, 1.0) * 32767.0).round() as i16;
            out.extend_from_slice(&v.to_le_bytes());
        }
    } else {
        for &x in samples {
            out.extend_from_slice(&x.to_le_bytes());
        }
    }
    out
}
