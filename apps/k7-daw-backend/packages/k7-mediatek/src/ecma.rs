//! ecma.rs — the ECMAScript dates a zone's query and a sidecar's mark go through: `new Date(s)` for the date-time
//! format ECMA-262 defines, `toISOString`, and the local wall clock `getHours` and kin read (`Number` and `String` are below).
//! What a reader gets wrong: a date-time without an offset is LOCAL time, a date alone is UTC (ECMA-262 21.4.1.32).

use std::fs::Metadata;
use std::time::UNIX_EPOCH;

use chrono::{Datelike, Local, LocalResult, NaiveDate, NaiveDateTime, Offset, TimeZone, Timelike};
use serde_json::Value;

/// A moment `new Date(s)` names: an instant, or a wall-clock time still to be placed in the local zone.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Parsed {
    Utc(f64),
    Local(NaiveDateTime),
}

/// `new Date(s).getTime()` for ECMA-262's date-time string format, or None where it is NaN.
pub fn parse_date(s: &str) -> Option<f64> {
    match parse_iso(s)? {
        Parsed::Utc(ms) => Some(ms),
        Parsed::Local(n) => Some(local_to_ms(n)),
    }
}

/// `YYYY[-MM[-DD]][THH:mm[:ss[.sss]][Z|±HH:mm]]`, with a six-digit signed year; V8's other spellings are not read.
pub fn parse_iso(s: &str) -> Option<Parsed> {
    let b = s.as_bytes();
    let mut at = 0;
    let num = |at: &mut usize, n: usize| -> Option<i64> {
        let part = b.get(*at..*at + n)?;
        if !part.iter().all(u8::is_ascii_digit) {
            return None;
        }
        *at += n;
        std::str::from_utf8(part).ok()?.parse().ok()
    };
    let year = match b.first() {
        Some(b'+') | Some(b'-') => {
            let neg = b[0] == b'-';
            at = 1;
            let y = num(&mut at, 6)?;
            if neg && y == 0 {
                return None;
            }
            if neg { -y } else { y }
        }
        _ => num(&mut at, 4)?,
    };
    let mut month = 1;
    let mut day = 1;
    if b.get(at) == Some(&b'-') {
        at += 1;
        month = num(&mut at, 2)?;
        if b.get(at) == Some(&b'-') {
            at += 1;
            day = num(&mut at, 2)?;
        }
    }
    // V8 takes any day to 31 and lets it run into the next month, as MakeDay does.
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let date = NaiveDate::from_ymd_opt(i32::try_from(year).ok()?, month as u32, 1)?
        + chrono::Duration::days(day - 1);
    if at == b.len() {
        return Some(Parsed::Utc(utc_ms(date.and_hms_opt(0, 0, 0)?)));
    }
    if b.get(at) != Some(&b'T') {
        return None;
    }
    at += 1;
    let hour = num(&mut at, 2)?;
    if b.get(at) != Some(&b':') {
        return None;
    }
    at += 1;
    let minute = num(&mut at, 2)?;
    let mut second = 0;
    let mut millis = 0;
    if b.get(at) == Some(&b':') {
        at += 1;
        second = num(&mut at, 2)?;
        if b.get(at) == Some(&b'.') {
            at += 1;
            let start = at;
            while b.get(at).is_some_and(u8::is_ascii_digit) {
                at += 1;
            }
            if at == start {
                return None;
            }
            let frac = format!("{:0<3}", &s[start..at.min(start + 3)]);
            millis = frac.parse::<i64>().ok()?;
        }
    }
    // 24:00 is the end of the day, and only as all zeros.
    let end_of_day = hour == 24 && minute == 0 && second == 0 && millis == 0;
    if (hour > 23 && !end_of_day) || minute > 59 || second > 59 {
        return None;
    }
    let wall = date.and_hms_milli_opt(
        if end_of_day { 0 } else { hour as u32 },
        minute as u32,
        second as u32,
        millis as u32,
    )? + chrono::Duration::days(i64::from(end_of_day));
    let offset = match b.get(at) {
        None => return Some(Parsed::Local(wall)),
        Some(b'Z') if at + 1 == b.len() => 0,
        Some(&c @ (b'+' | b'-')) => {
            at += 1;
            let h = num(&mut at, 2)?;
            if b.get(at) != Some(&b':') {
                return None;
            }
            at += 1;
            let m = num(&mut at, 2)?;
            if at != b.len() || h > 23 || m > 59 {
                return None;
            }
            let o = h * 60 + m;
            if c == b'-' { -o } else { o }
        }
        _ => return None,
    };
    let ms = utc_ms(wall) - offset as f64 * 60_000.0;
    (ms.abs() <= 8.64e15).then_some(Parsed::Utc(ms))
}

fn utc_ms(n: NaiveDateTime) -> f64 {
    n.and_utc().timestamp_millis() as f64
}

/// A wall-clock time in the local zone, an ambiguous one the earlier, one in a gap moved past it (ECMA-262's rule).
fn local_to_ms(n: NaiveDateTime) -> f64 {
    match Local.from_local_datetime(&n) {
        LocalResult::Single(t) | LocalResult::Ambiguous(t, _) => t.timestamp_millis() as f64,
        LocalResult::None => {
            let before = Local
                .from_local_datetime(&(n - chrono::Duration::hours(3)))
                .earliest()
                .map(|t| t.offset().fix().local_minus_utc())
                .unwrap_or(0);
            utc_ms(n) - f64::from(before) * 1000.0
        }
    }
}

/// `toISOString`: `YYYY-MM-DDTHH:mm:ss.sssZ`, a year past four digits signed and six wide.
pub fn to_iso_string(ms: f64) -> String {
    let t = chrono::DateTime::from_timestamp_millis(ms as i64).unwrap_or_default();
    let y = t.year();
    let year = if (0..=9999).contains(&y) {
        format!("{y:04}")
    } else if y < 0 {
        format!("-{:06}", -y)
    } else {
        format!("+{y:06}")
    };
    format!(
        "{year}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        t.month(),
        t.day(),
        t.hour(),
        t.minute(),
        t.second(),
        t.timestamp_subsec_millis()
    )
}

/// `Date.now()`.
pub fn now_ms() -> f64 {
    chrono::Utc::now().timestamp_millis() as f64
}

/// The local wall clock at an instant: year, month (1-12), day, hours, minutes, seconds.
pub fn local_fields(ms: f64) -> (i32, u32, u32, u32, u32, u32) {
    let t = Local
        .timestamp_millis_opt(ms as i64)
        .earliest()
        .unwrap_or_else(|| Local.timestamp_millis_opt(0).unwrap());
    (
        t.year(),
        t.month(),
        t.day(),
        t.hour(),
        t.minute(),
        t.second(),
    )
}

/// `stat().mtimeMs`: seconds times a thousand plus nanoseconds over a million, in doubles, as node computes it.
pub fn mtime_ms(m: &Metadata) -> f64 {
    let Ok(t) = m.modified() else {
        return 0.0;
    };
    match t.duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs() as f64 * 1000.0 + d.subsec_nanos() as f64 / 1e6,
        Err(e) => {
            let d = e.duration();
            let (sec, nsec) = match d.subsec_nanos() {
                0 => (-(d.as_secs() as f64), 0.0),
                n => (-(d.as_secs() as f64) - 1.0, (1_000_000_000 - n) as f64),
            };
            sec * 1000.0 + nsec / 1e6
        }
    }
}

/// JavaScript's `WhiteSpace` and `LineTerminator`, what `trim()` and `Number()` drop.
fn js_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202f}'
                | '\u{205f}'
                | '\u{3000}'
                | '\u{feff}'
    )
}

/// `String.prototype.trim`.
pub fn js_trim(s: &str) -> &str {
    s.trim_matches(js_space)
}

/// `String(v)` of a JSON value, `null` within an array being empty as `join` makes it.
pub fn js_string(v: &Value) -> String {
    match v {
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => k7_js::number::to_string(n.as_f64().unwrap_or(f64::NAN)),
        Value::String(s) => s.clone(),
        Value::Array(a) => a
            .iter()
            .map(|e| {
                if e.is_null() {
                    String::new()
                } else {
                    js_string(e)
                }
            })
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".into(),
    }
}

/// `String(v ?? "")`.
pub fn js_string_or_empty(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => String::new(),
        Some(v) => js_string(v),
    }
}

/// Digits of one radix, as a number; None for none or another character.
fn radix(digits: &str, base: u32) -> Option<f64> {
    if digits.is_empty() {
        return None;
    }
    digits.chars().try_fold(0f64, |n, c| {
        c.to_digit(base).map(|d| n * base as f64 + d as f64)
    })
}

/// A decimal literal as `StrDecimalLiteral` reads it, its sign included; None for anything else.
fn decimal(s: &str) -> Option<f64> {
    let body = s.strip_prefix(['+', '-']).unwrap_or(s);
    if body == "Infinity" {
        return Some(if s.starts_with('-') {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        });
    }
    let (mantissa, exponent) = match body.find(['e', 'E']) {
        Some(i) => (&body[..i], Some(&body[i + 1..])),
        None => (body, None),
    };
    let (int, frac) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let digits = |t: &str| t.chars().all(|c| c.is_ascii_digit());
    if !digits(int) || !digits(frac) || (int.is_empty() && frac.is_empty()) {
        return None;
    }
    if let Some(e) = exponent {
        let e = e.strip_prefix(['+', '-']).unwrap_or(e);
        if e.is_empty() || !digits(e) {
            return None;
        }
    }
    s.parse().ok()
}

/// `StringToNumber`: blanks trimmed, empty is 0, `0x`, `0o`, `0b`, `Infinity` and decimals; NaN otherwise.
pub fn string_to_number(s: &str) -> f64 {
    let t = js_trim(s);
    if t.is_empty() {
        return 0.0;
    }
    let prefixed = |p: &str| {
        t.get(..2)
            .filter(|h| h.eq_ignore_ascii_case(p))
            .map(|_| &t[2..])
    };
    let n = match (prefixed("0x"), prefixed("0o"), prefixed("0b")) {
        (Some(d), _, _) => radix(d, 16),
        (_, Some(d), _) => radix(d, 8),
        (_, _, Some(d)) => radix(d, 2),
        _ => decimal(t),
    };
    n.unwrap_or(f64::NAN)
}

/// `Number(v)` of a JSON value.
pub fn js_number(v: &Value) -> f64 {
    match v {
        Value::Null => 0.0,
        Value::Bool(b) => f64::from(u8::from(*b)),
        Value::Number(n) => n.as_f64().unwrap_or(f64::NAN),
        Value::String(s) => string_to_number(s),
        Value::Array(_) => string_to_number(&js_string(v)),
        Value::Object(_) => f64::NAN,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn coerces_as_javascript_does() {
        assert_eq!(
            js_string(&json!([1, null, [2, "a"], {}])),
            "1,,2,a,[object Object]"
        );
        for (s, n) in [
            (" 12 ", 12.0),
            ("", 0.0),
            ("0x1A", 26.0),
            ("1e3", 1000.0),
            (".5", 0.5),
            ("5.", 5.0),
            ("-Infinity", f64::NEG_INFINITY),
        ] {
            assert_eq!(string_to_number(s), n, "{s:?}");
        }
        for s in [
            "fast", "inf", "1e", "0x", "+0x1", "1.2.3", ".", "NaN", "1_000",
        ] {
            assert!(string_to_number(s).is_nan(), "{s:?}");
        }
        assert_eq!(js_number(&json!([5])), 5.0);
        assert_eq!(js_number(&json!(true)), 1.0);
    }

    #[test]
    fn reads_the_iso_forms_and_refuses_the_rest() {
        assert_eq!(parse_date("1970-01-01T00:00:00.000Z"), Some(0.0));
        assert_eq!(parse_date("1970-01-02"), Some(86_400_000.0));
        assert_eq!(parse_date("1970"), Some(0.0));
        assert_eq!(parse_date("1970-02"), Some(31.0 * 86_400_000.0));
        assert_eq!(parse_date("1970-01-01T01:00+01:00"), Some(0.0));
        assert_eq!(parse_date("1970-01-01T00:00:00.1239Z"), Some(123.0));
        assert_eq!(parse_date("1970-01-01T24:00Z"), Some(86_400_000.0));
        assert!(matches!(
            parse_iso("2026-08-03T21:05:07"),
            Some(Parsed::Local(_))
        ));
        for s in [
            "hier soir",
            "2026-13-01",
            "2026-02-32",
            "2026-08-03T25:00",
            "2026-08-03T21:05:07.Z",
            "-000000-01-01",
            "2026-08-03 21:05",
            "",
        ] {
            assert_eq!(parse_date(s), None, "{s}");
        }
    }

    #[test]
    fn spells_an_instant_as_to_iso_string() {
        assert_eq!(to_iso_string(0.0), "1970-01-01T00:00:00.000Z");
        assert_eq!(
            to_iso_string(1_785_000_000_123.0),
            "2026-07-25T17:20:00.123Z"
        );
    }
}
