// reads/misc.rs — the small rules the page asks of strings and numbers: a clip's shown name, the master's trim, a tap tempo, a folder's name.
// A reader would get wrong that words are not here: an error is a key (`required`, `noSlash`, `noDot`) the page spells with its messages.

use crate::clips::material_ref_of;
use serde::Serialize;

/// How a clip ref reads to a person: a take of a lane, a numbered sequence, or a name as written.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "camelCase")]
#[cfg_attr(feature = "ts", ts(tag = "kind", rename_all = "camelCase"))]
pub enum ClipNameParts {
    Take {
        lane: String,
        bar: f64,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "ts", ts(optional))]
        copy: Option<f64>,
    },
    Sequence {
        lane: String,
        n: f64,
    },
    Named {
        name: String,
    },
}

fn digits_end(s: &str) -> usize {
    s.len() - s.trim_end_matches(|c: char| c.is_ascii_digit()).len()
}

fn number(s: &str) -> f64 {
    s.parse().unwrap_or(f64::NAN)
}

/// `{lane}_{bar}.{beat}_{yymmdd}T{hhmmss}` without its copy suffix: the lane, the bar.
fn take_tail(s: &str) -> Option<(&str, &str)> {
    let b = s.as_bytes();
    let n = b.len();
    if n < 14 {
        return None;
    }
    let digits = |r: std::ops::Range<usize>| b[r].iter().all(u8::is_ascii_digit);
    if !(digits(n - 6..n) && b[n - 7] == b'T' && digits(n - 13..n - 7)) || b[n - 14] != b'_' {
        return None;
    }
    let head = &s[..n - 14];
    let beat = digits_end(head);
    if beat == 0 || !head[..head.len() - beat].ends_with('.') {
        return None;
    }
    let head = &head[..head.len() - beat - 1];
    let bar = digits_end(head);
    if bar == 0 || !head[..head.len() - bar].ends_with('_') {
        return None;
    }
    let lane = &head[..head.len() - bar - 1];
    (!lane.is_empty()).then_some((lane, &head[head.len() - bar..]))
}

/// Splits a clip ref (or a placement id, nested or not) into the parts its shown name is made of.
pub fn clip_name_parts(ref_: &str) -> ClipNameParts {
    let material = material_ref_of(ref_);
    let with_copy = material.rfind('-').and_then(|i| {
        let n = &material[i + 1..];
        let ok = !n.is_empty() && n.bytes().all(|c| c.is_ascii_digit());
        ok.then(|| take_tail(&material[..i]).map(|(lane, bar)| (lane, bar, Some(number(n))))).flatten()
    });
    if let Some((lane, bar, copy)) = with_copy.or_else(|| take_tail(material).map(|(l, b)| (l, b, None))) {
        return ClipNameParts::Take { lane: lane.to_owned(), bar: number(bar), copy };
    }
    let d = digits_end(material);
    if d > 0 {
        let head = &material[..material.len() - d];
        if let Some(lane) = head.strip_suffix("-seq-").filter(|l| !l.is_empty()) {
            return ClipNameParts::Sequence { lane: lane.to_owned(), n: number(&material[material.len() - d..]) };
        }
    }
    ClipNameParts::Named { name: material.to_owned() }
}

/// A strip's Volume range in dB: the floor and the top of the fader.
pub const VOLUME_DB_MIN: f64 = -60.0;
pub const VOLUME_DB_MAX: f64 = 6.0;

/// Where a fixed mix should peak: -6 dB gives the headroom back, and is where the meter already turns amber.
pub const FIX_TARGET_DB: f64 = -6.0;

/// How much the output has to come down for a pass that peaked at `peak_db`, to land at the target: never positive.
pub fn trim_for(peak_db: f64) -> f64 {
    (FIX_TARGET_DB - peak_db.max(0.0)).min(0.0)
}

/// The OUT node's new volume: relative to the meter reading downstream of this very gain, and clamped to the field.
pub fn fixed_master(master_db: f64, peak_db: f64, min: f64, max: f64) -> f64 {
    min.max(max.min(master_db + trim_for(peak_db)))
}

/// A gap past this starts a new run: 2 s is a 30 bpm beat, the slowest a person taps steadily.
const TAP_RESET_MS: f64 = 2000.0;
/// The average covers the last few taps only.
const TAP_WINDOW: usize = 8;

/// The run after a tap, and its tempo in whole bpm once a second tap gives an interval.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TapRun {
    pub taps: Vec<f64>,
    pub bpm: Option<f64>,
}

pub fn tap_tempo(taps: &[f64], now: f64) -> TapRun {
    let continues = taps.last().is_some_and(|&last| now - last <= TAP_RESET_MS && now > last);
    let run: Vec<f64> = if continues {
        let mut all = taps.to_vec();
        all.push(now);
        all[all.len().saturating_sub(TAP_WINDOW)..].to_vec()
    } else {
        vec![now]
    };
    if run.len() < 2 {
        return TapRun { taps: run, bpm: None };
    }
    let mean = (run[run.len() - 1] - run[0]) / (run.len() - 1) as f64;
    let bpm = (60000.0 / mean + 0.5).floor();
    TapRun { taps: run, bpm: Some(bpm) }
}

/// The folder a name makes: the stem, or the key of what is wrong with it.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FolderName {
    pub folder: String,
    pub error: String,
}

/// The name rules shared by every dialog: separators are refused rather than replaced, a leading dot hides, a typed suffix is stripped.
pub fn folder_name(raw: &str, suffixes: &[String]) -> FolderName {
    let bad = |key: &str| FolderName { folder: String::new(), error: key.to_owned() };
    let name = raw.trim();
    if name.is_empty() {
        return bad("required");
    }
    if name.contains(['/', '\\']) {
        return bad("noSlash");
    }
    if name.starts_with('.') {
        return bad("noDot");
    }
    let mut stem = name;
    for suffix in suffixes {
        let cut = stem.len().saturating_sub(suffix.len());
        if stem.len() > suffix.len() && stem.to_lowercase().ends_with(suffix.as_str()) && stem.is_char_boundary(cut) {
            stem = &stem[..cut];
        }
    }
    if stem.is_empty() {
        return bad("required");
    }
    FolderName { folder: stem.to_owned(), error: String::new() }
}

/// The open timeline once the ids changed: kept, or followed when it alone vanished as one alone appeared.
pub fn follow_rename(open: Option<&str>, before: &[String], after: &[String]) -> Option<String> {
    let open = open.filter(|o| !o.is_empty())?;
    if after.iter().any(|a| a == open) {
        return Some(open.to_owned());
    }
    let added: Vec<&String> = after.iter().filter(|id| !before.contains(id)).collect();
    let gone: Vec<&String> = before.iter().filter(|id| !after.contains(id)).collect();
    (gone.len() == 1 && gone[0] == open && added.len() == 1).then(|| added[0].clone())
}

/// The archetype a project's surfaces read as, or none when the service is too old to say.
pub fn shape_of_surfaces(surfaces: Option<&[String]>) -> Option<&'static str> {
    let s = surfaces?;
    let has = |t: &str| s.iter().any(|x| x == t);
    Some(if has("turntables") {
        "mix"
    } else if has("timeline") {
        if has("viewport") { "video" } else { "arrangement" }
    } else if has("viewport") {
        "live-video"
    } else {
        "live"
    })
}
