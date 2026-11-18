//! analyse.rs — one file measured into its `{file}.cache`, as `analyseFile` in `packages/audio-analysis/src/analyse.ts`
//! does on k7's backend: the waveform module decodes, sums to mono, measures and correlates, in Wasmtime; ffmpeg reads
//! what the module cannot open; the onsets, the tempo's pick and the sidecar's text are `envelope.rs` and `sidecar.rs`.
//! What a reader gets wrong: every sample's arithmetic is the module's, so node and Rust write the same bytes.

use std::fs::File;
use std::path::Path;
use std::time::{Duration, Instant};

use wasmtime::{Memory, Store, TypedFunc};

use crate::decode::Engine;
use crate::envelope::{
    FRAMES_PER_BUCKET, Facts, Measured, bpm_lags, detect_bpm, envelope_from, followers,
};
use crate::ffmpeg::OnFrames;
use crate::fingerprint::file_fingerprint;
use crate::sidecar::{self, Envelope};
use crate::waveform::{Files, Waveform};

/// `MAX_DECODE_FRAMES`: two hours at 48 kHz, about 1.4 GB of mono floats held at once.
pub const MAX_DECODE_FRAMES: u64 = 2 * 3600 * 48_000;

/// `ANALYSE_DEADLINE_MS`: a decode past this is stuck on its file, not slow.
pub const ANALYSE_DEADLINE: Duration = Duration::from_secs(10 * 60);

/// Frames per decoded block, `BLOCK` in decodeWasm.ts.
const BLOCK: i32 = 1 << 16;

/// Why a file was not measured, in the words node's would say it.
#[derive(Clone, Debug, PartialEq)]
pub enum AnalyseError {
    /// The module could not open the file and nothing else read it (`WasmOpenError`).
    Open(String),
    /// Anything else: an unreadable file, a failed decode, a file too long, the deadline.
    Failed(String),
}

impl AnalyseError {
    pub fn message(&self) -> &str {
        match self {
            AnalyseError::Open(m) | AnalyseError::Failed(m) => m,
        }
    }
}

/// The module's exports a measure calls.
struct Abi {
    memory: Memory,
    hint: TypedFunc<i32, i32>,
    open: TypedFunc<f64, i32>,
    rate: TypedFunc<(), f64>,
    channels: TypedFunc<(), i32>,
    next: TypedFunc<i32, i32>,
    frames_ptr: TypedFunc<(), i32>,
    error_ptr: TypedFunc<(), i32>,
    error_len: TypedFunc<(), i32>,
    input: TypedFunc<i32, i32>,
    push: TypedFunc<(i32, i32), ()>,
    measure: TypedFunc<(f64, f64), i32>,
    output_ptr: TypedFunc<(), i32>,
    output_len: TypedFunc<(), i32>,
    autocorr: TypedFunc<(i32, i32, i32), i32>,
}

/// One file's instance: its decoder, and the measure the frames are pushed into.
struct Session {
    store: Store<Files>,
    abi: Abi,
}

fn trap(e: wasmtime::Error) -> AnalyseError {
    AnalyseError::Failed(format!("the waveform module failed: {e}"))
}

impl Session {
    fn new(waveform: &Waveform) -> Result<Session, AnalyseError> {
        let (mut store, instance) = waveform.instance().map_err(AnalyseError::Failed)?;
        let e = |e: wasmtime::Error| AnalyseError::Failed(e.to_string());
        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or_else(|| AnalyseError::Failed("the waveform module exports no memory".into()))?;
        macro_rules! f {
            ($name:literal) => {
                instance.get_typed_func(&mut store, $name).map_err(e)?
            };
        }
        let abi = Abi {
            memory,
            hint: f!("dec_hint"),
            open: f!("dec_open"),
            rate: f!("dec_rate"),
            channels: f!("dec_channels"),
            next: f!("dec_next"),
            frames_ptr: f!("dec_frames_ptr"),
            error_ptr: f!("dec_error_ptr"),
            error_len: f!("dec_error_len"),
            input: f!("wf_input"),
            push: f!("wf_push"),
            measure: f!("wf_measure"),
            output_ptr: f!("wf_output_ptr"),
            output_len: f!("wf_output_len"),
            autocorr: f!("wf_autocorr"),
        };
        Ok(Session { store, abi })
    }

    /// `wasmError`: the module's last error as `${what} ${source}: ${its message}`.
    fn error(&mut self, what: &str, source: &Path) -> String {
        let at = self
            .abi
            .error_ptr
            .call(&mut self.store, ())
            .unwrap_or(0)
            .max(0) as usize;
        let len = self
            .abi
            .error_len
            .call(&mut self.store, ())
            .unwrap_or(0)
            .max(0) as usize;
        let bytes = self
            .abi
            .memory
            .data(&self.store)
            .get(at..at + len)
            .unwrap_or_default();
        format!(
            "{what} {}: {}",
            source.display(),
            String::from_utf8_lossy(bytes)
        )
    }

    fn floats(&self, at: usize, n: usize) -> Vec<f32> {
        let data = self.abi.memory.data(&self.store);
        data.get(at..at + n * 4)
            .unwrap_or_default()
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| f32::from_le_bytes(*c))
            .collect()
    }

    /// `wf_input` then a copy in: the pointer first, since allocating may grow the memory.
    fn write_input(&mut self, xs: &[f32]) -> Result<i32, AnalyseError> {
        let at = self
            .abi
            .input
            .call(&mut self.store, xs.len() as i32)
            .map_err(trap)?;
        let bytes: Vec<u8> = xs.iter().flat_map(|x| x.to_le_bytes()).collect();
        self.abi
            .memory
            .write(&mut self.store, at as usize, &bytes)
            .map_err(|e| AnalyseError::Failed(e.to_string()))?;
        Ok(xs.len() as i32)
    }

    /// `native.push`: interleaved frames summed to mono inside the module.
    fn push(&mut self, samples: &[f32], channels: i32) -> Result<(), AnalyseError> {
        let len = self.write_input(samples)?;
        self.abi
            .push
            .call(&mut self.store, (len, channels))
            .map_err(trap)
    }

    /// The decoder's block pushed as it lies in the module's memory, copied there rather than through the host.
    fn push_decoded(&mut self, at: usize, len: usize, channels: i32) -> Result<(), AnalyseError> {
        // The input's pointer first: allocating it may grow, and so move, the memory.
        let input = self
            .abi
            .input
            .call(&mut self.store, len as i32)
            .map_err(trap)? as usize;
        let data = self.abi.memory.data_mut(&mut self.store);
        if at + len * 4 > data.len() || input + len * 4 > data.len() {
            return Err(AnalyseError::Failed(
                "the waveform module handed a block outside its memory".into(),
            ));
        }
        data.copy_within(at..at + len * 4, input);
        self.abi
            .push
            .call(&mut self.store, (len as i32, channels))
            .map_err(trap)
    }

    /// `decodeWithWasm`: `Open` when the module cannot open the file, so ffmpeg may still read it; `out` takes the
    /// frames instead of the measure, and `stop` is asked between blocks.
    fn decode(
        &mut self,
        source: &Path,
        deadline: Instant,
        stop: Stop<'_>,
        mut out: Option<&mut OnFrames<'_>>,
    ) -> Result<Facts, AnalyseError> {
        let file = File::open(source).map_err(|e| AnalyseError::Failed(e.to_string()))?;
        let size = file
            .metadata()
            .map_err(|e| AnalyseError::Failed(e.to_string()))?
            .len();
        self.store.data_mut().input = Some(file);
        let ext = source
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let hint = self
            .abi
            .hint
            .call(&mut self.store, ext.len() as i32)
            .map_err(trap)?;
        self.abi
            .memory
            .write(&mut self.store, hint as usize, ext.as_bytes())
            .map_err(|e| AnalyseError::Failed(e.to_string()))?;
        if self
            .abi
            .open
            .call(&mut self.store, size as f64)
            .map_err(trap)?
            < 0
        {
            return Err(AnalyseError::Open(self.error("cannot open", source)));
        }
        let sample_rate = self.abi.rate.call(&mut self.store, ()).map_err(trap)?;
        let channels = self.abi.channels.call(&mut self.store, ()).map_err(trap)?;
        let mut frames: u64 = 0;
        loop {
            if Instant::now() > deadline {
                return Err(deadline_error(source));
            }
            if stop() {
                return Err(aborted(source));
            }
            let n = self.abi.next.call(&mut self.store, BLOCK).map_err(trap)?;
            if n < 0 {
                return Err(AnalyseError::Failed(self.error("cannot decode", source)));
            }
            if n == 0 {
                break;
            }
            too_long(source, frames + n as u64)?;
            frames += n as u64;
            let at = self
                .abi
                .frames_ptr
                .call(&mut self.store, ())
                .map_err(trap)? as usize;
            let len = n as usize * channels.max(0) as usize;
            match out.as_deref_mut() {
                Some(f) => f(&self.floats(at, len), channels)?,
                None => self.push_decoded(at, len, channels)?,
            }
        }
        self.store.data_mut().input = None;
        Ok(Facts {
            sample_rate,
            channels: f64::from(channels),
            frames: frames as f64,
        })
    }

    /// `native.measure` and `envelopeFrom`, the tempo's correlations asked of the module as node asks them.
    fn envelope(
        &mut self,
        facts: Facts,
        fingerprint: &str,
        sensitivity: f64,
    ) -> Result<Envelope, AnalyseError> {
        let (a_low, a_high) = followers(facts.sample_rate);
        let buckets = self
            .abi
            .measure
            .call(&mut self.store, (a_low, a_high))
            .map_err(trap)? as usize;
        let at = self
            .abi
            .output_ptr
            .call(&mut self.store, ())
            .map_err(trap)? as usize;
        let len = self
            .abi
            .output_len
            .call(&mut self.store, ())
            .map_err(trap)? as usize;
        let out = self.floats(at, len);
        let part = |i: usize| {
            out.get(i * buckets..(i + 1) * buckets)
                .unwrap_or_default()
                .to_vec()
        };
        let m = Measured {
            peak: part(0),
            rms: part(1),
            low: part(2),
            mid: part(3),
            high: part(4),
            tempo: out.get(5 * buckets..).unwrap_or_default().to_vec(),
        };
        let rate = facts.sample_rate / FRAMES_PER_BUCKET;
        let bpm = match bpm_lags(m.tempo.len(), rate) {
            Some((lo, hi)) => {
                let n = self.write_input(&m.tempo)?;
                let at = self
                    .abi
                    .autocorr
                    .call(&mut self.store, (n, lo as i32, hi as i32))
                    .map_err(trap)? as usize;
                let data = self.abi.memory.data(&self.store);
                let table: Vec<f64> = data
                    .get(at..at + (hi - lo + 1) * 8)
                    .unwrap_or_default()
                    .as_chunks::<8>()
                    .0
                    .iter()
                    .map(|c| f64::from_le_bytes(*c))
                    .collect();
                detect_bpm(&table, lo, hi, rate)
            }
            None => 0.0,
        };
        Ok(envelope_from(&m, facts, fingerprint, sensitivity, bpm))
    }
}

/// `DeadlineError`: the pool's words for a decode it killed.
fn deadline_error(source: &Path) -> AnalyseError {
    AnalyseError::Failed(format!(
        "analysing {} took longer than {} s",
        source.display(),
        ANALYSE_DEADLINE.as_secs()
    ))
}

/// `TooLongError`, counted on the frames as they arrive: no header is trusted with the length.
pub(crate) fn too_long(source: &Path, frames: u64) -> Result<(), AnalyseError> {
    if frames > MAX_DECODE_FRAMES {
        return Err(AnalyseError::Failed(format!(
            "{} is too long to analyse: {frames} frames, past the {MAX_DECODE_FRAMES} one decode may hold",
            source.display()
        )));
    }
    Ok(())
}

/// `analyseFile(source, sensitivity, { engineBin: null })`: measured, its sidecar written beside it.
pub fn analyse_file(
    waveform: &Waveform,
    source: &Path,
    sensitivity: f64,
) -> Result<Envelope, AnalyseError> {
    analyse_file_with(waveform, source, sensitivity, None, &|| false)
}

/// What a caller says between decoded blocks: true stops the decode (`signal` aborted).
pub type Stop<'a> = &'a dyn Fn() -> bool;

/// A decode a caller stopped, in the words `pump` says it.
pub(crate) fn aborted(source: &Path) -> AnalyseError {
    AnalyseError::Failed(format!("decoding {} was aborted", source.display()))
}

/// `analyseFile(source, sensitivity, { engineBin, signal })`: the engine's `--decode` when one is named, else the module
/// then ffmpeg; measured, its sidecar written beside it.
pub fn analyse_file_with(
    waveform: &Waveform,
    source: &Path,
    sensitivity: f64,
    engine: Option<&Path>,
    stop: Stop<'_>,
) -> Result<Envelope, AnalyseError> {
    let run = engine.map(|bin| Engine { bin, env: None });
    let env = measure_file_with(waveform, source, sensitivity, run.as_ref(), stop)?;
    sidecar::write_atomic(source, &env).map_err(|e| AnalyseError::Failed(e.to_string()))?;
    Ok(env)
}

/// `analyse_file_with` up to the sidecar: the envelope measured, nothing written, for a caller that orders what follows (2414).
pub fn measure_file_with(
    waveform: &Waveform,
    source: &Path,
    sensitivity: f64,
    engine: Option<&Engine<'_>>,
    stop: Stop<'_>,
) -> Result<Envelope, AnalyseError> {
    let deadline = Instant::now() + ANALYSE_DEADLINE;
    let fingerprint = file_fingerprint(source).map_err(|e| AnalyseError::Failed(e.to_string()))?;
    let mut session = Session::new(waveform)?;
    let facts = if let Some(run) = engine {
        let s = &mut session;
        crate::decode::engine_in(run, source, deadline, stop, &mut |samples, channels| {
            if stop() {
                return Err(aborted(source));
            }
            s.push(samples, channels)
        })?
    } else {
        match session.decode(source, deadline, stop, None) {
            Err(AnalyseError::Open(why)) => {
                // Only a file the module could not OPEN goes on to ffmpeg: no frame of it was handed over yet.
                session = Session::new(waveform)?;
                let s = &mut session;
                crate::ffmpeg::decode(source, deadline, &mut |samples, channels| {
                    if stop() {
                        return Err(aborted(source));
                    }
                    s.push(samples, channels)
                })
                .map_err(|e| match e {
                    crate::ffmpeg::FfmpegError::Missing => AnalyseError::Open(why.clone()),
                    crate::ffmpeg::FfmpegError::Failed(e) => e,
                })?
            }
            other => other?,
        }
    };
    session.envelope(facts, &fingerprint, sensitivity)
}

/// `decodeFrames(source, onFrames, { engineBin: null })`: every channel at the file's own rate, through the module,
/// then ffmpeg for a file it cannot open; `Open` when neither reads it.
pub fn decode_frames(
    waveform: &Waveform,
    source: &Path,
    stop: Stop<'_>,
    on_frames: &mut OnFrames<'_>,
) -> Result<Facts, AnalyseError> {
    let deadline = Instant::now() + ANALYSE_DEADLINE;
    let mut session = Session::new(waveform)?;
    match session.decode(source, deadline, stop, Some(&mut *on_frames)) {
        Err(AnalyseError::Open(why)) => {
            crate::ffmpeg::decode(source, deadline, &mut |samples, channels| {
                if stop() {
                    return Err(aborted(source));
                }
                on_frames(samples, channels)
            })
            .map_err(|e| match e {
                crate::ffmpeg::FfmpegError::Missing => AnalyseError::Open(why),
                crate::ffmpeg::FfmpegError::Failed(e) => e,
            })
        }
        other => other,
    }
}
