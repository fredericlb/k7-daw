//! stream.rs — the LSTR frame: float32 bytes against engine-protocol's own `encodeStreamFrame` (the hex below
//! is its output), int16 against the wire's rounding.

use k7_runtime_server::wire::stream::{CODEC_PCM_F32, CODEC_PCM_S16, Chunk, HEADER_BYTES, encode};

const SAMPLES: [f32; 6] = [0.0, 0.25, -0.5, 1.0, -1.0, 0.123456];

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn float32_is_the_bytes_stream_ts_writes() {
    let chunk = Chunk {
        samples: &SAMPLES,
        frames: 3,
        sample_rate: 48000.0,
    };
    let got = encode(&chunk, 7, 1234.5, CODEC_PCM_F32);
    assert_eq!(hex(&got), STREAM_TS_F32);
}

#[test]
fn int16_clips_and_rounds_half_away_from_zero() {
    let samples = [0.25, -0.5, 1.5, -1.5, 0.123456, -0.0000153];
    let chunk = Chunk {
        samples: &samples,
        frames: 3,
        sample_rate: 44100.0,
    };
    let got = encode(&chunk, u32::MAX, 0.0, CODEC_PCM_S16);
    assert_eq!(got.len(), HEADER_BYTES + 12);
    assert_eq!(got[20], CODEC_PCM_S16);
    assert_eq!(&got[8..12], &44100u32.to_le_bytes());
    assert_eq!(&got[16..20], &u32::MAX.to_le_bytes());
    let values: Vec<i16> = got[HEADER_BYTES..]
        .chunks(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]))
        .collect();
    assert_eq!(values, [8192, -16384, 32767, -32767, 4045, -1]);
}

#[test]
fn an_empty_chunk_is_a_header_alone() {
    let chunk = Chunk {
        samples: &[],
        frames: 0,
        sample_rate: 48000.0,
    };
    assert_eq!(encode(&chunk, 1, 2.0, CODEC_PCM_F32).len(), HEADER_BYTES);
}

/// `encodeStreamFrame({version:1, channels:2, sampleRate:48000, frames:3, seq:7, codec:0, captureMs:1234.5, samples})`.
const STREAM_TS_F32: &str = "5254534c0102000080bb000003000000070000000000000000000000004a9340000000000000803e000000bf0000803f000080bf80d6fc3d";
