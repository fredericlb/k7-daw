//! direct.rs — the media work of the host verbs, run on the caller's own thread: decodes, sidecar checks,
//! transcodes, a sound extracted from a video and a true peak, on the waveform module in Wasmtime (`k7-mediatek`).
//! What a reader gets wrong: the limits and the queues are the caller's; this runs one — docs/mediatek.md.

use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use k7_mediatek::analyse::{AnalyseError, analyse_file_with, decode_frames};
use k7_mediatek::sidecar::{self, Envelope};
use k7_mediatek::transcode::{Loudness, Piece, Target, transcode_to};
use k7_mediatek::waveform::{Shared, Waveform};
use serde_json::{Map, Value, json};

/// The folder the compiled waveform module is kept in, `K7_FRONT_CACHE` (set once at start).
static CACHE: OnceLock<std::path::PathBuf> = OnceLock::new();

/// Name the folder the process keeps its compiled waveform module in; the first call wins.
pub fn keep_module_in(dir: std::path::PathBuf) {
    let _ = CACHE.set(dir);
}

/// The waveform module of this process, compiled once into the front's cache folder where one is named.
pub fn module(cache: Option<&Path>) -> Result<Arc<Waveform>, String> {
    static SHARED: OnceLock<Arc<Shared>> = OnceLock::new();
    SHARED
        .get_or_init(|| {
            Shared::new(
                cache
                    .or(CACHE.get().map(|p| p.as_path()))
                    .map(Path::to_path_buf)
                    .or_else(test_cache),
            )
        })
        .get()
}

/// The compiled module the tests of every process share, keyed by the module's hash; production names its own folder.
fn test_cache() -> Option<PathBuf> {
    cfg!(test).then(|| std::env::temp_dir().join("k7-stream-cache"))
}

/// A failure the TypeScript rethrows as its own class (`WasmOpenError`, the deadlines), by `name`.
pub(crate) fn fail(name: &str, message: impl Into<String>) -> Value {
    json!({ "fail": { "name": name, "message": message.into() } })
}

pub(crate) fn text(args: &Value, key: &str) -> String {
    args.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

pub(crate) fn num(args: &Value, key: &str) -> Option<f64> {
    args.get(key).and_then(Value::as_f64)
}

/// `FileEnvelope` as `envelopeFrom` and `readSidecar` build it, keys in their order; the bands only when present.
pub fn envelope_json(e: &Envelope) -> Value {
    let mut m = Map::new();
    m.insert("fingerprint".into(), json!(e.fingerprint));
    m.insert("sampleRate".into(), json!(e.sample_rate));
    m.insert("channels".into(), json!(e.channels));
    m.insert("frames".into(), json!(e.frames));
    m.insert("seconds".into(), json!(e.seconds));
    m.insert("peak".into(), json!(e.peak));
    m.insert("rms".into(), json!(e.rms));
    m.insert("transients".into(), json!(e.transients));
    m.insert("sensitivity".into(), json!(e.sensitivity));
    m.insert("bpm".into(), json!(e.bpm));
    for (k, v) in [("low", &e.low), ("mid", &e.mid), ("high", &e.high)] {
        if let Some(v) = v {
            m.insert(k.into(), json!(v));
        }
    }
    Value::Object(m)
}

/// `checkSidecar(source, sensitivity)`: the sidecar's envelope while it still answers, and the threshold to build at.
pub fn check(source: &Path, sensitivity: Option<f64>) -> Value {
    let stored = std::fs::read_to_string(sidecar::cache_path(source))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.get("sensitivity").and_then(Value::as_f64));
    let wanted = sensitivity
        .or(stored)
        .unwrap_or(sidecar::DEFAULT_SENSITIVITY);
    let cached = sidecar::read(source);
    let usable = cached.as_ref().is_some_and(|c| {
        sensitivity.is_none_or(|s| c.sensitivity == s)
            && k7_mediatek::fingerprint::file_fingerprint(source).is_ok_and(|f| f == c.fingerprint)
    });
    match cached {
        Some(env) if usable => json!({ "env": envelope_json(&env), "sensitivity": wanted }),
        _ => json!({ "sensitivity": wanted }),
    }
}

/// The 44-byte RIFF header of a 32-bit float WAV, `wavHeader(floatFormat(channels, rate), bytes)`.
fn float_wav_header(channels: u32, rate: u32, data: u32) -> [u8; 44] {
    let mut h = [0u8; 44];
    h[0..4].copy_from_slice(b"RIFF");
    h[4..8].copy_from_slice(&(36u32.wrapping_add(data)).to_le_bytes());
    h[8..16].copy_from_slice(b"WAVEfmt ");
    h[16..20].copy_from_slice(&16u32.to_le_bytes());
    h[20..22].copy_from_slice(&3u16.to_le_bytes());
    h[22..24].copy_from_slice(&(channels as u16).to_le_bytes());
    h[24..28].copy_from_slice(&rate.to_le_bytes());
    h[28..32].copy_from_slice(&rate.wrapping_mul(channels).wrapping_mul(4).to_le_bytes());
    h[32..34].copy_from_slice(&((channels * 4) as u16).to_le_bytes());
    h[34..36].copy_from_slice(&32u16.to_le_bytes());
    h[36..40].copy_from_slice(b"data");
    h[40..44].copy_from_slice(&data.to_le_bytes());
    h
}

/// `extractWith`: one decoder's attempt into `{dest}.part`, renamed over `dest` once finished; its facts.
fn extract_with(
    wf: &Waveform,
    source: &Path,
    dest: &Path,
    engine: Option<&Path>,
    stop: &dyn Fn() -> bool,
) -> Result<Value, AnalyseError> {
    let failed = |e: std::io::Error| AnalyseError::Failed(e.to_string());
    // A name of its own: a dropped runtime's decode may still be writing beside it.
    let tmp = k7_mediatek::write::part_of(dest);
    let mut file = std::fs::File::create(&tmp).map_err(failed)?;
    let run = (|| {
        file.write_all(&[0u8; 44]).map_err(failed)?;
        let mut on = |samples: &[f32], _channels: i32| -> Result<(), AnalyseError> {
            if stop() {
                return Err(AnalyseError::Failed(format!(
                    "decoding {} was aborted",
                    source.display()
                )));
            }
            let bytes: Vec<u8> = samples.iter().flat_map(|x| x.to_le_bytes()).collect();
            file.write_all(&bytes).map_err(failed)
        };
        let facts = match engine {
            Some(bin) => k7_mediatek::decode::engine(
                bin,
                source,
                Instant::now() + k7_mediatek::analyse::ANALYSE_DEADLINE,
                &mut on,
            )?,
            None => decode_frames(wf, source, stop, &mut on)?,
        };
        let data = (facts.frames * facts.channels * 4.0) as u32;
        file.seek(SeekFrom::Start(0)).map_err(failed)?;
        file.write_all(&float_wav_header(
            facts.channels as u32,
            facts.sample_rate as u32,
            data,
        ))
        .map_err(failed)?;
        file.flush().map_err(failed)?;
        Ok(
            json!({ "sampleRate": facts.sample_rate, "channels": facts.channels, "frames": facts.frames }),
        )
    })();
    drop(file);
    match run {
        Ok(v) => {
            std::fs::rename(&tmp, dest).map_err(failed)?;
            Ok(v)
        }
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

/// `extractSound`: the module then ffmpeg, the engine only for a file neither opened; null for a silent film.
fn extract(
    wf: &Waveform,
    source: &Path,
    dest: &Path,
    engine: Option<&Path>,
    stop: &dyn Fn() -> bool,
) -> Value {
    let first = match extract_with(wf, source, dest, None, stop) {
        Ok(v) => return json!({ "facts": v }),
        Err(e) => e,
    };
    if first.message().contains("no audio stream") {
        return json!({ "facts": null });
    }
    match (&first, engine) {
        (AnalyseError::Open(_), Some(bin)) => match extract_with(wf, source, dest, Some(bin), stop)
        {
            Ok(v) => json!({ "facts": v }),
            Err(e) => fail("Error", e.message()),
        },
        (AnalyseError::Open(m), None) => fail("WasmOpenError", m.clone()),
        _ => fail("Error", first.message()),
    }
}

/// `measureFilePeak`: the largest magnitude on any channel, one block in memory, through the engine where one is named.
fn peak(wf: &Waveform, source: &Path, engine: Option<&Path>, stop: &dyn Fn() -> bool) -> Value {
    let mut peak = 0f32;
    let mut on = |samples: &[f32], _channels: i32| -> Result<(), AnalyseError> {
        if stop() {
            return Err(AnalyseError::Failed(format!(
                "decoding {} was aborted",
                source.display()
            )));
        }
        peak = samples.iter().fold(peak, |m, x| m.max(x.abs()));
        Ok(())
    };
    let facts = match engine {
        Some(bin) => k7_mediatek::decode::engine(
            bin,
            source,
            Instant::now() + k7_mediatek::analyse::ANALYSE_DEADLINE,
            &mut on,
        ),
        None => decode_frames(wf, source, stop, &mut on),
    };
    match facts {
        Ok(f) => {
            json!({ "facts": { "sampleRate": f.sample_rate, "channels": f.channels, "frames": f.frames, "peak": f64::from(peak) } })
        }
        Err(AnalyseError::Open(m)) => fail("WasmOpenError", m),
        Err(e) => fail("Error", e.message()),
    }
}

/// `{ kind: "opus" | "flac" | "wav", … }` as transcode.ts reads it.
fn target_of(t: &Value) -> Option<Target> {
    let int = |k: &str, d: f64| num(t, k).unwrap_or(d);
    match t.get("kind")?.as_str()? {
        "opus" => Some(Target::Opus {
            bitrate: int("bitrate", 128_000.0) as u32,
            channels: int("channels", 0.0) as i32,
        }),
        "flac" => Some(Target::Flac {
            bits: int("bits", 24.0) as u32,
        }),
        "wav" => Some(Target::Wav {
            rate: num(t, "rate")?,
            channels: int("channels", 0.0) as i32,
        }),
        _ => None,
    }
}

/// `transcodeOffThread`'s work: the answer, a `TranscodeDeadlineError` past the caller's deadline, or the module's refusal.
fn transcode(wf: &Waveform, args: &Value, stop: &dyn Fn() -> bool) -> Value {
    let pieces: Vec<Piece> = args
        .get("pieces")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|p| Piece {
                    source: PathBuf::from(text(p, "source")),
                    start: num(p, "start"),
                    duration: num(p, "duration"),
                })
                .collect()
        })
        .unwrap_or_default();
    let dest = PathBuf::from(text(args, "dest"));
    let Some(target) = args.get("target").and_then(target_of) else {
        return fail("Error", "not a target transcode writes");
    };
    let loudness = args
        .get("loudness")
        .filter(|l| l.is_object())
        .map(|l| Loudness {
            lufs: num(l, "lufs").unwrap_or(f64::NAN),
            true_peak: num(l, "truePeak").unwrap_or(f64::NAN),
        });
    let ms = num(args, "deadlineMs").unwrap_or(0.0).max(0.0);
    let deadline = Instant::now() + Duration::from_millis(ms as u64);
    let late = format!(
        "writing {} took longer than {} s",
        dest.display(),
        k7_js_round(ms / 1000.0)
    );
    let part = args.get("part").and_then(Value::as_str).map(PathBuf::from);
    let said = |_: &Path| -> Option<String> {
        if Instant::now() > deadline {
            return Some(late.clone());
        }
        stop().then(|| "the transcode was aborted".to_string())
    };
    match transcode_to(
        wf,
        &pieces,
        &dest,
        &target,
        loudness.as_ref(),
        &said,
        part.as_deref(),
    ) {
        Ok((t, rate)) => json!({ "frames": t.frames, "sampleRate": rate, "gainDb": t.gain_db }),
        Err(e) if e.message == late => fail("TranscodeDeadlineError", e.message),
        Err(e) if e.open => fail("WasmOpenError", e.message),
        Err(e) => fail("Error", e.message),
    }
}

/// `Math.round` of a positive number of seconds, for a message.
fn k7_js_round(x: f64) -> i64 {
    (x + 0.5).floor() as i64
}

/// One `media.*` ask's work, on a blocking thread.
pub(crate) fn run(
    kind: &str,
    args: &Value,
    cache: Option<&Path>,
    stop: &dyn Fn() -> bool,
) -> Result<Value, String> {
    let source = PathBuf::from(text(args, "source"));
    if kind == "media.check" {
        return Ok(check(&source, num(args, "sensitivity")));
    }
    let wf = module(cache)?;
    let engine = args
        .get("engineBin")
        .and_then(Value::as_str)
        .filter(|b| !b.is_empty())
        .map(PathBuf::from);
    Ok(match kind {
        "media.analyse" => {
            let sensitivity = num(args, "sensitivity").unwrap_or(sidecar::DEFAULT_SENSITIVITY);
            match analyse_file_with(&wf, &source, sensitivity, engine.as_deref(), stop) {
                Ok(env) => envelope_json(&env),
                Err(AnalyseError::Open(m)) => fail("WasmOpenError", m),
                Err(AnalyseError::Failed(m)) if m.contains("took longer than") => {
                    fail("DeadlineError", m)
                }
                Err(AnalyseError::Failed(m)) => fail("Error", m),
            }
        }
        "media.extract" => extract(
            &wf,
            &source,
            Path::new(&text(args, "dest")),
            engine.as_deref(),
            stop,
        ),
        "media.peak" => peak(&wf, &source, engine.as_deref(), stop),
        _ => transcode(&wf, args, stop),
    })
}

/// A `media.*` ask run on the caller's own thread: the host routes' peak and extract (3f).
pub fn run_direct(kind: &str, args: &Value, stop: &dyn Fn() -> bool) -> Result<Value, String> {
    run(kind, args, None, stop)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_the_float_header_wav_ts_writes() {
        let h = float_wav_header(2, 48000, 800);
        assert_eq!(&h[0..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(h[4..8].try_into().unwrap()), 836);
        assert_eq!(u16::from_le_bytes(h[20..22].try_into().unwrap()), 3);
        assert_eq!(u32::from_le_bytes(h[28..32].try_into().unwrap()), 384000);
        assert_eq!(u16::from_le_bytes(h[32..34].try_into().unwrap()), 8);
        assert_eq!(&h[36..40], b"data");
    }

    #[test]
    fn reads_the_three_targets() {
        assert_eq!(
            target_of(&json!({ "kind": "wav", "rate": 16000, "channels": 1 })),
            Some(Target::Wav {
                rate: 16000.0,
                channels: 1
            })
        );
        assert_eq!(
            target_of(&json!({ "kind": "opus" })),
            Some(Target::Opus {
                bitrate: 128_000,
                channels: 0
            })
        );
        assert_eq!(
            target_of(&json!({ "kind": "flac" })),
            Some(Target::Flac { bits: 24 })
        );
        assert_eq!(target_of(&json!({ "kind": "wav" })), None);
    }

    #[test]
    fn keeps_the_envelopes_keys_in_nodes_order() {
        let e = Envelope {
            fingerprint: "f".into(),
            sample_rate: 8000.0,
            channels: 1.0,
            frames: 8.0,
            seconds: 0.001,
            peak: vec![0.5],
            rms: vec![0.25],
            transients: vec![],
            sensitivity: 2.0,
            bpm: 0.0,
            low: Some(vec![0.1]),
            mid: None,
            high: None,
        };
        let keys: Vec<String> = envelope_json(&e)
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        assert_eq!(
            keys,
            [
                "fingerprint",
                "sampleRate",
                "channels",
                "frames",
                "seconds",
                "peak",
                "rms",
                "transients",
                "sensitivity",
                "bpm",
                "low"
            ]
        );
    }
}
