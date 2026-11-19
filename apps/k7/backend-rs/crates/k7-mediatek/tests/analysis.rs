//! A sidecar and an envelope written by the waveform module in Wasmtime and the ported glue, held to what node's
//! `analyseFile` wrote for the same files (`fixtures/analysis-vectors.json`, kept as committed), and V8's
//! `Math.exp`/`Math.log2` replayed over the same draws. The ignored test takes a real library's copies (`K7_ANALYSIS_VECTORS`).

use std::path::{Path, PathBuf};
use std::time::Instant;

use k7_js::math::{exp, log2};
use k7_mediatek::analyse::analyse_file;
use k7_mediatek::sidecar::{DEFAULT_SENSITIVITY, Envelope, cache_path};
use k7_mediatek::waveform::Waveform;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

struct SplitMix(u64);

impl SplitMix {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }
}

const TWO_PI: f64 = std::f64::consts::TAU;

fn unit(r: u64) -> f64 {
    (r >> 11) as f64 / 9007199254740992.0
}

fn draw_exp(g: &mut SplitMix) -> f64 {
    let kind = g.next() % 5;
    let r = g.next();
    match kind {
        0 => unit(r) * 1455.0 - 745.0,
        1 => (unit(r) - 0.5) / 2f64.powi(((r >> 3) % 60) as i32),
        2 => (-TWO_PI * if r & 1 == 1 { 4000.0 } else { 200.0 }) / (1 + r % 200000) as f64,
        3 => {
            let o = unit(r) * 6.0 - 3.0;
            -0.5 * o * o
        }
        _ => f64::from_bits(r),
    }
}

fn draw_log2(g: &mut SplitMix) -> f64 {
    let kind = g.next() % 4;
    let r = g.next();
    match kind {
        0 => ((r % 40000) + 1) as f64 / 100.0 / 120.0,
        1 => 1.0 + (unit(r) - 0.5) / 2f64.powi(((r >> 3) % 40) as i32),
        2 => unit(r) * 2f64.powi(((r >> 50) % 200) as i32 - 100),
        _ => f64::from_bits(r & ((1 << 63) - 1)),
    }
}

fn bits(x: f64) -> String {
    if x.is_nan() {
        "nan".into()
    } else {
        format!("{:x}", x.to_bits())
    }
}

fn hex(bytes: impl AsRef<[u8]>) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn digests(
    seed: u64,
    draw: fn(&mut SplitMix) -> f64,
    f: fn(f64) -> f64,
    batch: u64,
    n: usize,
) -> Vec<String> {
    let mut g = SplitMix(seed);
    (0..n)
        .map(|_| {
            let mut h = Sha256::new();
            for _ in 0..batch {
                let x = draw(&mut g);
                h.update(format!("{} {}\n", bits(x), bits(f(x))));
            }
            h.finalize().iter().map(|b| format!("{b:02x}")).collect()
        })
        .collect()
}

fn vectors() -> Value {
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/analysis-vectors.json");
    serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap()
}

#[test]
fn exp_and_log2_are_v8s() {
    let v = vectors();
    let batch = v["batch"].as_u64().unwrap();
    let want = |k: &str| -> Vec<String> {
        v[k].as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().to_string())
            .collect()
    };
    let e = want("exp");
    assert_eq!(digests(0x65787, draw_exp, exp, batch, e.len()), e);
    let l = want("log2");
    assert_eq!(digests(0x6c6f67, draw_log2, log2, batch, l.len()), l);
}

/// The vectors' `synthWav`: integers only, so both languages write the same bytes.
fn synth_wav(rate: u32, channels: u32, frames: u32, bpm: f64, seed: i32) -> Vec<u8> {
    let mut b = Vec::with_capacity(44 + (frames * channels * 2) as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + frames * channels * 2).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&(channels as u16).to_le_bytes());
    b.extend_from_slice(&rate.to_le_bytes());
    b.extend_from_slice(&(rate * channels * 2).to_le_bytes());
    b.extend_from_slice(&((channels * 2) as u16).to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&(frames * channels * 2).to_le_bytes());
    let period = if bpm > 0.0 {
        ((f64::from(rate) * 60.0) / bpm).floor() as u32
    } else {
        0
    };
    let click = rate / 100;
    let mut s = seed;
    for i in 0..frames {
        for c in 0..channels {
            s = s.wrapping_mul(1103515245).wrapping_add(12345) & 0x7fffffff;
            let noise = ((s >> 16) % 64) - 32;
            let tri = ((i * (3 + c)) % 200) as i32 - 100;
            let on = period > 0 && i % period < click;
            let v = if on { tri * 250 + noise } else { noise };
            b.extend_from_slice(&(v as i16).to_le_bytes());
        }
    }
    b
}

/// The envelope `/envelope` answers from a fresh analysis, as the front's route spells it.
fn envelope_body(env: &Envelope) -> String {
    let mut o = Map::new();
    o.insert("sampleRate".into(), json!(env.sample_rate));
    o.insert("seconds".into(), json!(env.seconds));
    o.insert("rms".into(), json!(env.rms));
    o.insert("transients".into(), json!(env.transients));
    o.insert("peak".into(), json!(env.peak));
    for (k, v) in [("low", &env.low), ("mid", &env.mid), ("high", &env.high)] {
        if let Some(v) = v {
            o.insert(k.into(), json!(v));
        }
    }
    k7_mediatek::json::stringify(&Value::Object(o))
}

/// One file analysed as node's `analysed` reports it.
fn analysed(waveform: &Waveform, src: &Path) -> Value {
    match analyse_file(waveform, src, DEFAULT_SENSITIVITY) {
        Ok(env) => {
            let side = std::fs::read(cache_path(src)).unwrap();
            json!({
                "sidecar": hex(&side),
                "bytes": side.len(),
                "envelope": hex(envelope_body(&env)),
                "bpm": env.bpm,
                "transients": env.transients.len(),
            })
        }
        Err(e) => json!({ "error": e.message().replace(&src.display().to_string(), "<source>") }),
    }
}

fn same(name: &str, got: &Value, want: &Value) {
    for k in ["sidecar", "bytes", "envelope", "bpm", "transients", "error"] {
        let w = &want[k];
        if w.is_null() {
            assert!(got[k].is_null(), "{name}: {k} is {}", got[k]);
        } else if let (Some(a), Some(b)) = (got[k].as_f64(), w.as_f64()) {
            assert_eq!(a, b, "{name}: {k}");
        } else {
            assert_eq!(&got[k], w, "{name}: {k}");
        }
    }
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("k7-analysis-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn no_ffprobe() {
    // SAFETY: the variable is set before any thread of this test binary reads it, and to the same value each time.
    unsafe { std::env::set_var("FFPROBE_BIN", "k7-no-ffprobe-here") };
}

#[test]
fn writes_the_sidecar_node_writes() {
    no_ffprobe();
    let v = vectors();
    let dir = scratch("files");
    let waveform = Waveform::new(Some(&std::env::temp_dir().join("k7-stream-cache"))).unwrap();
    let files = v["files"].as_array().unwrap();
    let shapes: [(&str, u32, u32, u32, f64); 8] = [
        ("mono-8k-120", 8000, 1, 32000, 120.0),
        ("stereo-44k-140", 44100, 2, 264600, 140.0),
        ("mono-22k-87", 22050, 1, 176400, 87.0),
        ("quad-48k-174", 48000, 4, 144000, 174.0),
        ("mono-11k-70", 11025, 1, 110250, 70.0),
        ("stereo-48k-silence", 48000, 2, 96000, 0.0),
        ("one-frame", 44100, 1, 1, 0.0),
        ("no-frames", 44100, 1, 0, 0.0),
    ];
    let want = |name: &str| {
        files
            .iter()
            .find(|f| f["name"] == name)
            .unwrap_or_else(|| panic!("{name}"))
    };
    for (name, rate, channels, frames, bpm) in shapes {
        let wav = synth_wav(rate, channels, frames, bpm, (rate + channels) as i32);
        let w = want(name);
        assert_eq!(
            hex(&wav),
            w["wav"].as_str().unwrap(),
            "{name}: the WAV itself"
        );
        let src = dir.join(format!("{name}.wav"));
        std::fs::write(&src, wav).unwrap();
        same(name, &analysed(&waveform, &src), w);
    }
    let text = dir.join("song.yaml");
    std::fs::write(&text, "title: not audio\n").unwrap();
    same("song.yaml", &analysed(&waveform, &text), want("song.yaml"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
#[ignore = "takes K7_ANALYSIS_VECTORS, a real library's audio from makeAnalysisVectors.ts <out> <folder>"]
fn writes_the_sidecar_node_writes_for_a_real_library() {
    no_ffprobe();
    let file = std::env::var("K7_ANALYSIS_VECTORS").expect("K7_ANALYSIS_VECTORS");
    let v: Value = serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap();
    let root = PathBuf::from(v["root"].as_str().unwrap());
    let dir = scratch("corpus");
    let waveform = Waveform::new(Some(&std::env::temp_dir().join("k7-stream-cache"))).unwrap();
    let (mut node_ms, mut rust_ms, mut differ) = (0.0, 0.0, Vec::new());
    let files = v["files"].as_array().unwrap();
    for f in files {
        let name = f["name"].as_str().unwrap();
        let copy = dir.join(f["copy"].as_str().unwrap());
        std::fs::copy(root.join(name), &copy).unwrap();
        let t = Instant::now();
        let got = analysed(&waveform, &copy);
        rust_ms += t.elapsed().as_secs_f64() * 1000.0;
        node_ms += f["ms"].as_f64().unwrap_or(0.0);
        let ok = ["sidecar", "envelope", "error"]
            .iter()
            .all(|k| got[k] == f[*k]);
        if !ok {
            differ.push(format!("{name}: {got} against {f}"));
        }
        let _ = std::fs::remove_file(&copy);
        let _ = std::fs::remove_file(cache_path(&copy));
    }
    println!(
        "{} files, {} differ; node {:.0} ms, Rust {:.0} ms",
        files.len(),
        differ.len(),
        node_ms,
        rust_ms
    );
    let _ = std::fs::remove_dir_all(&dir);
    assert!(differ.is_empty(), "{}", differ.join("\n"));
}
