// start_time.rs — an event's `start` in beats and back, plus the JS value conversions the time writers share.
// A reader would get wrong the unit: a NUMBER start is seconds, a string a 1-based position (song-schema docs/timecode.md).

use crate::error::{Error, Res};
use k7x::edit::{Value, js_number};
use song_schema::{beats_to_position_on, parse_position_on};

/// JS `Math.round`: halves go up.
#[must_use]
pub fn js_round(x: f64) -> f64 {
    let r = x.floor();
    if x - r >= 0.5 { r + 1.0 } else { r }
}

/// `a > b`, false for a NaN: what `!(a > b)` guards in the TypeScript.
#[must_use]
pub fn above(a: f64, b: f64) -> bool {
    a > b
}

/// A beat rounded to six decimals, as `roundBeat`.
#[must_use]
pub fn round_beat(beats: f64) -> f64 {
    song_schema::round_beat(beats)
}

/// `String(v)` for a plain value.
#[must_use]
pub fn js_string(v: &Value) -> String {
    match v {
        Value::Null => "null".to_owned(),
        Value::Bool(b) => b.to_string(),
        Value::Num(n) => js_number(*n),
        Value::Str(s) => s.clone(),
        Value::List(l) => l
            .iter()
            .map(|x| {
                if matches!(x, Value::Null) {
                    String::new()
                } else {
                    js_string(x)
                }
            })
            .collect::<Vec<_>>()
            .join(","),
        Value::Map(_) => "[object Object]".to_owned(),
    }
}

/// `parsePosition`, with the failure as the thrown message.
///
/// # Errors
/// A text that is not a timecode.
pub fn position(tc: &str, num: f64, den: f64) -> Res<f64> {
    parse_position_on(tc, num, den).map_err(|e| Error::Msg(e.to_string()))
}

/// `beatsToPosition`, with the failure as the thrown message.
///
/// # Errors
/// A beat out of range for a timecode.
pub fn to_position(beats: f64, num: f64, den: f64) -> Res<String> {
    beats_to_position_on(beats, num, den).map_err(|e| Error::Msg(e.to_string()))
}

/// An event's `start` in beats, whichever unit it is written in; `dflt` stands for an absent one.
///
/// # Errors
/// A start that does not read as a position.
pub fn start_beats(v: Option<&Value>, num: f64, den: f64, tempo: f64, dflt: &str) -> Res<f64> {
    match v {
        Some(Value::Num(n)) => Ok(n * tempo / 60.0),
        None | Some(Value::Null) => position(dflt, num, den),
        Some(Value::Str(s)) if s.is_empty() => position(dflt, num, den),
        Some(other) => position(&js_string(other), num, den),
    }
}

/// `beats` written in the unit `like` is: seconds stay seconds, to the millisecond, anything else a position.
///
/// # Errors
/// A beat out of range for a timecode.
pub fn start_as(like: Option<&Value>, beats: f64, num: f64, den: f64, tempo: f64) -> Res<Value> {
    if !matches!(like, Some(Value::Num(_))) {
        return Ok(Value::Str(to_position(beats, num, den)?));
    }
    let s = js_round(beats * 60.0 / tempo * 1000.0) / 1000.0;
    Ok(Value::Num(if s > 0.0 { s } else { 0.0 }))
}

/// How near a position must be to an event's start to address it: exact on the grid, half a tick off it.
#[must_use]
pub fn start_tolerance(v: Option<&Value>) -> f64 {
    if matches!(v, Some(Value::Num(_))) {
        0.5 / 960.0
    } else {
        1e-6
    }
}

/// `Number(v)` of a plain value, NaN when it does not read.
#[must_use]
pub fn js_to_number(v: &Value) -> f64 {
    match v {
        Value::Num(n) => *n,
        Value::Bool(b) => f64::from(u8::from(*b)),
        Value::Null => 0.0,
        Value::Str(s) => {
            let t = s.trim_matches(crate::format_level::is_js_space);
            if t.is_empty() {
                0.0
            } else if t.bytes().any(|b| b.is_ascii_alphabetic() && b != b'e' && b != b'E') {
                f64::NAN
            } else {
                t.parse::<f64>().unwrap_or(f64::NAN)
            }
        }
        Value::List(l) => match l.as_slice() {
            [] => 0.0,
            [one] => js_to_number(&Value::Str(js_string(one))),
            _ => f64::NAN,
        },
        Value::Map(_) => f64::NAN,
    }
}

/// `+x.toFixed(6)`: six decimals with a tie going UP in magnitude, where Rust's formatting goes to even.
#[must_use]
pub fn to_fixed6(x: f64) -> f64 {
    if !x.is_finite() {
        return x;
    }
    let exact = format!("{:.60}", x.abs());
    let Some((int, frac)) = exact.split_once('.') else {
        return x;
    };
    let (keep, next) = frac.split_at(6);
    let mut digits: Vec<u8> = format!("{int}{keep}").into_bytes();
    if next.as_bytes().first().is_some_and(|d| *d >= b'5') {
        let mut i = digits.len();
        loop {
            if i == 0 {
                digits.insert(0, b'1');
                break;
            }
            i -= 1;
            if digits[i] == b'9' {
                digits[i] = b'0';
            } else {
                digits[i] += 1;
                break;
            }
        }
    }
    let text = String::from_utf8(digits).unwrap_or_default();
    let (a, b) = text.split_at(text.len() - 6);
    let v = format!("{a}.{b}").parse::<f64>().unwrap_or(x.abs());
    if x < 0.0 { -v } else { v }
}
