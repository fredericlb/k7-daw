// buffer_text.rs — what the editor reads off the OPEN BUFFER's k7x text and writes back into it: the io and slice reads, a clip's own window, a slice's window.
// A reader would get wrong that a write answers splices of the buffer, like a keystroke: the caller applies them, the undo history keeps the gesture.

use crate::start_time::js_round;
use k7x::edit::{Document, EditError, PathSeg, Value, WriteOptions, parse};
use k7x::tokens::slices_of;
use k7x::{Bump, parse_tree};
use song::modules::types::{IO_MODULES, META_NS};
use serde::{Deserialize, Serialize};
use song_schema::{beats_to_position_on, parse_position_on};

fn key(k: &str) -> PathSeg {
    PathSeg::Key(k.to_owned())
}

// ---- the reads one parse serves ------------------------------------------------------------------

/// An input's channels as authored: a list of whole channel numbers, or a name.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(untagged)]
pub enum Channels {
    List(Vec<f64>),
    Name(String),
}

/// What the editor reads off the open buffer beyond its text.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct BufferReads {
    /// The slice symbols live in this buffer, so the docked slicer's drags have somewhere to write.
    pub slices: bool,
    /// The file's `io` kind (`in`, `out`, …), or None for a module that runs a plugin.
    pub io: Option<String>,
    /// An input's channels as authored, read only when `io` is `in`.
    pub channels: Option<Channels>,
}

/// `\bio\b` over ASCII word characters.
fn names_io_word(text: &str) -> bool {
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    text.match_indices("io").any(|(i, _)| {
        !text[..i].chars().next_back().is_some_and(word) && !text[i + 2..].chars().next().is_some_and(word)
    })
}

/// JavaScript's `Number(value)` for what a channel list holds: a number, a numeric string, a boolean or null.
pub fn js_number_of(v: &Value) -> f64 {
    match v {
        Value::Num(n) => *n,
        Value::Bool(b) => f64::from(u8::from(*b)),
        Value::Null => 0.0,
        Value::Str(s) => {
            let t = s.trim_matches(k7x::edit::is_js_space);
            if t.is_empty() {
                return 0.0;
            }
            let radix = |p: &str, r: u32| t.strip_prefix(p).and_then(|d| u64::from_str_radix(d, r).ok()).map(|n| n as f64);
            if let Some(n) = radix("0x", 16).or_else(|| radix("0X", 16)).or_else(|| radix("0o", 8)).or_else(|| radix("0b", 2)) {
                return n;
            }
            let body = t.strip_prefix(['+', '-']).unwrap_or(t);
            let plain = body.starts_with(|c: char| c.is_ascii_digit() || c == '.')
                && body.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | 'e' | 'E' | '+' | '-'));
            if plain { t.parse().unwrap_or(f64::NAN) } else if body == "Infinity" { if t.starts_with('-') { f64::NEG_INFINITY } else { f64::INFINITY } } else { f64::NAN }
        }
        _ => f64::NAN,
    }
}

fn channel_of(doc: Option<&Document>) -> Option<Channels> {
    match doc?.get_in(&[key("channel")]).ok()?? {
        Value::Str(s) if !s.is_empty() => Some(Channels::Name(s)),
        Value::List(items) => {
            let ns: Vec<f64> = items.iter().map(js_number_of).filter(|n| n.fract() == 0.0 && *n > 0.0 && n.is_finite()).collect();
            (!ns.is_empty()).then_some(Channels::List(ns))
        }
        _ => None,
    }
}

fn io_module(name: &str) -> Option<&'static str> {
    IO_MODULES.iter().find(|(n, _)| *n == name).map(|(_, io)| *io)
}

/// A module's `io(…)` direction from THREE sources, because a module rarely says it: its own key, an `@meta` base, its path.
fn io_of(doc: Option<&Document>, path: &str) -> Option<String> {
    let string = |k: &str| match doc.and_then(|d| d.get_in(&[key(k)]).ok().flatten()) {
        Some(Value::Str(s)) if !s.is_empty() => Some(s),
        _ => None,
    };
    if let Some(io) = string("io") {
        return Some(io);
    }
    if let Some(base) = string("extends")
        && let Some(name) = base.strip_prefix(&format!("{META_NS}/"))
        && let Some(io) = io_module(name)
    {
        return Some(io.to_owned());
    }
    let parts: Vec<&str> = path.split(['/', '\\']).collect();
    parts.windows(2).find(|w| w[0] == "io" && io_module(w[1]).is_some()).and_then(|w| io_module(w[1]).map(str::to_owned))
}

/// The three reads, parsing at most once and only when the text can hold the keys a read looks for.
#[must_use]
pub fn buffer_reads(text: &str, path: &str) -> BufferReads {
    let want_slices = text.contains("rack_type");
    let want_io = names_io_word(text) || text.contains("extends");
    let doc = (want_slices || want_io).then(|| Document::new(text)).filter(|d| d.errors().is_empty());
    let slices = doc.is_some() && want_slices && scan_slices(text).is_some();
    // A path alone can make a file an input, so the io read runs even when nothing was parsed.
    let io = io_of(doc.as_ref(), path);
    let channels = if io.as_deref() == Some("in") { channel_of(doc.as_ref()) } else { None };
    BufferReads { slices, io, channels }
}

// ---- the slice rack's symbols ----------------------------------------------------------------------

/// Each symbol's window and where its numbers and name sit; None for a file that is no slice rack or does not parse.
#[must_use]
pub fn scan_slices(text: &str) -> Option<Vec<k7x::tokens::SliceSym>> {
    if !text.contains("rack_type") {
        return None;
    }
    let bump = Bump::new();
    let root = parse_tree(text, &bump).ok()?;
    slices_of(&root, text)
}

/// Seconds to 0.1 ms, the precision the slice handles commit at.
fn tidy(s: f64) -> f64 {
    js_round(s * 10000.0) / 10000.0
}

/// Sets one symbol's `from` and `to` in the buffer, the two numbers and nothing else; false when no such symbol is there.
///
/// # Errors
/// A text that does not read.
pub fn set_slice_window(doc: &mut Document, symbol: &str, from: f64, to: f64) -> Result<bool, EditError> {
    if !scan_slices(doc.text()).is_some_and(|s| s.iter().any(|x| x.symbol == symbol)) {
        return Ok(false);
    }
    for (k, v) in [("from", from), ("to", to)] {
        let path = [key("rack"), key("symbols"), key(symbol), key(k)];
        doc.set_in(&path, &Value::Num(tidy(v)), WriteOptions::default())?;
    }
    Ok(true)
}

// ---- a clip file's own window ----------------------------------------------------------------------

/// The project grid a timecode edge needs, and the one a dragged edge is written back on.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ClipGrid {
    pub tempo: f64,
    pub num: f64,
    pub den: f64,
}

/// One written edge: seconds as a number, a timecode as its string.
#[derive(Clone, Debug, PartialEq)]
pub enum Edge {
    Seconds(f64),
    Timecode(String),
}

/// The edges a clip file states at its root, as written; none for a text that does not read.
#[must_use]
pub fn scan_clip_window(text: &str) -> (Option<Edge>, Option<Edge>) {
    let Ok(Value::Map(entries)) = parse(text) else {
        return (None, None);
    };
    let edge = |k: &str| match entries.iter().find(|(key, _)| key == k) {
        Some((_, Value::Num(n))) => Some(Edge::Seconds(*n)),
        Some((_, Value::Str(s))) => Some(Edge::Timecode(s.clone())),
        _ => None,
    };
    (edge("from"), edge("to"))
}

/// A written edge in SECONDS, or None when it is absent or not a timecode.
fn edge_seconds(raw: Option<&Edge>, g: ClipGrid) -> Option<f64> {
    match raw? {
        Edge::Timecode(s) if s.is_empty() => None,
        Edge::Seconds(n) => (*n >= 0.0).then_some(*n),
        Edge::Timecode(s) => parse_position_on(s, g.num, g.den).ok().map(|beats| beats * 60.0 / g.tempo),
    }
}

/// One band of the window to draw over the source: `to` None is the end of the file, which only the view can place.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct WindowBand {
    pub from: f64,
    pub to: Option<f64>,
    pub hollow: bool,
}

/// The window as ONE band: HOLLOW with neither edge written, since the edges are still the HANDLES; none when not editable.
#[must_use]
pub fn clip_window_band(text: &str, g: ClipGrid, editable: bool) -> Option<WindowBand> {
    let (from, to) = scan_clip_window(text);
    let (from, to) = (edge_seconds(from.as_ref(), g), edge_seconds(to.as_ref(), g));
    if from.is_none() && to.is_none() {
        return editable.then_some(WindowBand {
            from: 0.0,
            to: None,
            hollow: true,
        });
    }
    Some(WindowBand {
        from: from.unwrap_or(0.0),
        to,
        hollow: false,
    })
}

/// How to write a dragged edge: the way it was already written. A number holds still, a timecode follows the grid.
fn spell(seconds: f64, was: Option<&Edge>, g: ClipGrid) -> Value {
    if matches!(was, Some(Edge::Timecode(_)))
        && let Ok(tc) = beats_to_position_on(seconds * g.tempo / 60.0, g.num, g.den)
    {
        return Value::Str(tc);
    }
    Value::Num(tidy(seconds))
}

/// Rewrites the clip's `from` and `to`: a `from` of 0 and a `to` that is none say it by leaving the key out.
///
/// # Errors
/// A text that does not read.
pub fn set_clip_window(doc: &mut Document, from: f64, to: Option<f64>, g: ClipGrid) -> Result<(), EditError> {
    let (was_from, was_to) = scan_clip_window(doc.text());
    if from > 0.0 {
        doc.set_in(&[key("from")], &spell(from, was_from.as_ref(), g), WriteOptions::default())?;
    } else {
        doc.delete_in(&[key("from")], false)?;
    }
    match to.filter(|t| t.is_finite()) {
        Some(t) => doc.set_in(&[key("to")], &spell(t, was_to.as_ref(), g), WriteOptions::default())?,
        None => {
            doc.delete_in(&[key("to")], false)?;
        }
    }
    Ok(())
}
