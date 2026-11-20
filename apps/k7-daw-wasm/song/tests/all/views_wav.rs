// views_wav.rs — a WAV's length from its head: the layout `wavLayout` finds and the division `wavSeconds` makes.
// A reader would get wrong that the head is all the Rust sees: a header it cannot place answers None, never a guess.

use crate::views_support;

use song::views::wav::{HEAD_BYTES, wav_seconds};
use views_support::{fast, wav};

fn chunk(id: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut v = id.to_vec();
    v.extend((body.len() as u32).to_le_bytes());
    v.extend(body);
    if body.len() % 2 == 1 {
        v.push(0);
    }
    v
}

fn fmt(tag: u16, align: u16, rate: u32) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend(tag.to_le_bytes());
    b.extend(2u16.to_le_bytes());
    b.extend(rate.to_le_bytes());
    b.extend((rate * u32::from(align)).to_le_bytes());
    b.extend(align.to_le_bytes());
    b.extend(24u16.to_le_bytes());
    b
}

fn riff(chunks: &[Vec<u8>]) -> Vec<u8> {
    let mut v = b"RIFF\0\0\0\0WAVE".to_vec();
    for c in chunks {
        v.extend(c);
    }
    v
}

fn data(declared: u32, bytes: usize) -> Vec<u8> {
    let mut v = b"data".to_vec();
    v.extend(declared.to_le_bytes());
    v.resize(8 + bytes, 0);
    v
}

#[test]
fn a_plain_header_is_its_data_over_the_frame_rate() {
    // wav.ts wavLayout and wavLength.ts (610): 1 second of 48 kHz 6-byte frames.
    let w = wav(1.0);
    let s = wav_seconds(&w[..44], w.len() as u64).expect("seconds");
    assert!((s - 1.0).abs() < 1e-12);
}

#[test]
fn the_size_on_disk_caps_the_declared_size() {
    // wavLength.ts: min(declared, on disk); a file cut short is as long as what is there (610).
    let w = wav(1.0);
    let s = wav_seconds(&w[..44], 44 + 48_000 * 3).expect("seconds");
    assert!((s - 0.5).abs() < 1e-12);
}

#[test]
fn a_zero_declared_size_is_zero_seconds() {
    // wavLength.ts: a 0 `data` size is 0 s here, not what is on disk (610).
    let f = riff(&[chunk(b"fmt ", &fmt(1, 6, 48_000)), data(0, 0)]);
    assert_eq!(wav_seconds(&f, 1 << 20), Some(0.0));
}

#[test]
fn every_plain_tag_reads_and_an_odd_one_does_not() {
    // wav.ts isPlain (610): PCM, float, and the extensible tag naming one of them.
    let at = |tag| riff(&[chunk(b"fmt ", &fmt(tag, 6, 48_000)), data(48_000, 48_000)]);
    assert!(wav_seconds(&at(1), 100_000).is_some() && wav_seconds(&at(3), 100_000).is_some());
    assert_eq!(wav_seconds(&at(2), 100_000), None);
    let mut ext = fmt(0xfffe, 6, 48_000);
    ext.resize(26, 0);
    ext[24] = 3;
    let f = riff(&[chunk(b"fmt ", &ext), data(48_000, 48_000)]);
    assert!(wav_seconds(&f, 100_000).is_some());
    ext[24] = 2;
    let f = riff(&[chunk(b"fmt ", &ext), data(48_000, 48_000)]);
    assert_eq!(wav_seconds(&f, 100_000), None);
    // An extensible tag in 25 bytes has no sub-format to read.
    let mut short = fmt(0xfffe, 6, 48_000);
    short.resize(25, 0);
    assert_eq!(wav_seconds(&riff(&[chunk(b"fmt ", &short), data(8, 8)]), 100_000), None);
}

#[test]
fn a_chunk_before_fmt_and_an_odd_sized_chunk_are_walked_over() {
    // wav.ts: a LIST block before `fmt `, an odd size with its pad byte (610).
    let f = riff(&[
        chunk(b"LIST", &[1, 2, 3]),
        chunk(b"fmt ", &fmt(1, 6, 48_000)),
        data(48_000, 48_000),
    ]);
    assert_eq!(wav_seconds(&f, 200_000), Some(1.0 / 6.0));
}

#[test]
fn the_head_the_walk_cannot_place_answers_none() {
    // hostile table (wav_seconds): empty, 11 bytes, RIFF only, a chunk size of 0xFFFFFFFF, short and late fmt, data first.
    assert_eq!(wav_seconds(&[], 0), None);
    assert_eq!(wav_seconds(&[0; 11], 11), None);
    assert_eq!(wav_seconds(b"RIFF\0\0\0\0WAVE", 12), None);
    let huge = riff(&[{
        let mut c = b"fmt ".to_vec();
        c.extend(u32::MAX.to_le_bytes());
        c
    }]);
    assert_eq!(wav_seconds(&huge, 1 << 40), None);
    assert_eq!(
        wav_seconds(&riff(&[chunk(b"fmt ", &[1, 0, 2, 0]), data(8, 8)]), 100),
        None
    );
    let late = riff(&[chunk(b"fmt ", &fmt(1, 6, 48_000))]);
    assert_eq!(wav_seconds(&late[..late.len() - 4], 100), None);
    assert_eq!(
        wav_seconds(&riff(&[data(8, 8), chunk(b"fmt ", &fmt(1, 6, 48_000))]), 100),
        None
    );
}

#[test]
fn a_zero_block_align_or_sample_rate_is_none() {
    // hostile table (wav_seconds): blockAlign 0, a sampleRate of 0 (610).
    assert_eq!(
        wav_seconds(&riff(&[chunk(b"fmt ", &fmt(1, 0, 48_000)), data(8, 8)]), 100),
        None
    );
    assert_eq!(
        wav_seconds(&riff(&[chunk(b"fmt ", &fmt(1, 6, 0)), data(8, 8)]), 100),
        None
    );
}

#[test]
fn a_walk_of_a_thousand_one_byte_chunks_and_an_odd_pad_at_the_end_of_the_head() {
    // hostile table (wav_seconds): the walk stops at the end of the buffer; a file size of 2^40 does not overflow.
    let mut chunks: Vec<Vec<u8>> = (0..1000).map(|_| chunk(b"junk", &[7])).collect();
    chunks.push(chunk(b"fmt ", &fmt(1, 6, 48_000)));
    chunks.push(data(u32::MAX, 0));
    let f = riff(&chunks);
    let s = fast("a thousand chunks", || wav_seconds(&f, 1 << 40));
    assert!(s.is_some_and(|v| v > 0.0 && v.is_finite()));
    let mut cut = riff(&[chunk(b"junk", &[1, 2, 3])]);
    cut.truncate(cut.len() - 1);
    assert_eq!(wav_seconds(&cut, 100), None);
    assert_eq!(HEAD_BYTES, 8192);
}

#[test]
fn an_id_is_read_with_its_high_bit_dropped_as_nodes_ascii_decode_does() {
    // wavLayout compares `toString("ascii")`, which masks bit 7 of each byte (610).
    let mut w = wav(1.0);
    let plain = wav_seconds(&w[..44], w.len() as u64).expect("seconds");
    for at in [0, 8, 12, 36] {
        w[at] |= 0x80;
    }
    assert_eq!(wav_seconds(&w[..44], w.len() as u64), Some(plain));
    // A byte that differs in its low seven bits is still another id.
    w[1] = b'X';
    assert_eq!(wav_seconds(&w[..44], w.len() as u64), None);
}
