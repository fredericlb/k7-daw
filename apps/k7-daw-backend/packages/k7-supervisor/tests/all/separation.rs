//! separation.rs — the behaviours `SeparationAudio_test`, `SeparationBudget_test`, `SeparationDecode_test` and
//! `JobSlots_test` held: the windows cover the file and end on its end, overlap-add gives back what it was fed,
//! the WAV is 24-bit PCM that clamps, a phone's budget and refusal, a decode comes back stereo at the model's
//! rate, and the cap holds however many threads ask. No model and no runtime in any of them.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use k7_supervisor::separation::audio::*;
use k7_supervisor::separation::budget::{bytes_needed, memory_refusal, thread_count};
use k7_supervisor::separation::decode::{decode_for_model, resample};
use k7_supervisor::separation::onnx::locate;
use k7_supervisor::separation::slots::JobSlots;

use crate::scratch::Scratch;

pub fn sine(rate: u32, frames: usize) -> Vec<f32> {
    (0..frames)
        .map(|i| {
            0.5 * (2.0 * std::f64::consts::PI * 440.0 * i as f64 / f64::from(rate)).sin() as f32
        })
        .collect()
}

fn sample_at(b: &[u8], i: usize) -> i32 {
    let o = 44 + i * 3;
    let u = u32::from(b[o]) | u32::from(b[o + 1]) << 8 | u32::from(b[o + 2]) << 16;
    if u & 0x80_0000 != 0 {
        u as i32 - 0x100_0000
    } else {
        u as i32
    }
}

#[test]
fn one_window_for_a_short_file() {
    assert_eq!(window_starts(1000), [0]);
    assert_eq!(window_starts(SEGMENT), [0]);
}

#[test]
fn the_last_window_ends_on_the_end() {
    let len = SEGMENT * 3 + 12345;
    let starts = window_starts(len);
    assert_eq!(starts.last().unwrap() + SEGMENT, len);
    assert_eq!(starts[0], 0);
    for w in starts.windows(2) {
        assert!(w[1] <= w[0] + SEGMENT, "a gap between windows");
    }
}

#[test]
fn the_triangle_is_never_zero() {
    let w = triangle(101);
    assert!(w.iter().all(|v| *v > 0.0));
    assert!((w[50] - 1.0).abs() < 1e-5);
}

#[test]
fn overlap_add_reconstructs() {
    let len = SEGMENT * 2 + 5000;
    let source: Vec<f32> = (0..len).map(|i| (i as f32 / 500.0).sin()).collect();
    let mut oa = OverlapAdd::new(len, 1);
    for at in window_starts(len) {
        oa.add(at, 0, 0, &source[at..(at + SEGMENT).min(len)]);
        oa.count_window(at);
    }
    let out = &oa.finish(0)[0];
    let worst = out
        .iter()
        .zip(&source)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f32::max);
    assert!(worst < 1e-5, "{worst}");
}

#[test]
fn each_stem_keeps_its_own() {
    let mut oa = OverlapAdd::new(SEGMENT, STEMS.len());
    oa.add(0, 2, 0, &vec![1.0; SEGMENT]);
    oa.count_window(0);
    assert!((oa.finish(2)[0][SEGMENT >> 1] - 1.0).abs() < 1e-5);
    assert_eq!(oa.finish(0)[0][SEGMENT >> 1], 0.0);
}

#[test]
fn the_wav_is_24_bit_pcm() {
    let b = wav_pcm24(&[vec![0.0, 0.5, -0.5], vec![1.0, -1.0, 0.0]], SAMPLE_RATE);
    assert_eq!(&b[..4], b"RIFF");
    assert_eq!(&b[8..12], b"WAVE");
    assert_eq!(b.len(), 44 + 3 * 2 * 3);
    assert_eq!(b[34], 24);
    assert_eq!(sample_at(&b, 1), 0x7f_ffff);
    assert_eq!(sample_at(&b, 3), -0x7f_ffff);
}

#[test]
fn an_overshoot_is_clamped() {
    let b = wav_pcm24(&[vec![2.0, -2.0]], SAMPLE_RATE);
    assert_eq!(sample_at(&b, 0), 0x7f_ffff);
    assert_eq!(sample_at(&b, 1), -0x80_0000);
}

#[test]
fn the_instrumental_is_everything_but_the_voice() {
    let mut oa = OverlapAdd::new(SEGMENT, STEMS.len());
    for s in 0..STEMS.len() {
        for c in 0..2 {
            oa.add(0, s, c, &vec![1.0; SEGMENT]);
        }
    }
    oa.count_window(0);
    assert!((output_channels(&oa, INSTRUMENTAL)[0][SEGMENT >> 1] - 3.0).abs() < 1e-4);
    assert!((peak_of(&output_channels(&oa, "vocals")) - 1.0).abs() < 1e-4);
}

#[test]
fn a_phone_takes_half_its_cores() {
    assert_eq!(thread_count(6, true), 3);
    assert_eq!(thread_count(1, true), 1);
    assert_eq!(thread_count(6, false), 1);
    assert_eq!(thread_count(16, false), 8);
}

#[test]
fn a_cut_needs_more_for_a_longer_song() {
    assert!(bytes_needed(160 << 20, 44_100 * 240) > bytes_needed(160 << 20, 44_100 * 10));
    assert!(bytes_needed(160 << 20, 0) > 320 << 20);
}

#[test]
fn a_cut_is_refused_when_memory_falls_short() {
    let need = bytes_needed(160 << 20, 44_100 * 240);
    assert!(memory_refusal(need, need, 0).is_none());
    assert!(memory_refusal(need + 1, need, 0).is_none());
    let why = memory_refusal(need - 1, need, 0).expect("refused");
    assert!(why.contains("MB"));
}

#[test]
fn a_raised_floor_refuses_what_fits() {
    assert!(memory_refusal(2 << 30, 1 << 20, 0).is_none());
    assert!(memory_refusal(2 << 30, 1 << 20, 4 << 30).is_some());
}

#[test]
fn a_mono_file_comes_back_on_both_sides() {
    let dir = Scratch::new("mono");
    let tone = sine(SAMPLE_RATE, 5000);
    let path = dir.path().join("mono.wav");
    std::fs::write(&path, wav_pcm24(std::slice::from_ref(&tone), SAMPLE_RATE)).unwrap();
    let d = decode_for_model(&path).unwrap();
    assert_eq!(d.source_rate, SAMPLE_RATE);
    assert_eq!(d.channels.len(), 2);
    assert_eq!(d.channels[0].len(), tone.len());
    assert_eq!(d.channels[1], d.channels[0]);
    let worst = d.channels[0]
        .iter()
        .zip(&tone)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f32::max);
    assert!(worst < 1e-6, "{worst}");
}

#[test]
fn a_file_at_another_rate_reaches_the_models_and_says_its_own() {
    let dir = Scratch::new("48k");
    let tone = sine(48_000, 48_000);
    let path = dir.path().join("48k.wav");
    std::fs::write(&path, wav_pcm24(&[tone.clone(), tone], 48_000)).unwrap();
    let d = decode_for_model(&path).unwrap();
    assert_eq!(d.source_rate, 48_000);
    assert_eq!(d.channels.len(), 2);
    assert_eq!(d.channels[0].len(), 44_100);
    let want = sine(SAMPLE_RATE, 44_100);
    let worst = (5000..40_000)
        .map(|i| (d.channels[1][i] - want[i]).abs())
        .fold(0.0, f32::max);
    assert!(worst < 1e-3, "{worst}");
}

#[test]
fn a_stem_goes_back_at_the_ratios_length() {
    let tone = sine(SAMPLE_RATE, 100_000);
    let out = resample(&vec![tone.clone(), tone], SAMPLE_RATE, 48_000).unwrap();
    assert_eq!(out.len(), 2);
    assert_eq!(
        out[0].len(),
        (100_000.0 * 48_000.0 / f64::from(SAMPLE_RATE)).ceil() as usize
    );
    let want = sine(48_000, out[0].len());
    let worst = (5000..90_000)
        .map(|i| (out[0][i] - want[i]).abs())
        .fold(0.0, f32::max);
    assert!(worst < 1e-3, "{worst}");
}

#[test]
fn what_is_not_audio_fails_with_a_reason() {
    let dir = Scratch::new("junk");
    let text = dir.path().join("text.wav");
    std::fs::write(&text, "this is not a wav file at all").unwrap();
    assert!(!decode_for_model(&text).err().expect("fails").is_empty());
    assert!(decode_for_model(&dir.path().join("absent.mp3")).is_err());
}

#[test]
fn never_more_than_the_cap_run() {
    let slots = JobSlots::new(2);
    let (now, most) = (AtomicUsize::new(0), AtomicUsize::new(0));
    std::thread::scope(|s| {
        for _ in 0..8 {
            s.spawn(|| {
                let _held = slots.hold(|_| {});
                let n = now.fetch_add(1, Ordering::SeqCst) + 1;
                most.fetch_max(n, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(5));
                now.fetch_sub(1, Ordering::SeqCst);
            });
        }
    });
    assert_eq!(most.load(Ordering::SeqCst), 2);
    assert_eq!(slots.running(), 0);
}

#[test]
fn a_zero_cap_still_runs_one() {
    let slots = JobSlots::new(0);
    let _held = slots.hold(|_| {});
    assert_eq!(slots.running(), 1);
}

#[test]
fn waiters_are_served_in_order_and_hear_what_is_ahead() {
    let slots = JobSlots::new(1);
    let order = Mutex::new(Vec::new());
    let ahead = Mutex::new(vec![usize::MAX; 3]);
    std::thread::scope(|s| {
        let held = slots.hold(|_| {});
        for i in 0..3 {
            let (slots, order, ahead) = (&slots, &order, &ahead);
            s.spawn(move || {
                let _mine = slots.hold(|n| {
                    let mut a = ahead.lock().unwrap();
                    if a[i] == usize::MAX {
                        a[i] = n;
                    }
                });
                order.lock().unwrap().push(i);
            });
            // Each waiter takes its ticket before the next one starts.
            while ahead.lock().unwrap()[i] == usize::MAX {
                std::thread::sleep(Duration::from_millis(2));
            }
        }
        drop(held);
    });
    assert_eq!(*order.lock().unwrap(), [0, 1, 2]);
    assert_eq!(*ahead.lock().unwrap(), [0, 1, 2]);
}

#[test]
fn the_runtime_is_looked_for_in_the_order_a_package_lays_it_out() {
    let exe = std::path::Path::new("/app/bin/k7-supervisor");
    let dirs = locate::candidate_dirs(Some("/opt/ort"), Some(exe));
    assert_eq!(dirs[0], std::path::Path::new("/opt/ort"));
    assert_eq!(dirs[1], std::path::Path::new("/app/bin"));
    assert_eq!(dirs[2], std::path::Path::new("/app/bin/onnxruntime"));
    assert_eq!(
        dirs[3],
        std::path::Path::new("/app/bin/resources/onnxruntime")
    );
    assert_eq!(dirs.len(), 5);
    assert_eq!(locate::candidate_dirs(Some(""), None).len(), 0);
}

#[test]
fn the_library_is_found_whatever_its_version_suffix() {
    let dir = Scratch::new("lib");
    assert!(locate::library_in(dir.path()).is_none());
    let name = if cfg!(windows) {
        "onnxruntime.dll"
    } else if cfg!(target_vendor = "apple") {
        "libonnxruntime.1.30.0.dylib"
    } else {
        "libonnxruntime.so.1.30.0"
    };
    std::fs::write(dir.path().join("libother.txt"), "").unwrap();
    std::fs::write(dir.path().join(name), "").unwrap();
    assert_eq!(locate::library_in(dir.path()), Some(dir.path().join(name)));
}
