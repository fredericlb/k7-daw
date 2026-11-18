//! JavaScript's number formats where a file holds them: `Number.prototype.toFixed` (the analysis
//! sidecars), `String(number)`, `Math.round` and live-sync's `textHash`. Native, because each is a
//! closed algorithm the spec pins down; proven against node by `tests/all/number.rs` (docs/parity.md).

/// `x.toFixed(digits)`, digits 0 to 100, as ECMA-262 defines it on the double's exact value.
pub fn to_fixed(x: f64, digits: usize) -> String {
    assert!(digits <= 100, "toFixed takes 0 to 100 digits");
    if x.is_nan() {
        return "NaN".into();
    }
    if x.abs() >= 1e21 {
        return to_string(x);
    }
    let (sign, x) = if x < 0.0 { ("-", -x) } else { ("", x.abs()) };
    let body = if fraction_bits(x) == digits + 1 {
        // An exact tie: the spec takes the larger n, where Rust's formatting rounds to even.
        round_half_up(&format!("{x:.*}", digits + 1))
    } else {
        format!("{x:.digits$}")
    };
    format!("{sign}{body}")
}

/// How many binary digits a finite non-negative double has after its point.
fn fraction_bits(x: f64) -> usize {
    if x == 0.0 {
        return 0;
    }
    let bits = x.to_bits();
    let exp = ((bits >> 52) & 0x7ff) as i64;
    let frac = bits & ((1 << 52) - 1);
    let (mant, e) = if exp == 0 {
        (frac, -1074)
    } else {
        (frac | (1 << 52), exp - 1075)
    };
    let e = e + mant.trailing_zeros() as i64;
    if e >= 0 { 0 } else { (-e) as usize }
}

/// An exact decimal ending in `5`, its last digit dropped and the rest rounded up.
fn round_half_up(exact: &str) -> String {
    let mut digits: Vec<u8> = exact[..exact.len() - 1].bytes().collect();
    if digits.last() == Some(&b'.') {
        digits.pop();
    }
    let mut i = digits.len();
    loop {
        if i == 0 {
            digits.insert(0, b'1');
            break;
        }
        i -= 1;
        match digits[i] {
            b'.' => continue,
            b'9' => digits[i] = b'0',
            d => {
                digits[i] = d + 1;
                break;
            }
        }
    }
    String::from_utf8(digits).expect("ASCII digits")
}

/// `String(x)`: ECMA-262's Number::toString, radix 10.
pub fn to_string(x: f64) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x == 0.0 {
        return "0".into();
    }
    if x < 0.0 {
        return format!("-{}", to_string(-x));
    }
    if x.is_infinite() {
        return "Infinity".into();
    }
    let (digits, n) = shortest(x);
    let k = digits.len() as i64;
    if k <= n && n <= 21 {
        format!("{digits}{}", "0".repeat((n - k) as usize))
    } else if 0 < n && n <= 21 {
        format!("{}.{}", &digits[..n as usize], &digits[n as usize..])
    } else if -6 < n && n <= 0 {
        format!("0.{}{digits}", "0".repeat((-n) as usize))
    } else {
        let e = n - 1;
        let sign = if e < 0 { '-' } else { '+' };
        let head = if k == 1 {
            digits.clone()
        } else {
            format!("{}.{}", &digits[..1], &digits[1..])
        };
        format!("{head}e{sign}{}", e.abs())
    }
}

/// A positive finite double's shortest round-tripping digits and its decimal point position `n`.
fn shortest(x: f64) -> (String, i64) {
    // Rust's `{:e}` is the shortest round-tripping digit string, but rounds a tie up.
    let sci = format!("{x:e}");
    let (mant, exp) = sci.split_once('e').expect("exponent");
    let digits: String = mant.chars().filter(|c| *c != '.').collect();
    let exp: i64 = exp.parse().expect("exponent");
    let k = digits.len();
    // The spec takes the even one when two shortest strings are equally close: an exact `5` past them.
    let next = format!("{x:.k$e}");
    let (nmant, nexp) = next.split_once('e').expect("exponent");
    if !nmant.ends_with('5') || nexp.parse::<i64>().ok() != Some(exp) {
        return (digits, exp + 1);
    }
    let exact = format!("{x:.800e}");
    let (emant, _) = exact.split_once('e').expect("exponent");
    let edigits: Vec<u8> = emant.bytes().filter(|c| *c != b'.').collect();
    if edigits[k] != b'5' || edigits[k + 1..].iter().any(|d| *d != b'0') {
        return (digits, exp + 1);
    }
    let low = &edigits[..k];
    if low[k - 1].is_multiple_of(2) {
        let s = String::from_utf8(low.to_vec()).expect("ASCII digits");
        return (s.trim_end_matches('0').to_string(), exp + 1);
    }
    (digits, exp + 1)
}

/// `Math.round(x)`: the nearest integer, a tie going towards +∞.
pub fn math_round(x: f64) -> f64 {
    if !x.is_finite() || x.fract() == 0.0 {
        return x;
    }
    let r = x.floor();
    let out = if x - r >= 0.5 { r + 1.0 } else { r };
    if out == 0.0 && x < 0.0 { -0.0 } else { out }
}

/// `textHash(text)` of `@studio/live-sync`: 53 bits over the text's UTF-16 units, as lowercase hex.
pub fn text_hash(text: &str) -> String {
    let mut h1: u32 = 0xdeadbeef;
    let mut h2: u32 = 0x41c6ce57;
    for c in text.encode_utf16() {
        h1 = (h1 ^ c as u32).wrapping_mul(2654435761);
        h2 = (h2 ^ c as u32).wrapping_mul(1597334677);
    }
    h1 = (h1 ^ (h1 >> 16)).wrapping_mul(2246822507) ^ (h2 ^ (h2 >> 13)).wrapping_mul(3266489909);
    h2 = (h2 ^ (h2 >> 16)).wrapping_mul(2246822507) ^ (h1 ^ (h1 >> 13)).wrapping_mul(3266489909);
    format!("{:x}", 4294967296u64 * (2097151 & h2) as u64 + h1 as u64)
}
