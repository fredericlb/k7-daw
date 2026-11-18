// clips/id.rs — the ids a placement gets, and the JavaScript number rules they depend on; after placementId.ts and timelineLane.ts.
// A reader would get wrong that an id is a STRING compared exactly: a number is written as JavaScript writes it (decision 152).

/// What separates a timeline from its lane, and the timelines a nested clip's id walks through.
pub const NEST_SEP: &str = ">";

/// `Number.prototype.toString` for a double: the shortest digits, `1e+21` past 21 digits, `1e-7` below 1e-6, `-0` as `0`.
pub fn js_number(v: f64) -> String {
    if v.is_nan() {
        return "NaN".to_owned();
    }
    if v.is_infinite() {
        return if v < 0.0 { "-Infinity" } else { "Infinity" }.to_owned();
    }
    if v == 0.0 {
        return "0".to_owned();
    }
    let sign = if v < 0.0 { "-" } else { "" };
    // `{:e}` prints the shortest digits that read back: `d.ddde<exp>`.
    let sci = format!("{:e}", v.abs());
    let (mantissa, exp) = sci.split_once('e').unwrap_or((sci.as_str(), "0"));
    let mut digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    let e: i32 = exp.parse().unwrap_or(0);
    if let Some(even) = tie_to_even(v.abs(), &digits, e) {
        digits = even;
    }
    let k = i32::try_from(digits.len()).unwrap_or(i32::MAX);
    // `n` is the position of the decimal point counted from the first digit, as the specification names it.
    let n = e + 1;
    let body = if k <= n && n <= 21 {
        format!("{digits}{}", "0".repeat(usize::try_from(n - k).unwrap_or(0)))
    } else if 0 < n && n <= 21 {
        let at = usize::try_from(n).unwrap_or(0);
        format!("{}.{}", digits.get(..at).unwrap_or(""), digits.get(at..).unwrap_or(""))
    } else if -6 < n && n <= 0 {
        format!("0.{}{digits}", "0".repeat(usize::try_from(-n).unwrap_or(0)))
    } else {
        let x = n - 1;
        let sign_e = if x < 0 { "-" } else { "+" };
        let lead = digits.get(..1).unwrap_or("");
        let tail = digits.get(1..).unwrap_or("");
        let frac = if tail.is_empty() {
            String::new()
        } else {
            format!(".{tail}")
        };
        format!("{lead}{frac}e{sign_e}{}", x.abs())
    };
    format!("{sign}{body}")
}

/// The digits JavaScript picks when two `k`-digit candidates are equally near `v` and both read back: the even one.
fn tie_to_even(v: f64, digits: &str, e: i32) -> Option<String> {
    let k = digits.len();
    let d: u64 = digits.parse().ok()?;
    let reads_back = |n: u64| {
        let at = e - i32::try_from(k).ok()? + 1;
        format!("{n}e{at}").parse::<f64>().ok().map(|x| x == v)
    };
    let same_len = |n: u64| n.to_string().len() == k;
    let (lo, hi) = if d > 0 && same_len(d - 1) && reads_back(d - 1) == Some(true) {
        (d - 1, d)
    } else if same_len(d + 1) && reads_back(d + 1) == Some(true) {
        (d, d + 1)
    } else {
        return None;
    };
    // The exact expansion of a double has at most 767 significant digits.
    let exact = format!("{v:.800e}");
    let (m, x) = exact.split_once('e')?;
    let ex: String = m.chars().filter(char::is_ascii_digit).collect();
    if x.parse::<i32>().ok()? != e || ex.get(..k)?.parse::<u64>().ok()? != lo {
        return None;
    }
    let rest = ex.get(k..)?;
    let tie = rest.starts_with('5') && rest[1..].bytes().all(|b| b == b'0');
    (tie && reads_back(lo) == Some(true) && reads_back(hi) == Some(true))
        .then(|| (if lo % 2 == 0 { lo } else { hi }).to_string())
}

/// A placement's id: the material, `@`, and the start in beats, written as JavaScript writes a number.
pub fn placement_id(ref_: &str, start_beats: f64) -> String {
    format!("{ref_}@{}", js_number(start_beats))
}

/// The address of lane `lane` of timeline `timeline`.
pub fn timeline_lane_id(timeline: &str, lane: &str) -> String {
    format!("{timeline}{NEST_SEP}{lane}")
}

/// `Math.max` of two numbers: a NaN wins, which Rust's `f64::max` does not do.
pub fn js_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a > b {
        a
    } else {
        b
    }
}

/// `Math.min` of two numbers: a NaN wins.
pub fn js_min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a < b {
        a
    } else {
        b
    }
}
