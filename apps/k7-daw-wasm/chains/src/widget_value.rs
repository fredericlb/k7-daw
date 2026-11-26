// widget_value.rs — a widget's value maths: its range, a value to the 0..1 travel of the control and back, a crossfade pair, the text.
// A reader would get wrong that every rounding is JavaScript's (halves up, `toFixed` on the exact value): the page's text must not change.

use serde::{Deserialize, Serialize};

/// The mix parameter of the engine's own wet/dry knob.
pub const MIX_PARAM: &str = "@mix";

/// The fields of a widget its value maths reads; any other field of the panel's widget is ignored.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(optional_fields))]
pub struct WidgetScale {
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: Option<f64>,
    pub curve: Option<String>,
    pub pair: Option<String>,
    pub bind: Option<String>,
}

/// JavaScript's `Math.round`: halves go up, toward +infinity.
pub fn js_round(x: f64) -> f64 {
    let r = x.floor();
    if x - r >= 0.5 { r + 1.0 } else { r }
}

/// JavaScript's `Math.min` and `Math.max`: a NaN anywhere makes the answer NaN, which Rust's would skip.
fn js_min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() { f64::NAN } else { a.min(b) }
}

fn js_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() { f64::NAN } else { a.max(b) }
}

fn clamp01(t: f64) -> f64 {
    t.clamp(0.0, 1.0)
}

/// A truthy `step`: JavaScript reads 0 and NaN as no step.
fn step_of(w: &WidgetScale) -> Option<f64> {
    w.step.filter(|s| *s != 0.0 && !s.is_nan())
}

impl WidgetScale {
    /// The range, defaulting to 0..1 — right for a normalised plugin param, wrong for native units.
    pub fn range(&self) -> (f64, f64) {
        (self.min.unwrap_or(0.0), self.max.unwrap_or(1.0))
    }

    fn is_log(&self) -> bool {
        self.curve.as_deref() == Some("log") && self.min.unwrap_or(0.0) > 0.0
    }

    fn pair_on(&self) -> bool {
        self.pair.as_deref().is_some_and(|p| !p.is_empty())
    }

    fn frac(&self, value: f64) -> f64 {
        let (min, max) = self.range();
        if max == min {
            0.0
        } else {
            clamp01((value - min) / (max - min))
        }
    }

    /// A value to its 0..1 travel: linear, log, or the equal-power curve of a crossfade pair.
    pub fn norm(&self, value: f64) -> f64 {
        let (min, max) = self.range();
        if max == min {
            return 0.0;
        }
        if self.pair_on() {
            return clamp01(self.frac(value).asin() * 2.0 / std::f64::consts::PI);
        }
        if self.is_log() {
            return clamp01((value / min).ln() / (max / min).ln());
        }
        clamp01((value - min) / (max - min))
    }

    /// A 0..1 travel back to a value, quantised to `step` when the widget has one.
    pub fn denorm(&self, t: f64) -> f64 {
        let (min, max) = self.range();
        let u = clamp01(t);
        if self.pair_on() {
            return min + (max - min) * (u * std::f64::consts::PI / 2.0).sin();
        }
        let v = if self.is_log() {
            min * (max / min).powf(u)
        } else {
            min + (max - min) * u
        };
        let Some(step) = step_of(self) else { return v };
        js_min(max, js_max(min, js_round(v / step) * step))
    }

    /// A knob read and typed as a MIX in percent: a crossfade over two params, or the engine's own `@mix`.
    pub fn is_mix_knob(&self) -> bool {
        self.pair_on() || self.bind.as_deref() == Some(MIX_PARAM)
    }

    /// The DRY level a crossfade knob sets beside `wet`: the same position on the cos side, √(1 − wet²).
    pub fn pair_value(&self, wet: f64) -> f64 {
        let (min, max) = self.range();
        let f = self.frac(wet);
        min + (max - min) * (1.0 - f * f).sqrt()
    }

    /// A control's reading: whole for a whole-number step, signed when its range goes below zero.
    pub fn text(&self, v: Option<f64>) -> String {
        let Some(v) = v else { return "–".to_owned() };
        if step_of(self).is_some_and(|s| s.is_finite() && s.fract() == 0.0) {
            let n = js_round(v);
            let whole = if n == 0.0 { 0 } else { n as i64 };
            return if self.min.unwrap_or(0.0) < 0.0 && n > 0.0 {
                format!("+{whole}")
            } else {
                whole.to_string()
            };
        }
        let a = v.abs();
        to_fixed(
            v,
            if a >= 100.0 {
                0
            } else if a >= 10.0 {
                1
            } else {
                2
            },
        )
    }

    /// A value TYPED into a control: clamped to its range and quantised to its `step`, None when it is no number.
    pub fn typed(&self, text: &str) -> Option<f64> {
        let t = text.trim();
        if t.is_empty() {
            return None;
        }
        let v = js_number(&t.replacen(',', ".", 1))?;
        if !v.is_finite() {
            return None;
        }
        if self.is_mix_knob() {
            return Some(self.denorm(v / 100.0));
        }
        let (min, max) = self.range();
        let stepped = match step_of(self) {
            Some(s) => js_round(v / s) * s,
            None => v,
        };
        Some(js_min(max, js_max(min, stepped)))
    }
}

/// JavaScript's `Number(text)` for a trimmed text: decimal, exponent, and the 0x, 0o and 0b prefixes.
fn js_number(t: &str) -> Option<f64> {
    for (prefix, radix) in [("0x", 16), ("0X", 16), ("0o", 8), ("0O", 8), ("0b", 2), ("0B", 2)] {
        if let Some(rest) = t.strip_prefix(prefix) {
            return u64::from_str_radix(rest, radix).ok().map(|n| n as f64);
        }
    }
    // Rust reads "inf" and "nan" too, which are not finite and refused by the caller.
    t.parse::<f64>().ok()
}

/// JavaScript's `Number.prototype.toFixed`: the exact decimal value, a tie going away from zero.
pub fn to_fixed(v: f64, digits: usize) -> String {
    if !v.is_finite() {
        return if v.is_nan() {
            "NaN".to_owned()
        } else if v > 0.0 {
            "Infinity".to_owned()
        } else {
            "-Infinity".to_owned()
        };
    }
    let wide = format!("{:.40}", v.abs());
    let (whole, frac) = wide.split_once('.').unwrap_or((&wide, ""));
    let mut kept: Vec<u8> = whole.bytes().chain(frac.bytes().take(digits)).collect();
    if frac.as_bytes().get(digits).copied().unwrap_or(b'0') >= b'5' {
        let mut i = kept.len();
        loop {
            if i == 0 {
                kept.insert(0, b'1');
                break;
            }
            i -= 1;
            if kept[i] == b'9' {
                kept[i] = b'0';
            } else {
                kept[i] += 1;
                break;
            }
        }
    }
    let int_len = kept.len() - digits;
    let mut out = String::from_utf8_lossy(&kept[..int_len]).into_owned();
    if digits > 0 {
        out.push('.');
        out.push_str(&String::from_utf8_lossy(&kept[int_len..]));
    }
    if v < 0.0 { format!("-{out}") } else { out }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn knob(min: Option<f64>, max: Option<f64>) -> WidgetScale {
        WidgetScale {
            min,
            max,
            ..Default::default()
        }
    }

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "{a} vs {b}");
    }

    #[test]
    fn a_linear_and_a_log_control_round_trip() {
        let linear = knob(Some(0.0), Some(100.0));
        let log = WidgetScale {
            curve: Some("log".into()),
            ..knob(Some(20.0), Some(20000.0))
        };
        for t in [0.0, 0.25, 0.5, 0.75, 1.0] {
            close(linear.norm(linear.denorm(t)), t);
            close(log.norm(log.denorm(t)), t);
        }
    }

    #[test]
    fn an_unspecified_range_is_zero_to_one() {
        assert_eq!(knob(None, None).range(), (0.0, 1.0));
    }

    #[test]
    fn a_step_quantises_what_the_user_can_reach() {
        let stepped = WidgetScale {
            step: Some(1.0),
            ..knob(Some(0.0), Some(10.0))
        };
        assert_eq!(stepped.denorm(0.57), 6.0);
    }

    #[test]
    fn a_whole_number_step_reads_as_a_signed_integer_on_a_bipolar_range() {
        let semis = WidgetScale {
            step: Some(1.0),
            ..knob(Some(-24.0), Some(24.0))
        };
        assert_eq!(semis.text(Some(7.0)), "+7");
        assert_eq!(semis.text(Some(-12.0)), "-12");
        assert_eq!(semis.text(Some(0.0)), "0");
        let unipolar = WidgetScale {
            step: Some(1.0),
            ..knob(Some(1.0), Some(32.0))
        };
        assert_eq!(unipolar.text(Some(16.0)), "16");
    }

    #[test]
    fn a_continuous_control_keeps_its_decimals_and_a_missing_value_is_a_dash() {
        assert_eq!(knob(None, None).text(Some(0.5)), "0.50");
        assert_eq!(knob(None, None).text(None), "–");
        assert_eq!(knob(None, None).text(Some(100.5)), "101");
        assert_eq!(knob(None, None).text(Some(-100.5)), "-101");
        assert_eq!(knob(None, None).text(Some(0.125)), "0.13");
        assert_eq!(knob(None, None).text(Some(12.34)), "12.3");
    }

    #[test]
    fn a_typed_value_takes_either_decimal_mark_clamps_and_rounds_to_the_step() {
        assert_eq!(knob(None, None).typed("0.25"), Some(0.25));
        assert_eq!(knob(None, None).typed(" 0,5 "), Some(0.5));
        let w = WidgetScale {
            step: Some(1.0),
            ..knob(Some(-24.0), Some(24.0))
        };
        assert_eq!(w.typed("99"), Some(24.0));
        assert_eq!(w.typed("-99"), Some(-24.0));
        assert_eq!(w.typed("7.4"), Some(7.0));
        assert_eq!(w.typed(""), None);
        assert_eq!(w.typed("loud"), None);
        assert_eq!(w.typed("0x10"), Some(16.0));
    }

    #[test]
    fn a_crossfade_knob_keeps_the_power_constant() {
        let xf = WidgetScale {
            pair: Some("Dry Level".into()),
            bind: Some("Wet Level".into()),
            ..Default::default()
        };
        for t in [0.0, 0.1, 0.25, 0.5, 0.75, 1.0] {
            let wet = xf.denorm(t);
            close(wet * wet + xf.pair_value(wet).powi(2), 1.0);
            close(xf.norm(wet), t);
        }
        close(xf.denorm(0.0), 0.0);
        close(xf.pair_value(xf.denorm(0.0)), 1.0);
        close(xf.denorm(0.5), std::f64::consts::FRAC_1_SQRT_2);
        close(xf.typed("50").unwrap_or(f64::NAN), std::f64::consts::FRAC_1_SQRT_2);
        close(xf.typed("100").unwrap_or(f64::NAN), 1.0);
    }

    #[test]
    fn the_engine_s_mix_param_is_a_mix_knob() {
        let w = WidgetScale {
            bind: Some(MIX_PARAM.into()),
            ..Default::default()
        };
        assert!(w.is_mix_knob());
        assert!(!knob(None, None).is_mix_knob());
    }
}
