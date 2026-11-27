// js.rs — the few JavaScript number behaviours the gestures' answers depend on, so a port is not a rounding away from its twin.
// A reader would get wrong that `f64::round` is not `Math.round`: -2.5 rounds to -2 there and to -3 here.

/// `Math.round`: the nearest integer, a tie going toward +infinity.
pub fn round(x: f64) -> f64 {
    if !x.is_finite() {
        return x;
    }
    let f = x.floor();
    let r = if x - f >= 0.5 { f + 1.0 } else { f };
    // A negative value that rounds to zero keeps the sign, as `Math.round(-0.4)` is -0.
    if r == 0.0 && x.is_sign_negative() { -0.0 } else { r }
}

/// `Math.max` of two: NaN when either is, and +0 over -0.
pub fn max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a == 0.0 && b == 0.0 {
        if a.is_sign_negative() { b } else { a }
    } else {
        a.max(b)
    }
}

/// `Math.min` of two: NaN when either is, and -0 under +0.
pub fn min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a == 0.0 && b == 0.0 {
        if a.is_sign_negative() { a } else { b }
    } else {
        a.min(b)
    }
}

/// `Math.max(lo, Math.min(hi, v))`.
pub fn clamp(v: f64, lo: f64, hi: f64) -> f64 {
    max(lo, min(hi, v))
}

/// `Number.prototype.toFixed`: a tie on the exact value goes away from zero, which `{:.N}` does not.
pub fn to_fixed(x: f64, digits: usize) -> String {
    if !x.is_finite() {
        return if x.is_nan() {
            "NaN".into()
        } else if x < 0.0 {
            "-Infinity".into()
        } else {
            "Infinity".into()
        };
    }
    let neg = x < 0.0;
    let ax = x.abs();
    // 40 more digits than asked: a double's exact expansion shows a tie as a 5 and then zeros.
    let wide = format!("{:.*}", digits + 40, ax);
    let tail = &wide[wide.len() - 40..];
    let tie = tail.starts_with('5') && tail[1..].bytes().all(|b| b == b'0');
    let ax = if tie { f64::from_bits(ax.to_bits() + 1) } else { ax };
    format!("{}{:.*}", if neg { "-" } else { "" }, digits, ax)
}

/// `String(x)` for the values the gestures print: a whole number without a point, no `-0`.
pub fn number_string(x: f64) -> String {
    if x == 0.0 { "0".into() } else { format!("{x}") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_a_tie_up_as_math_round_does() {
        assert_eq!(round(2.5), 3.0);
        assert_eq!(round(-2.5), -2.0);
        assert_eq!(round(-0.5), 0.0);
        assert!(round(-0.4).is_sign_negative());
        assert!(!round(0.4).is_sign_negative());
        assert_eq!(round(0.49999999999999994), 0.0);
        assert_eq!(round(1.4), 1.0);
    }

    #[test]
    fn max_and_min_pass_a_nan_on_and_order_the_zeros() {
        assert!(max(0.0, -0.0).is_sign_positive() && max(-0.0, 0.0).is_sign_positive());
        assert!(min(0.0, -0.0).is_sign_negative() && min(-0.0, 0.0).is_sign_negative());
        assert!(max(1.0, f64::NAN).is_nan());
        assert!(min(f64::NAN, 1.0).is_nan());
        assert_eq!(clamp(5.0, 0.0, 3.0), 3.0);
    }

    #[test]
    fn to_fixed_breaks_a_tie_away_from_zero() {
        assert_eq!(to_fixed(0.03125, 4), "0.0313");
        assert_eq!(to_fixed(0.5, 0), "1");
        assert_eq!(to_fixed(2.5, 0), "3");
        assert_eq!(to_fixed(-2.5, 0), "-3");
        assert_eq!(to_fixed(1.0, 3), "1.000");
        assert_eq!(to_fixed(0.1, 4), "0.1000");
        assert_eq!(to_fixed(12.34565, 4), "12.3456");
    }

    #[test]
    fn number_string_drops_the_sign_of_zero() {
        assert_eq!(number_string(-0.0), "0");
        assert_eq!(number_string(4.0), "4");
        assert_eq!(number_string(1.25), "1.25");
    }
}
