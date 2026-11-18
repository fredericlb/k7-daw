// modules/scalars.rs — the checked strings and numbers a module file is made of; each regex of the schema matched by hand.
// A reader would get wrong JavaScript's classes: `\S` and `.` are not Rust's, and `$` has no trailing-newline slack.

use super::types::{all_non_space, is_name};
use crate::files::scalar::{checked_num, checked_str};
use serde::Serialize;
use serde::de::{self, Deserialize, Deserializer};

checked_num!(
    Volume(f64),
    |v| (-60.0..=12.0).contains(v),
    "expected a level from -60 to 12 dB"
);

checked_str!(
    EngineName,
    is_name,
    "an engine is a folder name in the module's engines/"
);
checked_str!(
    ShaderName,
    is_name,
    "a shader is a folder name in the module's shaders/ — letters, digits, '_' and '-'"
);
checked_str!(ModelUrl, is_model_url, "model_url is an http(s) URL");
checked_str!(StrudelPath, is_strudel, "expected algo/<name>.strudel");
checked_str!(LutPath, is_lut, "a LUT is a .cube file");
checked_str!(HexColor, is_hex_color, "a #rgb, #rrggbb or #rrggbbaa colour");

fn is_model_url(s: &str) -> bool {
    let rest = s.strip_prefix("https://").or_else(|| s.strip_prefix("http://"));
    rest.is_some_and(all_non_space)
}

/// `algo/` then one or more characters that are not line terminators, then `.strudel`.
fn is_strudel(s: &str) -> bool {
    let Some(rest) = s.strip_prefix("algo/") else {
        return false;
    };
    let Some(mid) = rest.strip_suffix(".strudel") else {
        return false;
    };
    !mid.is_empty() && !mid.chars().any(|c| matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}'))
}

fn is_lut(s: &str) -> bool {
    s.len() >= 5 && s.get(s.len() - 5..).is_some_and(|t| t.eq_ignore_ascii_case(".cube"))
}

fn is_hex_color(s: &str) -> bool {
    let Some(h) = s.strip_prefix('#') else { return false };
    matches!(h.len(), 3 | 6 | 8) && h.bytes().all(|b| b.is_ascii_hexdigit())
}

/// A map key refused by a rule, so the error lands on the key.
pub trait KeyRule {
    fn ok(key: &str) -> bool;
    const MESSAGE: &'static str;
}

#[derive(Clone, Debug, PartialEq)]
pub struct TapKey;
impl KeyRule for TapKey {
    fn ok(key: &str) -> bool {
        is_name(key)
    }
    const MESSAGE: &'static str = "a name: letters, digits, '_' and '-'";
}

#[derive(Clone, Debug, PartialEq)]
pub struct EngineKey;
impl KeyRule for EngineKey {
    fn ok(key: &str) -> bool {
        is_name(key)
    }
    const MESSAGE: &'static str = "an engine is a folder name in the module's engines/";
}
