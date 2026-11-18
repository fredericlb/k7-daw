//! ffmpeg.rs — a file the waveform module cannot open, read through ffprobe and ffmpeg as `decodeWithFfmpeg` in
//! `packages/audio-analysis/src/decode.ts` reads it: the stream's facts first, then every channel as f32le, at its rate.
//! What a reader gets wrong: only local files are opened (`-protocol_whitelist file,pipe`), and a missing ffprobe is
//! asked for once per process, after which the module's own refusal is the answer.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::Instant;

use serde_json::Value;

use crate::analyse::{AnalyseError, MAX_DECODE_FRAMES, too_long};
use crate::envelope::Facts;

/// What the decoded frames are handed to: whole interleaved frames and the channel count.
pub type OnFrames<'a> = dyn FnMut(&[f32], i32) -> Result<(), AnalyseError> + 'a;

/// Why ffmpeg read nothing.
pub enum FfmpegError {
    /// ffprobe or ffmpeg is not installed: the caller answers with the module's refusal.
    Missing,
    Failed(AnalyseError),
}

/// The ffprobe found missing, so a host without ffmpeg asks for it once rather than once per file.
static MISSING: Mutex<Option<String>> = Mutex::new(None);

fn bin(var: &str, fallback: &str) -> String {
    std::env::var(var)
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

/// The demuxers an input may be read with: media containers only, never a playlist or a list of other files.
pub const FORMATS: &str = "wav,w64,mp3,flac,ogg,mov,aac,aiff,matroska,asf,caf,wv,ape,tta,dsf,ac3,eac3,amr,au,voc,mpc,mpc8,dts,truehd,avi,mpeg,mpegts,flv";

/// `input(source)`: the whitelists, then the file named as a file so a colon is never a protocol.
fn input(source: &Path) -> Vec<String> {
    let s = source.display().to_string();
    let named = if s == "-" || s.starts_with("pipe:") || s.starts_with("file:") {
        s
    } else {
        format!("file:{s}")
    };
    // hls, concat and the other list demuxers open the files they name: a crafted upload would read another account's.
    vec![
        "-protocol_whitelist".into(),
        "file,pipe".into(),
        "-format_whitelist".into(),
        FORMATS.into(),
        "-i".into(),
        named,
    ]
}

fn failed(m: String) -> FfmpegError {
    FfmpegError::Failed(AnalyseError::Failed(m))
}

/// `probe`: the first audio stream's rate and channels, and the length it claims.
fn probe(ffprobe: &str, source: &Path) -> Result<(f64, f64, f64), FfmpegError> {
    let mut args: Vec<String> = [
        "-v",
        "error",
        "-select_streams",
        "a:0",
        "-show_entries",
        "stream=sample_rate,channels,duration:format=duration",
        "-of",
        "json",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    args.extend(input(source));
    let out = match Command::new(ffprobe)
        .args(&args)
        .stdin(Stdio::null())
        .output()
    {
        Ok(o) => o,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(FfmpegError::Missing),
        Err(e) => return Err(failed(e.to_string())),
    };
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let err = err.trim();
        let tail: String = {
            let chars: Vec<char> = err.chars().collect();
            chars[chars.len().saturating_sub(500)..].iter().collect()
        };
        let code = out.status.code().map_or("null".into(), |c| c.to_string());
        return Err(failed(format!(
            "{ffprobe} failed: {}",
            if tail.is_empty() {
                format!("exit {code}")
            } else {
                tail
            }
        )));
    }
    let parsed: Value = serde_json::from_slice(&out.stdout).map_err(|e| failed(e.to_string()))?;
    let stream = &parsed["streams"][0];
    if !stream.is_object() {
        return Err(failed(format!("no audio stream in {}", source.display())));
    }
    let num = |v: &Value| match v {
        Value::String(s) => s.trim().parse::<f64>().unwrap_or(f64::NAN),
        Value::Number(n) => n.as_f64().unwrap_or(f64::NAN),
        _ => f64::NAN,
    };
    let rate = Some(num(&stream["sample_rate"]))
        .filter(|r| *r != 0.0 && !r.is_nan())
        .unwrap_or(48000.0);
    let channels = stream["channels"]
        .as_f64()
        .filter(|c| *c != 0.0)
        .unwrap_or(1.0);
    let duration = if stream["duration"].is_null() {
        &parsed["format"]["duration"]
    } else {
        &stream["duration"]
    };
    let seconds = Some(num(duration))
        .filter(|s| *s != 0.0 && !s.is_nan())
        .unwrap_or(0.0);
    Ok((rate, channels, (seconds * rate).ceil()))
}

/// `decodeWithFfmpeg`: the frames handed to `on_frames` whole, counted as they arrive.
pub fn decode(
    source: &Path,
    deadline: Instant,
    on_frames: &mut OnFrames<'_>,
) -> Result<Facts, FfmpegError> {
    if crate::programs::NONE {
        return Err(FfmpegError::Missing);
    }
    let ffprobe = bin("FFPROBE_BIN", "ffprobe");
    if MISSING.lock().unwrap_or_else(|e| e.into_inner()).as_deref() == Some(ffprobe.as_str()) {
        return Err(FfmpegError::Missing);
    }
    let probed = probe(&ffprobe, source);
    if matches!(probed, Err(FfmpegError::Missing)) {
        *MISSING.lock().unwrap_or_else(|e| e.into_inner()) = Some(ffprobe.clone());
    }
    let (sample_rate, channels, claimed) = probed?;
    if claimed > MAX_DECODE_FRAMES as f64 {
        too_long(source, claimed as u64).map_err(FfmpegError::Failed)?;
    }
    let ffmpeg = bin("FFMPEG_BIN", "ffmpeg");
    let mut args: Vec<String> = vec!["-v".into(), "error".into()];
    args.extend(input(source));
    args.extend(["-map", "0:a:0", "-f", "f32le", "-acodec", "pcm_f32le", "-"].map(String::from));
    let mut child = match Command::new(&ffmpeg)
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            *MISSING.lock().unwrap_or_else(|e| e.into_inner()) = Some(ffprobe);
            return Err(FfmpegError::Missing);
        }
        Err(e) => return Err(failed(e.to_string())),
    };
    let mut stderr = child.stderr.take();
    let errs = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(e) = stderr.as_mut() {
            let _ = e.read_to_string(&mut s);
        }
        s
    });
    let frame_bytes = channels as usize * 4;
    let mut stdout = child.stdout.take().expect("piped above");
    let mut carry: Vec<u8> = Vec::new();
    let mut buf = vec![0u8; 1 << 16];
    let mut frames: u64 = 0;
    let mut broke: Option<AnalyseError> = None;
    loop {
        if Instant::now() > deadline {
            broke = Some(AnalyseError::Failed(format!(
                "analysing {} took longer than {} s",
                source.display(),
                crate::analyse::ANALYSE_DEADLINE.as_secs()
            )));
            break;
        }
        let n = match stdout.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => {
                broke = Some(AnalyseError::Failed(e.to_string()));
                break;
            }
        };
        carry.extend_from_slice(&buf[..n]);
        let whole = carry.len() / frame_bytes;
        if whole == 0 {
            continue;
        }
        if let Err(e) = too_long(source, frames + whole as u64) {
            broke = Some(e);
            break;
        }
        let samples: Vec<f32> = carry[..whole * frame_bytes]
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| f32::from_le_bytes(*c))
            .collect();
        carry.drain(..whole * frame_bytes);
        if let Err(e) = on_frames(&samples, channels as i32) {
            broke = Some(e);
            break;
        }
        frames += whole as u64;
    }
    if broke.is_some() {
        let _ = child.kill();
    }
    let status = child.wait();
    let err = errs.join().unwrap_or_default();
    if let Some(e) = broke {
        return Err(FfmpegError::Failed(e));
    }
    match status {
        Ok(s) if s.success() => Ok(Facts {
            sample_rate,
            channels,
            frames: frames as f64,
        }),
        Ok(s) => {
            let code = s.code().map_or("null".into(), |c| c.to_string());
            let said = err.trim();
            Err(failed(if said.is_empty() {
                format!("{ffmpeg} could not decode {} ({code})", source.display())
            } else {
                said.to_string()
            }))
        }
        Err(e) => Err(failed(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 16-bit mono wav of `n` silent samples at 8 kHz.
    fn wav(n: u32) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend(b"RIFF");
        b.extend((36 + n * 2).to_le_bytes());
        b.extend(b"WAVEfmt ");
        b.extend(16u32.to_le_bytes());
        b.extend(1u16.to_le_bytes());
        b.extend(1u16.to_le_bytes());
        b.extend(8000u32.to_le_bytes());
        b.extend(16000u32.to_le_bytes());
        b.extend(2u16.to_le_bytes());
        b.extend(16u16.to_le_bytes());
        b.extend(b"data");
        b.extend((n * 2).to_le_bytes());
        b.resize(b.len() + (n * 2) as usize, 0);
        b
    }

    /// A playlist or a list of files is never opened, whatever it names; a plain wav still is.
    #[test]
    fn a_list_demuxer_is_never_opened() {
        for no in ["hls", "concat", "ffconcat", "image2", "dash", "tee"] {
            assert!(!FORMATS.split(',').any(|f| f == no), "{no}");
        }
        assert!(
            input(Path::new("a.wav"))
                .iter()
                .any(|a| a == "-format_whitelist")
        );
        let ffmpeg = bin("FFMPEG_BIN", "ffmpeg");
        if Command::new(&ffmpeg).arg("-version").output().is_err() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("k7-ffmpeg-formats-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.wav"), wav(800)).unwrap();
        std::fs::write(
            dir.join("list.ffconcat"),
            "ffconcat version 1.0\nfile a.wav\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("list.m3u8"),
            "#EXTM3U\n#EXTINF:0.1,\na.wav\n#EXT-X-ENDLIST\n",
        )
        .unwrap();
        let reads = |name: &str| {
            let mut args = vec!["-v".to_string(), "error".to_string()];
            args.extend(input(&dir.join(name)));
            args.extend(["-f", "null", "-"].map(String::from));
            Command::new(&ffmpeg)
                .args(&args)
                .stdin(Stdio::null())
                .output()
                .unwrap()
                .status
                .success()
        };
        assert!(reads("a.wav"));
        assert!(!reads("list.ffconcat"));
        assert!(!reads("list.m3u8"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
