// timecode.rs — "bar:beat:eighth:sixteenth.ticks" timecodes <-> beats (quarter notes), after ../../src/timecode.ts.
// A reader would get wrong that the JS quirks are kept on purpose: trim strips U+FEFF, `\d` is ASCII, a field is an f64.
// The event-start helpers (startBeats, startAs…) stay in TypeScript until the verbs move.

use crate::error::SchemaError;

/// Ticks per quarter note.
pub const TICKS_PER_QUARTER: f64 = 960.0;

/// The most colon fields the current notation uses; a longer chain is the deep one, read by its own rule.
const MAX_FIELDS: usize = 4;

/// Beyond this many ticks a float no longer counts whole numbers.
const MAX_TICKS: f64 = 9.0e15;

/// A time signature; both parts are at least 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimeSig {
    num: u32,
    den: u32,
}

impl TimeSig {
    pub fn new(num: u32, den: u32) -> Result<TimeSig, SchemaError> {
        if num == 0 || den == 0 {
            return Err(SchemaError::BadCurve(format!(
                "time signature {num}/{den}: the numerator and the denominator must be at least 1"
            )));
        }
        Ok(TimeSig { num, den })
    }

    pub fn num(self) -> u32 {
        self.num
    }

    pub fn den(self) -> u32 {
        self.den
    }

    /// Quarter notes in one metrical beat.
    pub fn beat(self) -> f64 {
        4.0 / f64::from(self.den)
    }

    /// Quarter notes in one bar.
    pub fn bar(self) -> f64 {
        f64::from(self.num) * self.beat()
    }
}

/// JS `String.prototype.trim`'s set: Rust's differs on U+0085 and U+FEFF.
fn is_js_space(c: char) -> bool {
    matches!(
        c,
        '\u{9}'..='\u{d}'
            | ' '
            | '\u{a0}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200a}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202f}'
            | '\u{205f}'
            | '\u{3000}'
            | '\u{feff}'
    )
}

fn is_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

/// The colon chain and the ticks after the dot, when `s` has the timecode's shape.
fn grammar(s: &str) -> Option<(&str, Option<&str>)> {
    let (head, ticks) = match s.find('.') {
        Some(i) => (s.get(..i)?, s.get(i + 1..)),
        None => (s, None),
    };
    if !head.split(':').all(is_digits) || ticks.is_some_and(|t| !is_digits(t)) {
        return None;
    }
    Some((head, ticks))
}

/// Whether `s`, untrimmed, has the shape a position or a duration is written in.
pub fn is_timecode(s: &str) -> bool {
    grammar(s).is_some()
}

/// A digit string as JS `parseInt` reads it: a float, so a huge field is large rather than an overflow.
fn number(digits: &str) -> f64 {
    digits.parse::<f64>().unwrap_or(f64::NAN)
}

/// JS `Math.round`: halves go up.
fn js_round(x: f64) -> f64 {
    let r = x.floor();
    if x - r >= 0.5 { r + 1.0 } else { r }
}

/// A beat rounded to six decimals, so a float's tail never reaches a file or a key.
pub fn round_beat(beats: f64) -> f64 {
    js_round(beats * 1e6) / 1e6
}

/// What one colon field is worth in quarter notes; the chain halves, or divides by four when `deep`.
fn level_beats(k: usize, q: f64, bar: f64, deep: bool) -> f64 {
    let steps = i32::try_from(k.saturating_sub(1)).unwrap_or(i32::MAX);
    match k {
        0 => bar,
        1 => q,
        _ => q / if deep { 4.0_f64.powi(steps) } else { 2.0_f64.powi(steps) },
    }
}

fn to_beats(tc: &str, num: f64, den: f64, one_based: bool) -> Result<f64, SchemaError> {
    let Some((head, ticks)) = grammar(tc.trim_matches(is_js_space)) else {
        return Err(SchemaError::BadTimecode(format!(
            "bad timecode '{tc}' — expected \"mesure:noire:croche:doublecroche\", optionally \".ticks\""
        )));
    };
    let fields: Vec<f64> = head.split(':').map(number).collect();
    let (q, bar) = (4.0 / den, num * (4.0 / den));
    let shift = if one_based { 1.0 } else { 0.0 };
    let deep = fields.len() > MAX_FIELDS;
    let mut beats = 0.0;
    for (k, v) in fields.iter().enumerate() {
        beats += (v - shift) * level_beats(k, q, bar, deep);
    }
    // Ticks are a remainder past the last field, never shifted by the 1-based rule.
    Ok(beats + ticks.map_or(0.0, number) / TICKS_PER_QUARTER)
}

fn from_beats(beats: f64, num: f64, den: f64, one_based: bool) -> Result<String, SchemaError> {
    // A time before the start is the start, as the TypeScript clamps it; only the upper range is refused.
    let scaled = (beats * TICKS_PER_QUARTER).max(0.0);
    if beats.is_nan() || !scaled.is_finite() || scaled > MAX_TICKS {
        return Err(SchemaError::BadTimecode(format!(
            "beats {beats} are out of range for a timecode"
        )));
    }
    let (q, bar) = (4.0 / den, num * (4.0 / den));
    let shift = u64::from(one_based);
    // Everything in ticks, rounded once at the top, so no level can overflow.
    let per_sixteenth = js_round(q / 4.0 * TICKS_PER_QUARTER);
    let per_eighth = per_sixteenth * 2.0;
    let per_beat = js_round(q * TICKS_PER_QUARTER);
    let per_bar = js_round(bar * TICKS_PER_QUARTER);
    // Adding 0.0 turns a negative zero into a plain one, which `as u64` and a printout both want.
    let mut t = js_round(scaled).max(0.0) + 0.0;
    let mut take = |per: f64| {
        let n = if per > 0.0 { (t / per).floor() } else { 0.0 };
        t -= n * per;
        n as u64
    };
    let bars = take(per_bar);
    let beat = take(per_beat);
    let eighth = take(per_eighth);
    let sixteenth = take(per_sixteenth);
    let ticks = t as u64;
    let mut out = vec![bars + shift, beat + shift];
    // A trailing level at its own origin is left off, only when everything below it is too.
    if eighth > 0 || sixteenth > 0 {
        out.push(eighth + shift);
    }
    if sixteenth > 0 {
        out.push(sixteenth + shift);
    }
    let head = out.iter().map(u64::to_string).collect::<Vec<_>>().join(":");
    Ok(if ticks > 0 { format!("{head}.{ticks}") } else { head })
}

/// A 1-based timeline or clip-relative position, in beats.
pub fn parse_position(tc: &str, sig: TimeSig) -> Result<f64, SchemaError> {
    to_beats(tc, f64::from(sig.num()), f64::from(sig.den()), true)
}

/// `parse_position` on a meter whose parts are floats, as a header the TypeScript reads with `parseInt` can state.
pub fn parse_position_on(tc: &str, num: f64, den: f64) -> Result<f64, SchemaError> {
    to_beats(tc, num, den, true)
}

/// A 0-based duration (length, offset, fade), in beats.
pub fn parse_duration(tc: &str, sig: TimeSig) -> Result<f64, SchemaError> {
    to_beats(tc, f64::from(sig.num()), f64::from(sig.den()), false)
}

pub fn beats_to_position(beats: f64, sig: TimeSig) -> Result<String, SchemaError> {
    from_beats(beats, f64::from(sig.num()), f64::from(sig.den()), true)
}

/// `beats_to_position` on a meter whose parts are floats.
pub fn beats_to_position_on(beats: f64, num: f64, den: f64) -> Result<String, SchemaError> {
    from_beats(beats, num, den, true)
}

/// `beats_to_duration` on a meter whose parts are floats.
pub fn beats_to_duration_on(beats: f64, num: f64, den: f64) -> Result<String, SchemaError> {
    from_beats(beats, num, den, false)
}

pub fn beats_to_duration(beats: f64, sig: TimeSig) -> Result<String, SchemaError> {
    from_beats(beats, f64::from(sig.num()), f64::from(sig.den()), false)
}
