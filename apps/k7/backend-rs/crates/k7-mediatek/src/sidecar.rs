//! sidecar.rs — the `{file}.cache` beside an audio file: the fields in their order at their fixed precision (`toFixed`), and a version below 4 refused.
//! What a reader gets wrong: the text is the format, and what a sidecar holds on disk is read back as written.

use std::path::{Path, PathBuf};

use k7_js::number::{math_round, to_fixed, to_string};
use serde_json::Value;

use crate::fingerprint::file_fingerprint;

/// `CACHE_VERSION`: v2 added onsets, v3 the tempo, v4 the three bands.
pub const CACHE_VERSION: u32 = 4;
const MIN_CACHE_VERSION: f64 = 4.0;

/// `DEFAULT_TRANSIENT_SENSITIVITY`: standard deviations above the mean jump.
pub const DEFAULT_SENSITIVITY: f64 = 2.0;

/// `FileEnvelope`: a file's measure, as an analysis made it or a sidecar holds it.
#[derive(Clone, Debug, PartialEq)]
pub struct Envelope {
    pub fingerprint: String,
    pub sample_rate: f64,
    pub channels: f64,
    pub frames: f64,
    pub seconds: f64,
    pub peak: Vec<f64>,
    pub rms: Vec<f64>,
    pub transients: Vec<f64>,
    pub sensitivity: f64,
    pub bpm: f64,
    /// Absent, not empty, for a sidecar written without them: `[]` would draw silence.
    pub low: Option<Vec<f64>>,
    pub mid: Option<Vec<f64>>,
    pub high: Option<Vec<f64>>,
}

/// `kick.wav` -> `kick.wav.cache`, beside the audio so it travels with the file.
pub fn cache_path(source: &Path) -> PathBuf {
    let mut name = source.as_os_str().to_owned();
    name.push(".cache");
    PathBuf::from(name)
}

fn fixed(out: &mut String, xs: &[f64], places: usize) {
    for (i, v) in xs.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&to_fixed(*v, places));
    }
}

/// `serialiseSidecar`: the sidecar's bytes.
pub fn serialise(env: &Envelope) -> String {
    let mut s = format!(
        "{{\"v\":{CACHE_VERSION},\"fingerprint\":\"{}\",\"sampleRate\":{},\"channels\":{},\"frames\":{},\"seconds\":{}",
        env.fingerprint,
        to_string(math_round(env.sample_rate)),
        to_string(env.channels),
        to_string(env.frames),
        to_fixed(env.seconds, 6)
    );
    let none = Vec::new();
    for (key, xs) in [
        ("peak", &env.peak),
        ("rms", &env.rms),
        ("low", env.low.as_ref().unwrap_or(&none)),
        ("mid", env.mid.as_ref().unwrap_or(&none)),
        ("high", env.high.as_ref().unwrap_or(&none)),
    ] {
        s.push_str(&format!(",\"{key}\":["));
        fixed(&mut s, xs, 4);
        s.push(']');
    }
    s.push_str(&format!(
        ",\"bpm\":{},\"sensitivity\":{},\"transients\":[",
        to_fixed(env.bpm, 2),
        to_fixed(env.sensitivity, 4)
    ));
    fixed(&mut s, &env.transients, 6);
    s.push_str("]}");
    s
}

/// `writeSidecar`: written in place, as `writeFileSync` writes it.
pub fn write(source: &Path, env: &Envelope) -> std::io::Result<()> {
    std::fs::write(cache_path(source), serialise(env))
}

fn numbers(v: &Value) -> Vec<f64> {
    v.as_array()
        .map(|a| a.iter().map(|x| x.as_f64().unwrap_or(f64::NAN)).collect())
        .unwrap_or_default()
}

/// A field as `c.x ?? fallback` reads it, a number being the only value the writers put there.
fn or(v: &Value, fallback: f64) -> f64 {
    if v.is_null() {
        fallback
    } else {
        v.as_f64().unwrap_or(f64::NAN)
    }
}

/// A sidecar's text as `readSidecar` parses it: none for malformed or too old.
fn parse(text: &str) -> Option<Envelope> {
    envelope_of(&serde_json::from_str(text).ok()?)
}

/// The envelope a parsed sidecar holds, by `readSidecar`'s rules.
fn envelope_of(c: &Value) -> Option<Envelope> {
    if !c.is_object() {
        return None;
    }
    let ok = or(&c["v"], 1.0) >= MIN_CACHE_VERSION
        && c["fingerprint"].is_string()
        && c["peak"].is_array()
        && c["rms"].is_array();
    if !ok {
        return None;
    }
    let band = |k: &str| (!c[k].is_null()).then(|| numbers(&c[k]));
    Some(Envelope {
        fingerprint: c["fingerprint"].as_str().unwrap_or("").to_string(),
        sample_rate: or(&c["sampleRate"], 48000.0),
        channels: or(&c["channels"], 1.0),
        frames: or(&c["frames"], 0.0),
        seconds: or(&c["seconds"], 0.0),
        peak: numbers(&c["peak"]),
        rms: numbers(&c["rms"]),
        transients: numbers(&c["transients"]),
        sensitivity: or(&c["sensitivity"], DEFAULT_SENSITIVITY),
        bpm: or(&c["bpm"], 0.0),
        low: band("low"),
        mid: band("mid"),
        high: band("high"),
    })
}

/// `readSidecar`: whatever is on disk beside `source`, or None for missing, unreadable, malformed or too old.
pub fn read(source: &Path) -> Option<Envelope> {
    parse(&std::fs::read_to_string(cache_path(source)).ok()?)
}

/// What a sidecar on disk says: its envelope when it is a current one, and the threshold it carries either way.
#[derive(Clone, Debug, PartialEq)]
pub struct Checked {
    pub env: Option<Envelope>,
    pub stored: Option<f64>,
}

/// `readSidecar` that tells a stale sidecar (missing, a link, not UTF-8, not JSON, too old) from an I/O error (2415).
pub fn check_read(source: &Path) -> std::io::Result<Checked> {
    use std::io::ErrorKind::{InvalidData, NotFound};
    let at = cache_path(source);
    let stale = Checked {
        env: None,
        stored: None,
    };
    match std::fs::symlink_metadata(&at) {
        Err(e) if e.kind() == NotFound => return Ok(stale),
        Err(e) => return Err(e),
        // A link is never read: whatever it names may lie outside the library, and the next write replaces it.
        Ok(m) if m.file_type().is_symlink() => return Ok(stale),
        Ok(_) => {}
    }
    let text = match std::fs::read_to_string(&at) {
        Ok(t) => t,
        Err(e) if matches!(e.kind(), NotFound | InvalidData) => return Ok(stale),
        Err(e) => return Err(e),
    };
    let parsed = serde_json::from_str::<Value>(&text).ok();
    let stored = parsed
        .as_ref()
        .and_then(|v| v.get("sensitivity").and_then(Value::as_f64));
    Ok(Checked {
        env: parsed.as_ref().and_then(envelope_of),
        stored,
    })
}

/// `read` for a caller that must not take an unreadable sidecar for a stale one (2415).
pub fn read_checked(source: &Path) -> std::io::Result<Option<Envelope>> {
    check_read(source).map(|c| c.env)
}

/// `writeSidecar` made whole (2414): written new beside the source as `<source>.<8 hex>.cache`, renamed over the sidecar's
/// name, removed on any failure. A reader never sees half a sidecar, and a link at the final name is replaced, not written through.
pub fn write_atomic(source: &Path, env: &Envelope) -> std::io::Result<()> {
    use std::io::Write;
    let mut raw = [0u8; 4];
    getrandom::fill(&mut raw).map_err(|e| std::io::Error::other(e.to_string()))?;
    let hex: String = raw.iter().map(|b| format!("{b:02x}")).collect();
    let mut name = source.as_os_str().to_owned();
    name.push(format!(".{hex}.cache"));
    let tmp = PathBuf::from(name);
    let done = (|| {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;
        f.write_all(serialise(env).as_bytes())?;
        f.flush()?;
        drop(f);
        std::fs::rename(&tmp, cache_path(source))
    })();
    if done.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    done
}

/// `sidecarPresent`: `read(source).is_some()` without parsing the arrays, off the writers' own head and last byte.
pub fn present(source: &Path) -> bool {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut f) = std::fs::File::open(cache_path(source)) else {
        return false;
    };
    let Ok(size) = f.metadata().map(|m| m.len()) else {
        return false;
    };
    let mut head = vec![0u8; size.min(64) as usize];
    if f.read_exact(&mut head).is_err() {
        return false;
    }
    let Some(version) = head_version(&head) else {
        // Not the writers' own shape, a hand-edited file say: the full parse decides.
        return read(source).is_some();
    };
    let mut last = [0u8; 1];
    if f.seek(SeekFrom::Start(size - 1)).is_err() || f.read_exact(&mut last).is_err() {
        return false;
    }
    version >= MIN_CACHE_VERSION && last[0] == b'}'
}

/// `/^\{"v":(\d+),"fingerprint":"/` over the head, read as latin1: the version's digits as a number.
fn head_version(head: &[u8]) -> Option<f64> {
    let rest = head.strip_prefix(b"{\"v\":")?;
    let digits = rest.iter().take_while(|b| b.is_ascii_digit()).count();
    if digits == 0 || !rest[digits..].starts_with(b",\"fingerprint\":\"") {
        return None;
    }
    std::str::from_utf8(&rest[..digits]).ok()?.parse().ok()
}

/// `freshSidecar`: the sidecar while it still describes these bytes at this sensitivity; the error is the file's.
pub fn fresh(source: &Path, sensitivity: f64) -> Result<Option<Envelope>, String> {
    let Some(have) = read(source) else {
        return Ok(None);
    };
    let now = file_fingerprint(source).map_err(|e| e.to_string())?;
    Ok((have.fingerprint == now && have.sensitivity == sensitivity).then_some(have))
}

/// `fresh` with the I/O errors told apart (2415): a stale sidecar is `Ok(None)`, an unreadable one an error.
pub fn fresh_checked(source: &Path, sensitivity: f64) -> Result<Option<Envelope>, String> {
    let Some(have) = read_checked(source).map_err(|e| e.to_string())? else {
        return Ok(None);
    };
    let now = file_fingerprint(source).map_err(|e| e.to_string())?;
    Ok((have.fingerprint == now && have.sensitivity == sensitivity).then_some(have))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env() -> Envelope {
        Envelope {
            fingerprint: "4-0123456789abcdef".into(),
            sample_rate: 8000.0,
            channels: 1.0,
            frames: 32000.0,
            seconds: 4.0,
            peak: vec![0.03125, 1.0],
            rms: vec![0.5, 0.0],
            transients: vec![0.5],
            sensitivity: 2.0,
            bpm: 120.0,
            low: Some(vec![0.1]),
            mid: None,
            high: Some(vec![]),
        }
    }

    #[test]
    fn writes_the_fields_in_order_and_reads_them_back() {
        let text = serialise(&env());
        assert_eq!(
            text,
            "{\"v\":4,\"fingerprint\":\"4-0123456789abcdef\",\"sampleRate\":8000,\"channels\":1,\"frames\":32000,\"seconds\":4.000000,\"peak\":[0.0313,1.0000],\"rms\":[0.5000,0.0000],\"low\":[0.1000],\"mid\":[],\"high\":[],\"bpm\":120.00,\"sensitivity\":2.0000,\"transients\":[0.500000]}"
        );
        let dir = std::env::temp_dir().join(format!("k7-sidecar-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("a.wav");
        std::fs::write(&src, b"RIFF").unwrap();
        assert!(!present(&src));
        write(&src, &env()).unwrap();
        assert!(present(&src));
        let back = read(&src).unwrap();
        assert_eq!(back.peak, vec![0.0313, 1.0]);
        assert_eq!(back.mid, Some(vec![]));
        std::fs::write(cache_path(&src), &text[..text.len() - 3]).unwrap();
        assert!(!present(&src));
        std::fs::write(
            cache_path(&src),
            "{\"v\":3,\"fingerprint\":\"x\",\"peak\":[],\"rms\":[]}",
        )
        .unwrap();
        assert!(!present(&src));
        std::fs::write(
            cache_path(&src),
            "{ \"v\": 4, \"fingerprint\": \"x\", \"peak\": [], \"rms\": [] }",
        )
        .unwrap();
        assert!(present(&src));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
