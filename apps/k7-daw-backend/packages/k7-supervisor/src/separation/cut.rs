//! cut.rs — one separation from the mix to the written stems, on a thread the job gave it. The rationale for every
//! step is `docs/separation.md`. What a reader gets wrong: a failed window is retried on a FRESH session before the
//! cut gives up on its provider, since a lost device kills the session rather than the GPU.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::json;

use super::audio::{
    CHANNELS, Channels, OverlapAdd, SAMPLE_RATE, SEGMENT, STEMS, output_channels, peak_of,
    wav_pcm24, window_starts,
};
use super::decode::{decode_for_model, resample};
use super::job::{Job, Request, mark_lost};
use super::onnx::OnnxSession;

/// One window's worth of the mix, zero-padded when the file is shorter than a window.
fn window_of(chans: &Channels, at: usize) -> Vec<f32> {
    let mut mix = vec![0.0; CHANNELS * SEGMENT];
    for (c, src) in chans.iter().enumerate() {
        let n = src.len().saturating_sub(at).min(SEGMENT);
        mix[c * SEGMENT..c * SEGMENT + n].copy_from_slice(&src[at..at + n]);
    }
    mix
}

/// A phone refuses a cut it cannot hold, up front: iOS kills the app past its limit with no word.
#[cfg(target_os = "ios")]
fn require_memory(model: &Path, frames: usize) -> Result<(), String> {
    use super::budget::{bytes_needed, memory_refusal};
    unsafe extern "C" {
        fn os_proc_available_memory() -> usize;
    }
    let model_bytes = std::fs::metadata(model).map_or(0, |m| m.len() as usize);
    let floor = std::env::var("STUDIO_SEPARATION_MIN_MEMORY_MB")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .map_or(0, |mb| mb << 20);
    // The simulator reports none: its host's memory stands in, which a device never needs.
    let mut available = unsafe { os_proc_available_memory() };
    if available == 0 {
        let (pages, size) = unsafe {
            (
                libc::sysconf(libc::_SC_PHYS_PAGES),
                libc::sysconf(libc::_SC_PAGESIZE),
            )
        };
        available = pages.max(0) as usize * size.max(0) as usize;
    }
    match memory_refusal(available, bytes_needed(model_bytes, frames), floor) {
        Some(why) => Err(why),
        None => Ok(()),
    }
}

#[cfg(not(target_os = "ios"))]
fn require_memory(_model: &Path, _frames: usize) -> Result<(), String> {
    Ok(())
}

/// One output AT the record's rate, resampled when the model's differs.
fn write_stem(path: &Path, chans: &Channels, to: u32) -> Result<(), String> {
    let bytes = if to == SAMPLE_RATE {
        wav_pcm24(chans, SAMPLE_RATE)
    } else {
        wav_pcm24(&resample(chans, SAMPLE_RATE, to)?, to)
    };
    std::fs::write(path, bytes).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

/// The first line of an error, which is what a window's failure says.
fn first_line(e: &str) -> &str {
    e.lines().next().unwrap_or(e)
}

pub(super) fn separate(
    job: &Arc<Job>,
    r: &Request,
    outputs: &[String],
    providers: &[String],
) -> Result<(), String> {
    let model = PathBuf::from(&r.model);
    require_memory(&model, 0)?;
    job.say("decoding", None, None);
    let decoded = decode_for_model(Path::new(&r.source))?;
    let chans = decoded.channels;
    let length = chans.first().map_or(0, Vec::len);
    if length == 0 {
        return Err(format!("{} decoded to no audio", r.source));
    }
    require_memory(&model, length)?;
    job.say("loading model", None, None);
    let mut session = Arc::new(OnnxSession::open(&model, providers)?);
    job.set_session(session.clone());
    let skipped = if session.skipped().is_empty() {
        String::new()
    } else {
        format!(" — skipped {}", session.skipped())
    };
    job.say(
        "separating",
        Some(0),
        Some(format!(
            "execution provider: {}{}{skipped}",
            session.provider(),
            session.adapter()
        )),
    );

    let starts = window_starts(length);
    let mut oa = OverlapAdd::new(length, STEMS.len());
    let mut retried = false;
    for (i, &at) in starts.iter().enumerate() {
        let mix = window_of(&chans, at);
        let stems = loop {
            if job.is_done() {
                return Err("the cut was ended".into());
            }
            match session.run(&mix) {
                Ok(stems) => break stems,
                Err(e) => {
                    if session.provider() == "cpu" {
                        return Err(e);
                    }
                    // A device lost kills the SESSION rather than the GPU, so the first answer is a fresh one.
                    let provider = session.provider().to_string();
                    let next = if retried {
                        "cpu".to_string()
                    } else {
                        provider.clone()
                    };
                    if retried {
                        mark_lost(&provider);
                    }
                    job.say(
                        "separating",
                        Some((i * 100 / starts.len()) as u32),
                        Some(format!(
                            "{provider} failed at window {}/{}, {} — {}",
                            i + 1,
                            starts.len(),
                            if retried {
                                "finishing on cpu"
                            } else {
                                "retrying on a fresh session"
                            },
                            first_line(&e)
                        )),
                    );
                    retried = true;
                    session = Arc::new(OnnxSession::open(&model, &[next])?);
                    job.set_session(session.clone());
                }
            }
        };
        // `[1, 4, 2, SEGMENT]`, flat, the stem index outermost: STEMS is that order, not a guess.
        if stems.len() < STEMS.len() * CHANNELS * SEGMENT {
            return Err(format!(
                "the model answered {} values, not {}",
                stems.len(),
                STEMS.len() * CHANNELS * SEGMENT
            ));
        }
        for s in 0..STEMS.len() {
            for c in 0..CHANNELS {
                let from = (s * CHANNELS + c) * SEGMENT;
                oa.add(at, s, c, &stems[from..from + SEGMENT]);
            }
        }
        oa.count_window(at);
        job.say(
            "separating",
            Some(((i + 1) * 100 / starts.len()) as u32),
            None,
        );
    }

    job.say("writing", Some(100), None);
    let out_dir = PathBuf::from(&r.out_dir);
    std::fs::create_dir_all(&out_dir)
        .map_err(|e| format!("cannot create {}: {e}", out_dir.display()))?;
    let rate = if decoded.source_rate > 0 {
        decoded.source_rate
    } else {
        SAMPLE_RATE
    };
    if rate != SAMPLE_RATE {
        job.say(
            "writing",
            Some(100),
            Some(format!(
                "writing at {rate} Hz, the record's own — the model works at {SAMPLE_RATE}"
            )),
        );
    }
    let mut files = Vec::new();
    for name in outputs {
        let path = out_dir.join(format!("{name}.wav"));
        let out = output_channels(&oa, name);
        // An overshoot is CLAMPED and SAID: the other way to find out is a click.
        let peak = peak_of(&out);
        if peak > 1.0 {
            job.say(
                "writing",
                Some(100),
                Some(format!("{name}: peak {peak:.3} — clamped to full scale")),
            );
        }
        write_stem(&path, &out, rate)?;
        files.push(path.to_string_lossy().into_owned());
    }
    job.finish(json!({"files": files, "phase": "done", "percent": 100}));
    Ok(())
}
