// views_support/mod.rs — what the views' tests share: a ClipDoc from JSON, a WAV image, a bound for a loaded container.
// A reader would get wrong the bound: thirty seconds on musl, whose allocator is far slower than any machine k7 runs on.

#![allow(dead_code)]

use serde_json::{Value, json};
use song::clips::ClipDoc;
use song::source::SourceError;
use song::views::lengths::Measure;
use std::collections::HashMap;
use std::time::{Duration, Instant};

pub fn bound() -> Duration {
    Duration::from_secs(if cfg!(target_env = "musl") { 60 } else { 15 })
}

/// Runs `f` and fails when it took longer than the bound.
pub fn fast<T>(what: &str, f: impl FnOnce() -> T) -> T {
    let t = Instant::now();
    let out = f();
    assert!(t.elapsed() < bound(), "{what} took {:?}", t.elapsed());
    out
}

/// A compiled clip with every required field, `over` merged on top.
pub fn clip(over: Value) -> ClipDoc {
    let mut base = json!({
        "id": "c@0", "type": "audio", "name": "c", "source": "", "start": 0, "length": 0, "offset": 0,
        "fadeIn": 0, "fadeOut": 0, "loopStart": 0, "loopEnd": 0, "notes": []
    });
    if let (Some(b), Some(o)) = (base.as_object_mut(), over.as_object()) {
        for (k, v) in o {
            b.insert(k.clone(), v.clone());
        }
    }
    serde_json::from_value(base).expect("a clip")
}

/// A RIFF/WAVE image: 48 kHz, 2 channels, 24 bits, `seconds` long.
pub fn wav(seconds: f64) -> Vec<u8> {
    let (rate, frame) = (48_000u32, 6u32);
    let data = (f64::from(rate) * seconds).round() as u32 * frame;
    let mut h = Vec::new();
    h.extend(b"RIFF");
    h.extend((36 + data).to_le_bytes());
    h.extend(b"WAVEfmt ");
    h.extend(16u32.to_le_bytes());
    h.extend(1u16.to_le_bytes());
    h.extend(2u16.to_le_bytes());
    h.extend(rate.to_le_bytes());
    h.extend((rate * frame).to_le_bytes());
    h.extend((frame as u16).to_le_bytes());
    h.extend(24u16.to_le_bytes());
    h.extend(b"data");
    h.extend(data.to_le_bytes());
    h.resize(44 + data as usize, 0);
    h
}

/// Answers from a map and counts every ask.
#[derive(Default)]
pub struct Counting {
    pub table: HashMap<String, f64>,
    pub asked: usize,
}

impl Measure for Counting {
    fn seconds(&mut self, path: &str) -> Result<Option<f64>, SourceError> {
        self.asked += 1;
        Ok(self.table.get(path).copied())
    }
}
