// clips/audio_def.rs — `loadAudioClipDoc` and `materialRefOf`: the audio clip a project's `clips/<id>.k7x` defines, unplaced (2304).
// A reader would get wrong that a SYNTAX error in any clip file throws, as the TypeScript's readers do, while a file that
// decodes badly is only absent; the audition never needs more of a clip than its source and its window.

use super::paths::audio_paths;
use crate::cache::Cache;
use crate::files::{Clip, WindowEdge};
use crate::read::{FamilyRead, read_family};
use crate::source::path::{PathRule, join};
use crate::source::{Source, SourceError};
use std::sync::Arc;

/// The project's grid a timecode window needs to become seconds (`ClipGrid`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClipGrid {
    pub tempo: f64,
    pub num: f64,
    pub den: f64,
}

/// The audio clip as the audition reads it: the confined source ("" outside every mount) and its window in seconds.
#[derive(Clone, Debug, PartialEq)]
pub struct AudioClipDef {
    pub id: String,
    pub source: String,
    pub source_from: Option<f64>,
    pub source_to: Option<f64>,
}

/// JavaScript's whitespace: the white-space characters minus U+0085, plus U+FEFF.
fn js_space(c: char) -> bool {
    (c.is_whitespace() && c != '\u{85}') || c == '\u{feff}'
}

/// `Number.isFinite(Number(s))` for the text a placement id ends with.
fn js_finite(s: &str) -> bool {
    let t = s.trim_matches(js_space);
    if t.is_empty() {
        return true;
    }
    let radix = |marks: [&str; 2], base: u32| {
        let digits = marks.iter().find_map(|m| t.strip_prefix(m))?;
        if digits.is_empty() {
            return Some(false);
        }
        let mut v = 0.0_f64;
        for c in digits.chars() {
            v = v * f64::from(base) + f64::from(c.to_digit(base)?);
        }
        Some(v.is_finite())
    };
    for (marks, base) in [(["0x", "0X"], 16), (["0o", "0O"], 8), (["0b", "0B"], 2)] {
        if t.starts_with(marks[0]) || t.starts_with(marks[1]) {
            return radix(marks, base).unwrap_or(false);
        }
    }
    let body = t.strip_prefix(['+', '-']).unwrap_or(t);
    let (mantissa, exp) = match body.find(['e', 'E']) {
        Some(i) => (&body[..i], Some(&body[i + 1..])),
        None => (body, None),
    };
    let (int, frac) = mantissa.split_once('.').map_or((mantissa, None), |(a, b)| (a, Some(b)));
    let digits = |x: &str| x.bytes().all(|b| b.is_ascii_digit());
    let mantissa_ok =
        digits(int) && frac.is_none_or(digits) && (!int.is_empty() || frac.is_some_and(|f| !f.is_empty()));
    let exp_ok = exp.is_none_or(|e| {
        let e = e.strip_prefix(['+', '-']).unwrap_or(e);
        !e.is_empty() && digits(e)
    });
    mantissa_ok && exp_ok && t.parse::<f64>().is_ok_and(f64::is_finite)
}

/// The MATERIAL a placement id names: after the last `>`, split on the LAST `@` and only before a number, a repeat's `#n` aside.
pub fn material_ref_of(nested: &str) -> &str {
    let clip = nested.rfind('>').map_or(nested, |i| &nested[i + 1..]);
    match clip.rfind('@') {
        Some(at)
            if at > 0 && !clip[at + 1..].is_empty() && js_finite(super::distinct::without_repeat(&clip[at + 1..])) =>
        {
            &clip[..at]
        }
        _ => clip,
    }
}

/// A window edge in SECONDS, as `windowEdge` reads it: a timecode needs the grid, and a half-typed one is no window.
fn edge(e: Option<&WindowEdge>, grid: Option<ClipGrid>) -> Option<f64> {
    match e? {
        WindowEdge::Seconds(s) => (*s > 0.0).then_some(*s),
        WindowEdge::Timecode(tc) => {
            let g = grid?;
            let beats = song_schema::parse_position_on(tc, g.num, g.den).ok()?;
            (beats > 0.0).then(|| (beats * 60.0) / g.tempo)
        }
    }
}

/// The first syntax error among the clip files that did not decode, as the TypeScript throws it: `message (line:col)`.
fn syntax_error(source: &dyn Source, project: &str, family: &FamilyRead<Clip>) -> Option<String> {
    let mut found: Vec<(String, String)> = Vec::new();
    for f in &family.findings {
        let Ok(text) = source.read_text(&join(project, &f.file)) else {
            continue;
        };
        let bump = k7x::Bump::new();
        if let Err(e) = k7x::parse_tree(&text, &bump) {
            let name = f.file.rsplit('/').next().unwrap_or("");
            let base = name.rfind('.').map_or(name, |i| &name[..i]);
            found.push((
                format!("{base},{name}"),
                format!("{} ({}:{})", e.message, e.line, e.column),
            ));
        }
    }
    found
        .into_iter()
        .min_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()))
        .map(|(_, m)| m)
}

/// The audio clip `id` of the folder project `project`, or None for a video, a caption, a midi clip, a missing or invalid file.
pub fn audio_clip_def(
    source: &dyn Source,
    project: &str,
    id: &str,
    grid: Option<ClipGrid>,
    rule: PathRule,
) -> Result<Option<AudioClipDef>, String> {
    match source.stat(&join(project, "project.k7x")) {
        Ok(_) => {}
        Err(SourceError::NotFound | SourceError::Outside | SourceError::Io(_)) => return Ok(None),
        Err(e) => return Err(e.to_string()),
    }
    let family = read_family::<Clip>(source, &mut Cache::with_paths(rule), project).map_err(|e| e.to_string())?;
    if let Some(why) = syntax_error(source, project, &family) {
        return Err(why);
    }
    let Some(Clip::Audio(a)) = family.get(id).map(Arc::as_ref) else {
        return Ok(None);
    };
    let one = FamilyRead {
        values: vec![(id.to_owned(), Arc::new(Clip::Audio(a.clone())))],
        names: Vec::new(),
        findings: Vec::new(),
    };
    let paths = audio_paths(source, project, &one, rule).map_err(|e| e.to_string())?;
    Ok(Some(AudioClipDef {
        id: id.to_owned(),
        source: paths.get(id).cloned().unwrap_or_default(),
        source_from: edge(a.from.as_ref(), grid),
        source_to: edge(a.to.as_ref(), grid),
    }))
}
