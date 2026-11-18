//! decode.rs — a file decoded by the engine's `--decode`, as `pump` in `packages/audio-analysis/src/decode.ts` reads
//! it: one JSON line of facts, then every channel as interleaved f32le at the file's rate, counted as it arrives.
//! What a reader gets wrong: the header's `frames` refuses a file too long before decoding, never sizes the result.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::analyse::{ANALYSE_DEADLINE, AnalyseError, Stop, too_long};
use crate::envelope::Facts;
use crate::ffmpeg::OnFrames;

fn failed(m: impl Into<String>) -> AnalyseError {
    AnalyseError::Failed(m.into())
}

/// `parseHeader`: the rate and the channels the engine states, refused when either is not a positive number.
fn header(line: &str) -> Result<(f64, i32, f64), AnalyseError> {
    let v: Value = serde_json::from_str(line)
        .map_err(|_| failed(format!("the engine's decode header is not JSON: {line}")))?;
    let num = |k: &str| v.get(k).and_then(Value::as_f64).unwrap_or(f64::NAN);
    let (rate, channels) = (num("sampleRate"), num("channels"));
    if !(rate > 0.0 && channels >= 1.0) {
        return Err(failed(format!(
            "the engine's decode header names no stream: {line}"
        )));
    }
    Ok((rate, channels as i32, num("frames")))
}

/// How long the engine's stderr is kept: what a failed decode says is at its start.
const STDERR_KEPT: usize = 64 * 1024;

/// How often a decode that writes nothing looks at its deadline and its stop.
const TICK: Duration = Duration::from_millis(50);

/// How long a reaped child's stderr is waited for.
const ERR_GRACE: Duration = Duration::from_secs(2);

/// The child's exit, polled each tick against the deadline and the stop; an error when either ends the wait.
fn reap_by(
    child: &mut std::process::Child,
    source: &Path,
    deadline: Instant,
    stop: Stop<'_>,
) -> Result<std::process::ExitStatus, AnalyseError> {
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => {}
            Err(e) => return Err(failed(e.to_string())),
        }
        if Instant::now() > deadline {
            return Err(failed(format!(
                "analysing {} took longer than {} s",
                source.display(),
                ANALYSE_DEADLINE.as_secs()
            )));
        }
        if stop() {
            return Err(crate::analyse::aborted(source));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// The engine to run and what it runs under: `env` None inherits this process's, Some is the whole environment (2416).
pub struct Engine<'a> {
    pub bin: &'a Path,
    pub env: Option<&'a [(String, String)]>,
}

/// A child killed and reaped when the decode ends any way, a dropped task included.
struct Reaped(std::process::Child);

impl Drop for Reaped {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// `pump(spawn(engineBin, ["--decode", source]))` as the platform inherits its environment; no stop but the deadline.
pub fn engine(
    bin: &Path,
    source: &Path,
    deadline: Instant,
    on_frames: &mut OnFrames<'_>,
) -> Result<Facts, AnalyseError> {
    engine_in(
        &Engine { bin, env: None },
        source,
        deadline,
        &|| false,
        on_frames,
    )
}

/// The engine's decode under a watchdog (2416): the frames handed to `on_frames` whole; an error of `on_frames`, the
/// deadline or `stop` kills the child whatever it is writing, and its stderr keeps 64 KiB.
pub fn engine_in(
    run: &Engine<'_>,
    source: &Path,
    deadline: Instant,
    stop: Stop<'_>,
    on_frames: &mut OnFrames<'_>,
) -> Result<Facts, AnalyseError> {
    if crate::programs::NONE {
        return Err(failed(format!(
            "the engine's --decode cannot run: {}",
            crate::programs::WHY
        )));
    }
    let mut cmd = Command::new(run.bin);
    cmd.arg("--decode").arg(source);
    if let Some(env) = run.env {
        cmd.env_clear().envs(env.iter().map(|(k, v)| (k, v)));
    }
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| failed(format!("{}: {e}", run.bin.display())))?;
    let mut stderr = child.stderr.take();
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| failed("the engine's decode has no output"))?;
    let child = Reaped(child);
    let (err_tx, err_rx) = std::sync::mpsc::sync_channel::<String>(1);
    std::thread::spawn(move || {
        let mut kept = Vec::new();
        let mut buf = [0u8; 8192];
        while let Some(Ok(n)) = stderr.as_mut().map(|e| e.read(&mut buf)) {
            if n == 0 {
                break;
            }
            // The rest is read and dropped: a child that fills its pipe would block for ever.
            kept.extend_from_slice(&buf[..n.min(STDERR_KEPT.saturating_sub(kept.len()))]);
        }
        let _ = err_tx.send(String::from_utf8_lossy(&kept).into_owned());
    });
    // The pipe is read on a thread of its own, so the watchdog below never waits on a `read`.
    let (tx, rx) = std::sync::mpsc::sync_channel::<std::io::Result<Vec<u8>>>(8);
    std::thread::spawn(move || {
        let mut buf = vec![0u8; 1 << 16];
        loop {
            let sent = match stdout.read(&mut buf) {
                Ok(0) => return,
                Ok(n) => tx.send(Ok(buf[..n].to_vec())),
                Err(e) => tx.send(Err(e)),
            };
            if sent.is_err() {
                return;
            }
        }
    });
    let mut head: Option<(f64, i32)> = None;
    let mut carry: Vec<u8> = Vec::new();
    let mut frames: u64 = 0;
    let mut broke: Option<AnalyseError> = None;
    let mut exited: Option<std::process::ExitStatus> = None;
    let mut child = child;
    loop {
        if Instant::now() > deadline {
            broke = Some(failed(format!(
                "analysing {} took longer than {} s",
                source.display(),
                ANALYSE_DEADLINE.as_secs()
            )));
            break;
        }
        if stop() {
            broke = Some(crate::analyse::aborted(source));
            break;
        }
        let chunk = match rx.recv_timeout(TICK) {
            Ok(Ok(c)) => c,
            Ok(Err(e)) => {
                broke = Some(failed(e.to_string()));
                break;
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                // The output is closed, not the child: it is waited for under the same deadline and stop.
                match reap_by(&mut child.0, source, deadline, stop) {
                    Ok(status) => exited = Some(status),
                    Err(e) => broke = Some(e),
                }
                break;
            }
        };
        carry.extend_from_slice(&chunk);
        if head.is_none() {
            let Some(nl) = carry.iter().position(|b| *b == b'\n') else {
                continue;
            };
            let line = String::from_utf8_lossy(&carry[..nl]).into_owned();
            match header(&line) {
                Ok((rate, channels, said)) => {
                    if said.is_finite() && said > crate::analyse::MAX_DECODE_FRAMES as f64 {
                        broke = too_long(source, said as u64).err();
                        break;
                    }
                    head = Some((rate, channels));
                }
                Err(e) => {
                    broke = Some(e);
                    break;
                }
            }
            carry.drain(..=nl);
        }
        let Some((_, channels)) = head else { continue };
        let frame_bytes = channels as usize * 4;
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
        if let Err(e) = on_frames(&samples, channels) {
            broke = Some(e);
            break;
        }
        frames += whole as u64;
    }
    if let Some(e) = broke {
        // Killed here and the threads left to finish on their own: a grandchild may hold the pipes open.
        drop(child);
        return Err(e);
    }
    // A grandchild may hold stderr open after the child is gone, so its text is waited for briefly.
    let err = err_rx.recv_timeout(ERR_GRACE).unwrap_or_default();
    let code = exited.and_then(|s| s.code());
    match (code, head) {
        (Some(0), Some((sample_rate, channels))) => Ok(Facts {
            sample_rate,
            channels: f64::from(channels),
            frames: frames as f64,
        }),
        _ => {
            let said = err.trim();
            let code = code.map_or_else(|| "null".to_string(), |c| c.to_string());
            Err(failed(if said.is_empty() {
                format!(
                    "{} could not decode {} ({code})",
                    run.bin.display(),
                    source.display()
                )
            } else {
                said.to_string()
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_engines_header_as_parse_header_does() {
        assert_eq!(
            header(r#"{"sampleRate":48000,"channels":2,"frames":96000}"#).unwrap(),
            (48000.0, 2, 96000.0)
        );
        assert!(header(r#"{"sampleRate":0,"channels":2}"#).is_err());
        assert!(header("not json").is_err());
    }
}
