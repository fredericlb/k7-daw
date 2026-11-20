//! num.rs — JavaScript's `Number(string)`, for the few places the TypeScript reads a count or a stamp out of text.
//! What a reader gets wrong: the empty string and white space alone are 0, not NaN, and a hex literal is a number.

/// JavaScript's white space and line terminators, which `Number` trims; Rust's own set differs (U+0085).
fn is_js_space(c: char) -> bool {
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

/// `Number(text)`; NaN when it is none.
pub fn js_number(text: &str) -> f64 {
    let t = text.trim_matches(is_js_space);
    if t.is_empty() {
        return 0.0;
    }
    let (neg, body) = match t.as_bytes()[0] {
        b'-' => (true, &t[1..]),
        b'+' => (false, &t[1..]),
        _ => (false, t),
    };
    let signed = |f: f64| if neg { -f } else { f };
    if body == "Infinity" {
        return signed(f64::INFINITY);
    }
    if t.len() == body.len() {
        for (prefix, radix) in [
            ("0x", 16),
            ("0X", 16),
            ("0o", 8),
            ("0O", 8),
            ("0b", 2),
            ("0B", 2),
        ] {
            if let Some(digits) = body.strip_prefix(prefix) {
                return radix_number(digits, radix);
            }
        }
    }
    if !is_decimal(body) {
        return f64::NAN;
    }
    body.parse::<f64>().map_or(f64::NAN, signed)
}

/// A literal's digits in `radix`: NaN unless every character is a digit of it (no sign), a float past what an integer holds.
fn radix_number(digits: &str, radix: u32) -> f64 {
    if digits.is_empty() || !digits.chars().all(|c| c.is_digit(radix)) {
        return f64::NAN;
    }
    digits.chars().fold(0.0, |acc, c| {
        acc * f64::from(radix) + f64::from(c.to_digit(radix).unwrap_or(0))
    })
}

/// `digits [. digits] [e [sign] digits]` with at least one digit before or after the point.
fn is_decimal(s: &str) -> bool {
    let (mantissa, exp) = match s.split_once(['e', 'E']) {
        Some((m, e)) => (m, Some(e)),
        None => (s, None),
    };
    let (int, frac) = mantissa
        .split_once('.')
        .map_or((mantissa, ""), |(i, f)| (i, f));
    let digits = |x: &str| x.bytes().all(|b| b.is_ascii_digit());
    let exp_ok = exp.is_none_or(|e| {
        let d = e.strip_prefix(['+', '-']).unwrap_or(e);
        !d.is_empty() && digits(d)
    });
    digits(int) && digits(frac) && !(int.is_empty() && frac.is_empty()) && exp_ok
}
