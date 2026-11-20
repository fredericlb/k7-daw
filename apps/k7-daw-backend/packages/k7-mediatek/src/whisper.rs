//! whisper.rs — the pure half of `track transcribe`: each CLI's arguments and output file, the audio extract's arguments, and the SRT read and written.
//! What a reader gets wrong: pure; `host-whisper.json` pins every string and every JavaScript rounding spelled here, the spawn is the front's.

use k7_js::number::math_round;

use crate::video_copy::input;

/// The CLIs tried, in order: openai-whisper, then whisper.cpp.
pub const BINS: [&str; 2] = ["whisper", "whisper-cli"];

/// One spoken segment, in the SOURCE video's own seconds.
#[derive(Clone, Debug, PartialEq)]
pub struct Segment {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

fn stem_of(wav: &str) -> String {
    let name = wav.rsplit('/').next().unwrap_or(wav);
    let lower = name.to_ascii_lowercase();
    match lower.strip_suffix(".wav") {
        Some(_) => name[..name.len() - 4].to_string(),
        None => name.to_string(),
    }
}

fn below(dir: &str, name: &str) -> String {
    format!("{}/{name}", dir.trim_end_matches('/'))
}

/// `extractAudioArgs`: the 16 kHz mono wav whisper reads, the video opened under the whitelist.
pub fn extract_audio_args(video: &str, wav: &str) -> Vec<String> {
    let mut args: Vec<String> = ["-y", "-hide_banner", "-loglevel", "error"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    args.extend(input(video));
    args.extend(
        ["-vn", "-ac", "1", "-ar", "16000"]
            .iter()
            .map(|s| s.to_string()),
    );
    args.push(wav.to_string());
    args
}

/// A CLI's arguments; whisper.cpp refuses to run without a model.
///
/// # Errors
/// whisper.cpp with no model, or a name that is not one of `BINS`.
pub fn cli_args(
    bin: &str,
    wav: &str,
    out_dir: &str,
    model: Option<&str>,
) -> Result<Vec<String>, String> {
    let model = model.filter(|m| !m.is_empty());
    let strs = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    match bin {
        "whisper" => {
            let mut a = strs(&[wav, "--output_format", "srt", "--output_dir", out_dir]);
            if let Some(m) = model {
                a.extend(strs(&["--model", m]));
            }
            Ok(a)
        }
        "whisper-cli" => {
            let m = model.ok_or("whisper-cli needs a model — set STUDIO_WHISPER_MODEL to a ggml file (e.g. models/ggml-base.bin)")?;
            let mut a = strs(&["-f", wav, "-m", m, "-l", "auto", "-osrt", "-of"]);
            a.push(below(out_dir, "transcribe"));
            Ok(a)
        }
        other => Err(format!("not a whisper CLI: {other}")),
    }
}

/// Where a CLI leaves its SRT.
pub fn srt_path(bin: &str, wav: &str, out_dir: &str) -> String {
    match bin {
        "whisper-cli" => below(out_dir, "transcribe.srt"),
        _ => below(out_dir, &format!("{}.srt", stem_of(wav))),
    }
}

fn digits(s: &str, at: usize, min: usize, max: usize) -> Option<(f64, usize)> {
    let n = s[at..]
        .bytes()
        .take_while(u8::is_ascii_digit)
        .count()
        .min(max);
    (n >= min).then(|| (s[at..at + n].parse().unwrap_or(0.0), at + n))
}

fn expect(s: &str, at: usize, ch: char) -> Option<usize> {
    s[at..].starts_with(ch).then_some(at + 1)
}

/// One clock, `HH:MM:SS` then `,` or `.` and 1 to 3 digits of milliseconds; the hours may run past two digits.
fn clock(s: &str, at: usize) -> Option<(f64, usize)> {
    let (h, at) = digits(s, at, 2, usize::MAX)?;
    let (m, at) = digits(s, expect(s, at, ':')?, 2, 2)?;
    let (sec, at) = digits(s, expect(s, at, ':')?, 2, 2)?;
    let dot = s[at..].chars().next().filter(|c| matches!(c, ',' | '.'))?;
    let (ms, end) = digits(s, at + dot.len_utf8(), 1, 3)?;
    let width = end - (at + 1);
    let ms = ms * 10f64.powi(3 - i32::try_from(width).unwrap_or(3));
    Some((h * 3600.0 + m * 60.0 + sec + ms / 1000.0, end))
}

fn skip_space(s: &str, at: usize) -> usize {
    at + s[at..].len() - s[at..].trim_start().len()
}

/// A cue's timing line, `00:01:02,480 --> 00:01:05,120`, or none.
fn timing(line: &str) -> Option<(f64, f64)> {
    let (start, at) = clock(line, 0)?;
    let at = skip_space(line, at);
    let at = line[at..].starts_with("-->").then_some(at + 3)?;
    let (end, _) = clock(line, skip_space(line, at))?;
    Some((start, end))
}

fn blank(l: &str) -> bool {
    l.trim_matches(|c: char| c.is_whitespace() || c == '\u{feff}')
        .is_empty()
}

/// The segments an SRT text holds, parsed defensively: a cue is its timing line plus what follows. Empty cues are dropped.
pub fn parse_srt(text: &str) -> Vec<Segment> {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut out = Vec::new();
    let mut block: Vec<&str> = Vec::new();
    let mut cue = |block: &mut Vec<&str>| {
        let lines: Vec<&str> = block.drain(..).filter(|l| !blank(l)).collect();
        let Some(at) = lines.iter().position(|l| timing(l).is_some()) else {
            return;
        };
        let Some((start, end)) = timing(lines[at]) else {
            return;
        };
        let content = lines[at + 1..].join(" ");
        let content = content.trim_matches(|c: char| c.is_whitespace() || c == '\u{feff}');
        if !content.is_empty() {
            out.push(Segment {
                start,
                end,
                text: content.to_string(),
            });
        }
    };
    for line in text.split('\n') {
        if line.is_empty() {
            cue(&mut block);
        } else {
            block.push(line);
        }
    }
    cue(&mut block);
    out
}

fn srt_time(t: f64) -> String {
    let ms = math_round(t * 1000.0).max(0.0) as u64;
    format!(
        "{:02}:{:02}:{:02},{:03}",
        ms / 3_600_000,
        (ms % 3_600_000) / 60_000,
        (ms % 60_000) / 1000,
        ms % 1000
    )
}

/// `srtOf`: valid SRT, each block ending with its newline and joined by one more.
pub fn srt_of(segments: &[Segment]) -> String {
    segments
        .iter()
        .enumerate()
        .map(|(i, s)| {
            format!(
                "{}\n{} --> {}\n{}\n",
                i + 1,
                srt_time(s.start),
                srt_time(s.end),
                s.text
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
