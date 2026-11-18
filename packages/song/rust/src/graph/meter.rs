// graph/meter.rs — the meter a graph reads its placements on and spells their `tc` on, as the TypeScript's two readings of one header.
// A reader would get wrong that there are two: `buildGraph` defaults each half of the signature (`num || 4`), `graphDoc` spells on the
// header only when both halves are digits and 4/4 otherwise, so a meter with a zero half places on one grid and spells on another (454).

use crate::cache::Cache;
use crate::files::WindowEdge;
use crate::lanes::inputs::Inputs;
use crate::read::read_head;
use crate::source::{Source, SourceError};
use song_schema::{beats_to_position_on, parse_position_on};

/// The two grids of a header and the tempo.
#[derive(Clone, Copy, Debug)]
pub struct Meter {
    place: (f64, f64),
    spell: (f64, f64),
    tempo: f64,
}

/// JS `parseInt(s, 10)`: leading space, a sign, then digits; NaN when there are none.
fn parse_int(s: &str) -> f64 {
    let s = s.trim_start_matches(|c: char| c.is_whitespace() || c == '\u{feff}');
    let (neg, rest) = match s.as_bytes().first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    let end = rest.bytes().position(|b| !b.is_ascii_digit()).unwrap_or(rest.len());
    match rest[..end].parse::<f64>() {
        Ok(v) if neg => -v,
        Ok(v) => v,
        Err(_) => f64::NAN,
    }
}

/// `n || 4`: a zero, a NaN and a missing half read as 4.
fn or_four(n: Option<f64>) -> f64 {
    n.filter(|n| *n != 0.0 && !n.is_nan()).unwrap_or(4.0)
}

/// The header's halves when it is `digits/digits`, else None.
fn digits_pair(text: &str) -> Option<(f64, f64)> {
    let (a, b) = text.split_once('/')?;
    let all = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    (all(a) && all(b)).then(|| (a.parse().unwrap_or(f64::NAN), b.parse().unwrap_or(f64::NAN)))
}

impl Meter {
    /// The meter of the project file, else 4/4 at the file's tempo or 120 (418).
    pub(super) fn of(inputs: &Inputs) -> Meter {
        let settings = inputs
            .head
            .root
            .as_ref()
            .and_then(|r| r.project.as_ref())
            .and_then(|f| f.project.as_ref());
        let tempo = settings.and_then(|p| p.tempo.as_ref()).map_or(120.0, |t| t.0);
        let text = settings.and_then(|p| p.time_signature.as_ref()).map(|t| t.0.as_str());
        let mut halves = text.unwrap_or("4/4").split('/').map(parse_int);
        let place = (or_four(halves.next()), or_four(halves.next()));
        // A half of zero has no bar to spell on: 4/4 stands (454).
        let spell = text
            .and_then(digits_pair)
            .filter(|(n, d)| *n != 0.0 && *d != 0.0)
            .unwrap_or((4.0, 4.0));
        Meter { place, spell, tempo }
    }

    /// An event's start in beats: a number is seconds, a timecode a 1-based position, none is `1:1:1`.
    pub fn position(&self, e: Option<&WindowEdge>) -> f64 {
        match e {
            Some(WindowEdge::Seconds(s)) => (s * self.tempo) / 60.0,
            Some(WindowEdge::Timecode(t)) => self.at(t),
            None => self.at("1:1:1"),
        }
    }

    fn at(&self, tc: &str) -> f64 {
        parse_position_on(tc, self.place.0, self.place.1).unwrap_or(f64::NAN)
    }

    /// `start` as the song spells it; a position that has no spelling is `""` (417).
    pub fn spell(&self, beats: f64) -> String {
        beats_to_position_on(beats, self.spell.0, self.spell.1).unwrap_or_default()
    }
}

/// Whether the project file's signature is `digits/digits` with a half of zero, which spells on 4/4 (454).
pub fn has_zero_half(source: &dyn Source, cache: &mut Cache, project: &str) -> Result<bool, SourceError> {
    let head = read_head(source, cache, project)?;
    let text = head
        .root
        .as_ref()
        .and_then(|r| r.project.as_ref())
        .and_then(|f| f.project.as_ref())
        .and_then(|p| p.time_signature.as_ref())
        .map(|t| t.0.as_str());
    Ok(text.and_then(digits_pair).is_some_and(|(n, d)| n == 0.0 || d == 0.0))
}
