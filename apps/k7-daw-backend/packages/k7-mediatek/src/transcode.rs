//! transcode.rs — `@studio/audio-analysis`'s `transcode.ts` to Opus, run on the very WebAssembly module node runs:
//! one instance per transcode, the same calls in the same order (seek with pre-roll, loudness measured in the module,
//! the decoder's block encoded where it lies), the output written to a part file and renamed over its destination.
//! What a reader gets wrong: a thread cannot be killed, so the caller's `stop` is asked between blocks.

use std::fs::File;
use std::path::{Path, PathBuf};

use k7_js::number::math_round;
use wasmtime::{Memory, Store, TypedFunc};

use crate::waveform::{Files, Waveform};

/// Frames per decoded block, `BLOCK` in transcode.ts.
const BLOCK: i32 = 1 << 16;

/// How far before a cut's start a seek lands: Opus needs 80 ms of pre-roll, and a seek may fall short.
const PRE_ROLL: f64 = 0.25;

/// `KINDS`: what `enc_open` is told to write.
const OPUS: i32 = 0;
const FLAC: i32 = 1;
const WAV: i32 = 2;

/// How `wasmError` names a source the module could not open.
const OPEN_FAILED: &str = "cannot open";

/// transcode.ts's `Target`: Opus in WebM at 48 kHz, FLAC at the first source's rate, or 16-bit WAV at `rate`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    Opus { bitrate: u32, channels: i32 },
    Flac { bits: u32 },
    Wav { rate: f64, channels: i32 },
}

impl Target {
    /// `enc_open`'s kind, wanted channels (0 keeps the source's) and parameter, as transcode.ts derives them.
    fn opened(&self) -> (i32, i32, f64) {
        match *self {
            Target::Opus { bitrate, channels } => (OPUS, channels, f64::from(bitrate)),
            Target::Flac { bits } => (FLAC, 0, f64::from(bits)),
            Target::Wav { rate, channels } => (WAV, channels, rate),
        }
    }
}

/// A file, or `duration` seconds of it from `start`.
pub struct Piece {
    pub source: PathBuf,
    pub start: Option<f64>,
    pub duration: Option<f64>,
}

impl Piece {
    /// The whole file.
    pub fn whole(source: PathBuf) -> Piece {
        Piece {
            source,
            start: None,
            duration: None,
        }
    }
}

/// EBU R128's integrated loudness to bring one piece to, under a true-peak ceiling in dBTP.
pub struct Loudness {
    pub lufs: f64,
    pub true_peak: f64,
}

/// `{ kind: "opus", bitrate, channels }`: Opus in WebM at 48 kHz; `channels` 0 keeps the first source's.
pub struct Opus {
    pub bitrate: u32,
    pub channels: i32,
}

/// What a transcode wrote: its frames, and the gain it applied in dB (0 unless a loudness was asked).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transcoded {
    pub frames: f64,
    pub gain_db: f64,
}

/// What the caller says between blocks: `Some(why)` stops the transcode with that error.
pub type Stop<'a> = &'a dyn Fn(&Path) -> Option<String>;

/// The exports a transcode calls (`EncodeAbi`).
struct Abi {
    memory: Memory,
    hint: TypedFunc<i32, i32>,
    open: TypedFunc<f64, i32>,
    rate: TypedFunc<(), f64>,
    channels: TypedFunc<(), i32>,
    next: TypedFunc<i32, i32>,
    seek: TypedFunc<f64, f64>,
    error_ptr: TypedFunc<(), i32>,
    error_len: TypedFunc<(), i32>,
    enc_open: TypedFunc<(i32, f64, i32, i32, f64), i32>,
    enc_reformat: TypedFunc<(f64, i32), i32>,
    enc_push: TypedFunc<(i32, i32, f32), i32>,
    enc_finish: TypedFunc<(), f64>,
    lufs_open: TypedFunc<(f64, i32), i32>,
    lufs_push: TypedFunc<(i32, i32), i32>,
    lufs_integrated: TypedFunc<(), f64>,
    lufs_true_peak: TypedFunc<(), f64>,
}

/// Where a decoded block goes.
#[derive(Clone, Copy)]
enum Push {
    Measure,
    Encode(f32),
}

/// One instance reading whichever source is open and writing one output file (transcode.ts's `Session`).
struct Session {
    store: Store<Files>,
    abi: Abi,
    /// Set when the module could not open a source (`WasmOpenError`), as opposed to any other failure.
    refused: bool,
}

fn trap(e: wasmtime::Error) -> String {
    format!("the waveform module failed: {e}")
}

impl Session {
    fn new(waveform: &Waveform) -> Result<Session, String> {
        let (mut store, instance) = waveform.instance()?;
        let e = |e: wasmtime::Error| e.to_string();
        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or("the waveform module exports no memory")?;
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
            seek: f!("dec_seek"),
            error_ptr: f!("dec_error_ptr"),
            error_len: f!("dec_error_len"),
            enc_open: f!("enc_open"),
            enc_reformat: f!("enc_reformat"),
            enc_push: f!("enc_push_decoded"),
            enc_finish: f!("enc_finish"),
            lufs_open: f!("lufs_open"),
            lufs_push: f!("lufs_push_decoded"),
            lufs_integrated: f!("lufs_integrated"),
            lufs_true_peak: f!("lufs_true_peak"),
        };
        Ok(Session {
            store,
            abi,
            refused: false,
        })
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

    /// `open`: `source` in the decoder, the one before closed, its extension as the hint; its rate and channels.
    fn open(&mut self, source: &Path) -> Result<(f64, i32), String> {
        self.store.data_mut().input = None;
        let input =
            File::open(source).map_err(|e| format!("cannot open {}: {e}", source.display()))?;
        let size = input
            .metadata()
            .map_err(|e| format!("cannot open {}: {e}", source.display()))?
            .len();
        self.store.data_mut().input = Some(input);
        let ext = source
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        // The pointer first: allocating may grow the memory.
        let hint = self
            .abi
            .hint
            .call(&mut self.store, ext.len() as i32)
            .map_err(trap)?;
        self.abi
            .memory
            .write(&mut self.store, hint as usize, ext.as_bytes())
            .map_err(|e| e.to_string())?;
        if self
            .abi
            .open
            .call(&mut self.store, size as f64)
            .map_err(trap)?
            < 0
        {
            self.refused = true;
            return Err(self.error(OPEN_FAILED, source));
        }
        let rate = self.abi.rate.call(&mut self.store, ()).map_err(trap)?;
        let channels = self.abi.channels.call(&mut self.store, ()).map_err(trap)?;
        Ok((rate, channels))
    }

    /// `eachRange`: every decoded block of `piece` in its range, its `[from, from + count)` frames pushed as `push` says.
    fn each_range(
        &mut self,
        piece: &Piece,
        rate: f64,
        stop: Stop<'_>,
        push: Push,
    ) -> Result<(), String> {
        let source = piece.source.as_path();
        let start = math_round(piece.start.unwrap_or(0.0) * rate);
        let end = piece
            .duration
            .map_or(f64::INFINITY, |d| start + math_round(d * rate));
        let mut pos = 0.0;
        if start > PRE_ROLL * rate {
            let at = self
                .abi
                .seek
                .call(&mut self.store, start / rate - PRE_ROLL)
                .map_err(trap)?;
            // A file that cannot seek is decoded from its start, which is slower and as exact.
            if at >= 0.0 {
                pos = math_round(at * rate);
            } else {
                self.open(source)?;
            }
        }
        while pos < end {
            if let Some(why) = stop(source) {
                return Err(why);
            }
            let n = self.abi.next.call(&mut self.store, BLOCK).map_err(trap)?;
            if n < 0 {
                return Err(self.error("cannot decode", source));
            }
            if n == 0 {
                break;
            }
            let from = (start - pos).max(0.0);
            let to = f64::from(n).min(end - pos);
            if to > from {
                let (from, count) = (from as i32, (to - from) as i32);
                let (r, what) = match push {
                    Push::Measure => (
                        self.abi.lufs_push.call(&mut self.store, (from, count)),
                        "cannot measure",
                    ),
                    Push::Encode(gain) => (
                        self.abi.enc_push.call(&mut self.store, (from, count, gain)),
                        "cannot encode",
                    ),
                };
                if r.map_err(trap)? < 0 {
                    return Err(self.error(what, source));
                }
            }
            pos += f64::from(n);
        }
        Ok(())
    }

    /// `gainFor`: the gain in dB that brings `piece` to `target`, never past its true-peak ceiling.
    fn gain_for(
        &mut self,
        piece: &Piece,
        target: &Loudness,
        stop: Stop<'_>,
    ) -> Result<f64, String> {
        let (rate, channels) = self.open(&piece.source)?;
        if self
            .abi
            .lufs_open
            .call(&mut self.store, (rate, channels))
            .map_err(trap)?
            < 0
        {
            return Err(self.error("cannot measure", &piece.source));
        }
        self.each_range(piece, rate, stop, Push::Measure)?;
        let lufs = self
            .abi
            .lufs_integrated
            .call(&mut self.store, ())
            .map_err(trap)?;
        let peak = self
            .abi
            .lufs_true_peak
            .call(&mut self.store, ())
            .map_err(trap)?;
        // Silence has no loudness to correct.
        if !lufs.is_finite() {
            return Ok(0.0);
        }
        let gain = target.lufs - lufs;
        Ok(if peak.is_finite() {
            gain.min(target.true_peak - peak)
        } else {
            gain
        })
    }

    /// The encode itself, into the output already open; the frames written.
    fn encode(
        &mut self,
        pieces: &[Piece],
        dest: &Path,
        target: &Target,
        gain: f32,
        stop: Stop<'_>,
    ) -> Result<(f64, f64), String> {
        let mut first_rate = 0.0;
        for (i, piece) in pieces.iter().enumerate() {
            let (rate, channels) = self.open(&piece.source)?;
            if i == 0 {
                first_rate = rate;
                let (kind, want, param) = target.opened();
                let opened = (kind, rate, channels, want, param);
                if self
                    .abi
                    .enc_open
                    .call(&mut self.store, opened)
                    .map_err(trap)?
                    < 0
                {
                    return Err(self.error("cannot encode", &piece.source));
                }
            } else if self
                .abi
                .enc_reformat
                .call(&mut self.store, (rate, channels))
                .map_err(trap)?
                < 0
            {
                return Err(self.error("cannot join", &piece.source));
            }
            self.each_range(piece, rate, stop, Push::Encode(gain))?;
        }
        let frames = self
            .abi
            .enc_finish
            .call(&mut self.store, ())
            .map_err(trap)?;
        if frames < 0.0 {
            return Err(self.error("cannot finish", dest));
        }
        let rate = match *target {
            Target::Opus { .. } => 48_000.0,
            Target::Wav { rate, .. } => rate,
            Target::Flac { .. } => first_rate,
        };
        Ok((frames, rate))
    }
}

/// `partFor`: a dot-name beside `dest`, so no walk of its folder counts the file before it is finished.
pub fn part_for(dest: &Path) -> PathBuf {
    let mut r = [0u8; 16];
    let _ = getrandom::fill(&mut r);
    // A v4 UUID, as `randomUUID` draws it.
    r[6] = (r[6] & 0x0f) | 0x40;
    r[8] = (r[8] & 0x3f) | 0x80;
    let hex: String = r.iter().map(|b| format!("{b:02x}")).collect();
    let uuid = format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    );
    let name = dest
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    dest.with_file_name(format!(".{name}.{uuid}.part"))
}

/// `transcode(pieces, dest, { kind: "opus", … }, { loudness })`: `pieces` decoded in order, joined, into `dest`.
pub fn transcode(
    waveform: &Waveform,
    pieces: &[Piece],
    dest: &Path,
    target: &Opus,
    loudness: Option<&Loudness>,
    stop: Stop<'_>,
) -> Result<Transcoded, String> {
    let to = Target::Opus {
        bitrate: target.bitrate,
        channels: target.channels,
    };
    transcode_to(waveform, pieces, dest, &to, loudness, stop, None)
        .map(|(t, _)| t)
        .map_err(|e| e.message)
}

/// Why a transcode failed; `open` when the module could not open a source, so ffmpeg may still read it (`WasmOpenError`).
#[derive(Debug, Clone, PartialEq)]
pub struct TranscodeError {
    pub message: String,
    pub open: bool,
}

/// `transcode(pieces, dest, target, { loudness, part })`: what was written, and its rate.
pub fn transcode_to(
    waveform: &Waveform,
    pieces: &[Piece],
    dest: &Path,
    target: &Target,
    loudness: Option<&Loudness>,
    stop: Stop<'_>,
    part: Option<&Path>,
) -> Result<(Transcoded, f64), TranscodeError> {
    let plain = |message: String| TranscodeError {
        message,
        open: false,
    };
    if pieces.is_empty() {
        return Err(plain("nothing to transcode".into()));
    }
    if loudness.is_some() && pieces.len() > 1 {
        return Err(plain(
            "a loudness is brought to one piece, not to a join".into(),
        ));
    }
    let mut s = Session::new(waveform).map_err(plain)?;
    let tmp = part.map_or_else(|| part_for(dest), Path::to_path_buf);
    let out = (|| {
        // Measuring decodes the piece once more, which a loudness asks for and nothing else does.
        let gain_db = match loudness {
            Some(l) => s.gain_for(&pieces[0], l, stop)?,
            None => 0.0,
        };
        // `10 ** (gainDb / 20)` as the f32 the module's parameter takes.
        let gain = 10f64.powf(gain_db / 20.0) as f32;
        let output =
            File::create(&tmp).map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
        s.store.data_mut().output = Some(output);
        let (frames, rate) = s.encode(pieces, dest, target, gain, stop)?;
        s.store.data_mut().output = None;
        std::fs::rename(&tmp, dest).map_err(|e| format!("cannot write {}: {e}", dest.display()))?;
        Ok((Transcoded { frames, gain_db }, rate))
    })();
    let open = s.refused;
    drop(s);
    let _ = std::fs::remove_file(&tmp);
    out.map_err(|message: String| TranscodeError { open, message })
}
