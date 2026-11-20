//! `/logs`' query as the route reads it (2622): JavaScript's `Number` over a query string, `num`, the tail's clamp, the
//! machine's half and the answer's key order. What a reader gets wrong: `Number(" 3 ")` is 3 and `Number("-0x10")` is NaN.

use serde_json::{Value, json};

use super::is_space;
use super::ring::Ring;

/// `Number(s)` for a string: JavaScript's grammar, NaN where it does not hold.
pub fn to_number(s: &str) -> f64 {
    let t = s.trim_matches(is_space);
    if t.is_empty() {
        return 0.0;
    }
    let b = t.as_bytes();
    if b.len() > 2 && b[0] == b'0' {
        let radix = match b[1] {
            b'x' | b'X' => Some(16),
            b'o' | b'O' => Some(8),
            b'b' | b'B' => Some(2),
            _ => None,
        };
        if let Some(r) = radix {
            return radix_number(&t[2..], r);
        }
    }
    let (sign, rest) = match b[0] {
        b'+' => (1.0, &t[1..]),
        b'-' => (-1.0, &t[1..]),
        _ => (1.0, t),
    };
    if rest == "Infinity" {
        return sign * f64::INFINITY;
    }
    if decimal(rest) {
        return rest.parse::<f64>().map_or(f64::NAN, |v| sign * v);
    }
    f64::NAN
}

/// An unsigned integer in `radix`, every digit valid.
fn radix_number(digits: &str, radix: u32) -> f64 {
    let mut exact: Option<u128> = Some(0);
    let mut approx = 0.0;
    for c in digits.chars() {
        let Some(d) = c.to_digit(radix) else {
            return f64::NAN;
        };
        approx = approx * radix as f64 + d as f64;
        exact = exact.and_then(|v| v.checked_mul(radix as u128)?.checked_add(d as u128));
    }
    exact.map_or(approx, |v| v as f64)
}

/// `digits [. digits] [e [+-] digits]` with a digit before or after the point.
fn decimal(s: &str) -> bool {
    let b = s.as_bytes();
    let digits = |from: usize| b[from..].iter().take_while(|c| c.is_ascii_digit()).count();
    let int = digits(0);
    let mut i = int;
    let mut frac = 0;
    if b.get(i) == Some(&b'.') {
        frac = digits(i + 1);
        i += 1 + frac;
    }
    if int + frac == 0 {
        return false;
    }
    if matches!(b.get(i), Some(b'e' | b'E')) {
        i += 1;
        if matches!(b.get(i), Some(b'+' | b'-')) {
            i += 1;
        }
        let e = digits(i);
        if e == 0 {
            return false;
        }
        i += e;
    }
    i == b.len()
}

/// The route's `num`: the value as a finite number, else absent.
pub fn num(v: Option<&str>) -> Option<f64> {
    v.map(to_number).filter(|n| n.is_finite())
}

/// The route's tail: `Number(v) || 300` clamped to 1..2000.
pub fn tail_of(v: Option<&str>) -> f64 {
    let n = v.map_or(f64::NAN, to_number);
    let n = if n.is_nan() || n == 0.0 { 300.0 } else { n };
    n.clamp(1.0, 2000.0)
}

/// What the route reads of the query.
#[derive(Clone, Debug, PartialEq)]
pub struct Query {
    pub after: Option<f64>,
    pub offset: Option<f64>,
    pub tail: f64,
}

impl Query {
    pub fn parse(after: Option<&str>, offset: Option<&str>, tail: Option<&str>) -> Query {
        Query {
            after: num(after),
            offset: num(offset),
            tail: tail_of(tail),
        }
    }

    /// The tail as `slice(-tail)` reads it: its integer part.
    pub fn count(&self) -> usize {
        self.tail.trunc() as usize
    }

    /// What the machine is asked: `{type: "log", offset?, tail}`.
    pub fn machine_request(&self) -> Value {
        match self.offset {
            Some(o) => json!({ "type": "log", "offset": o, "tail": self.tail }),
            None => json!({ "type": "log", "tail": self.tail }),
        }
    }
}

/// The machine's half: its reply when it is `{offset: number, lines: string[]}`, else the empty one.
pub fn machine_half(reply: Option<&Value>) -> Value {
    let ok = reply.is_some_and(|r| {
        r["offset"].is_number()
            && r["lines"]
                .as_array()
                .is_some_and(|l| l.iter().all(Value::is_string))
    });
    match reply {
        Some(r) if ok => json!({ "offset": r["offset"], "lines": r["lines"] }),
        _ => json!({ "offset": 0, "lines": [] }),
    }
}

/// The ring's half: `{seq, lines}`.
pub fn service_half(ring: &Ring, q: &Query) -> Value {
    let (seq, lines) = ring.since(q.after, q.count());
    json!({ "seq": seq, "lines": lines.iter().map(|l| l.to_json()).collect::<Vec<_>>() })
}

/// The route's answer: `offset`, `lines`, `service`.
pub fn answer(machine: &Value, service: Value) -> Value {
    json!({ "offset": machine["offset"], "lines": machine["lines"], "service": service })
}
