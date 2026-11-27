// doc/category.rs — a track's category from its name, and the colour its category says it with (decisions 326, 327).
// A reader would get wrong that there is no regex: the words are a table of needles with ASCII word boundaries, as `\b` reads them.

use super::arrange::parse_track_name;
use indexmap::IndexMap;

/// The categories, in the order their tracks are laid out.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TrackCategory {
    Vox,
    Drums,
    Bass,
    Guitar,
    Keys,
    Synth,
    Other,
}

/// The categories' names, in the order their tracks are laid out.
pub const TRACK_CATEGORIES: [&str; 7] = ["vox", "drums", "bass", "guitar", "keys", "synth", "other"];

/// A needle and whether a word edge is required before it and after it (`\b`).
type Word = (&'static str, bool, bool);

const N: bool = false;
const B: bool = true;

/// Tested in this order, since "acoustic drum kit" is drums before it is anything acoustic.
const WORDS: [(TrackCategory, &[Word]); 6] = [
    (
        TrackCategory::Drums,
        &[
            ("drum", N, N),
            ("batterie", N, N),
            ("kick", N, N),
            ("snare", N, N),
            ("hi-hat", N, N),
            ("hihat", N, N),
            ("hats", B, B),
            ("hat", B, B),
            ("perc", N, N),
            ("cymbal", N, N),
            ("ride", B, B),
            ("crash", N, N),
            ("toms", B, B),
            ("tom", B, B),
            ("kit", B, B),
            ("505", B, B),
            ("606", B, B),
            ("707", B, B),
            ("808", B, B),
            ("909", B, B),
            ("breakbeat", N, N),
        ],
    ),
    (
        TrackCategory::Vox,
        &[
            ("vox", B, B),
            ("voc", B, B),
            ("vocals", B, B),
            ("vocal", N, N),
            ("voice", B, B),
            ("voix", B, B),
            ("chant", N, N),
            ("choir", N, N),
            ("choeur", N, N),
            ("chœeur", N, N),
            ("singer", N, N),
        ],
    ),
    (TrackCategory::Bass, &[("bass", N, N), ("basse", N, N), ("sub", B, B)]),
    (
        TrackCategory::Guitar,
        &[
            ("guit", N, N),
            ("gtr", B, B),
            ("gt", B, B),
            ("strat", N, N),
            ("tele", B, B),
            ("folk", B, B),
            ("riff", N, N),
            ("amp", B, B),
        ],
    ),
    (
        TrackCategory::Keys,
        &[
            ("piano", N, N),
            ("keys", B, B),
            ("key", B, B),
            ("wurli", N, N),
            ("rhodes", N, N),
            ("organ", N, N),
            ("orgue", N, N),
            ("clav", B, N),
            ("ep", B, B),
            ("harpsi", N, N),
            ("celesta", N, N),
            ("mellotron", N, N),
        ],
    ),
    (
        TrackCategory::Synth,
        &[
            ("synth", N, N),
            ("poly", B, N),
            ("pad", B, B),
            ("arp", B, N),
            ("juno", N, N),
            ("jun", B, B),
            ("moog", N, N),
            ("pluck", N, N),
            ("lead", B, B),
            ("surge", N, N),
            ("mu-", N, N),
            ("plaits", N, N),
            ("braids", N, N),
            ("tides", N, N),
        ],
    ),
];

fn is_word(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Whether `needle` sits in `text` with the word edges it asks for; the search resumes after each refused hit.
fn has_word(text: &str, (needle, left, right): Word) -> bool {
    let bytes = text.as_bytes();
    let mut from = 0;
    while let Some(rest) = text.get(from..) {
        let Some(found) = rest.find(needle) else { return false };
        let at = from + found;
        let end = at + needle.len();
        let before = at
            .checked_sub(1)
            .and_then(|i| bytes.get(i))
            .is_some_and(|&b| is_word(b));
        let after = bytes.get(end).is_some_and(|&b| is_word(b));
        if !(left && before) && !(right && after) {
            return true;
        }
        from = at + 1;
    }
    false
}

/// The text lowercased as `toLowerCase` does it, each run of `_` and `.` one space.
fn squeezed(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_run = false;
    for c in text.to_lowercase().chars() {
        if c == '_' || c == '.' {
            if !in_run {
                out.push(' ');
            }
            in_run = true;
        } else {
            out.push(c);
            in_run = false;
        }
    }
    out
}

/// The category a text names, or none when it names none.
pub fn category_of_text(text: &str) -> Option<TrackCategory> {
    let t = squeezed(text);
    WORDS
        .iter()
        .find(|(_, words)| words.iter().any(|w| has_word(&t, *w)))
        .map(|(c, _)| *c)
}

/// A track's category: its declared prefix, else words in its name, else what its instrument says (`hints`), else `other`.
pub fn track_category<S: AsRef<str>>(name: &str, hints: &[S]) -> TrackCategory {
    use super::arrange::TrackFamily as F;
    let parsed = parse_track_name(name);
    if let Some((_, family)) = parsed.prefix {
        return match family {
            F::Voice | F::Choir => TrackCategory::Vox,
            F::Drums => TrackCategory::Drums,
            F::Bass => TrackCategory::Bass,
            F::Guitar => TrackCategory::Guitar,
            F::Piano => TrackCategory::Keys,
            F::Synth => TrackCategory::Synth,
            F::Sax => TrackCategory::Other,
        };
    }
    if let Some(said) = category_of_text(parsed.label) {
        return said;
    }
    hints
        .iter()
        .find_map(|h| category_of_text(h.as_ref()))
        .unwrap_or(TrackCategory::Other)
}

/// Each category's hue and saturation, and the lightness span its shades are spread over.
fn palette(c: TrackCategory) -> (f64, f64, f64, f64) {
    match c {
        TrackCategory::Vox => (330.0, 75.0, 52.0, 76.0),
        TrackCategory::Drums => (2.0, 72.0, 40.0, 64.0),
        TrackCategory::Bass => (25.0, 48.0, 28.0, 50.0),
        TrackCategory::Guitar => (130.0, 50.0, 34.0, 60.0),
        TrackCategory::Keys => (212.0, 72.0, 42.0, 68.0),
        TrackCategory::Synth => (272.0, 58.0, 46.0, 72.0),
        TrackCategory::Other => (220.0, 6.0, 44.0, 70.0),
    }
}

fn hsl(h: f64, s: f64, l: f64) -> String {
    let a = (s / 100.0) * (l / 100.0).min(1.0 - l / 100.0);
    let f = |n: f64| {
        let k = (n + h / 30.0) % 12.0;
        let c = l / 100.0 - a * (k - 3.0).min(9.0 - k).clamp(-1.0, 1.0);
        format!("{:02x}", (c * 255.0).round() as u8)
    };
    format!("#{}{}{}", f(0.0), f(8.0), f(4.0))
}

/// The `i`th of `n` shades of a category: lightness stepped across its span, the hue nudged so neighbours differ.
pub fn category_shade(c: TrackCategory, i: usize, n: usize) -> String {
    let (h, s, lo, hi) = palette(c);
    if n <= 1 {
        return hsl(h, s, (lo + hi) / 2.0);
    }
    let t = i as f64 / (n - 1) as f64;
    let nudge = (t - 0.5) * 12.0;
    hsl((h + nudge + 360.0) % 360.0, s, lo + (hi - lo) * t)
}

/// Every track's colour, by id: shaded by its place among its own category in the given order; a repeated id keeps the last.
pub fn auto_colors<'a>(tracks: &[(&'a str, TrackCategory)]) -> IndexMap<&'a str, String> {
    let mut count: IndexMap<TrackCategory, usize> = IndexMap::new();
    for (_, c) in tracks {
        *count.entry(*c).or_insert(0) += 1;
    }
    let mut seen: IndexMap<TrackCategory, usize> = IndexMap::new();
    let mut out = IndexMap::new();
    for (id, c) in tracks {
        let slot = seen.entry(*c).or_insert(0);
        let i = *slot;
        *slot += 1;
        out.insert(*id, category_shade(*c, i, count.get(c).copied().unwrap_or(1)));
    }
    out
}
