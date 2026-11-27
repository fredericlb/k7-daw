// library_pins.rs — a workspace's `_library-pins.k7x`: library name to the version it pins, read leniently and written in name order.
// A reader would get wrong that an entry that is no version is dropped on read, so the library reads as unpinned.

use crate::error::Res;
use k7x::edit::{StringifyOptions, Value, parse, stringify};

/// The folder name of the draft an admin tests, the one non-number version.
pub const DRAFT_VERSION: &str = "draft";

/// What a library is pinned at: a published number, or the draft.
#[derive(Clone, Debug, PartialEq)]
pub enum Pin {
    Number(f64),
    Draft,
}

fn whole(v: &Value) -> Option<f64> {
    match v {
        Value::Num(n) if n.fract() == 0.0 && *n >= 1.0 => Some(*n),
        _ => None,
    }
}

/// Reads `_library-pins.k7x`, in the file's order; a text that does not read is no pins.
#[must_use]
pub fn parse_pins(text: &str) -> Vec<(String, Pin)> {
    let Ok(Value::Map(entries)) = parse(text) else {
        return vec![];
    };
    entries
        .into_iter()
        .filter_map(|(name, v)| match &v {
            Value::Str(s) if s == DRAFT_VERSION => Some((name, Pin::Draft)),
            other => whole(other).map(|n| (name, Pin::Number(n))),
        })
        .collect()
}

fn utf16_order(a: &str, b: &str) -> std::cmp::Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// Writes `_library-pins.k7x`, libraries in name order so a diff stays one line.
///
/// # Errors
/// A value that does not print.
pub fn write_pins(pins: &[(String, Pin)]) -> Res<String> {
    let mut sorted: Vec<&(String, Pin)> = pins.iter().collect();
    sorted.sort_by(|a, b| utf16_order(&a.0, &b.0));
    let value = Value::object(
        sorted
            .into_iter()
            .map(|(n, p)| {
                let v = match p {
                    Pin::Number(x) => Value::Num(*x),
                    Pin::Draft => Value::Str(DRAFT_VERSION.to_owned()),
                };
                (n.clone(), v)
            })
            .collect(),
    );
    Ok(stringify(
        &value,
        &StringifyOptions {
            header: &[],
            inline: None,
        },
    )?)
}
