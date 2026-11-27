// doc/arrange.rs — a track name taken apart and the order of lanes, after song-schema's trackKind.ts (decision 325).
// A reader would get wrong that an order is a list of INDEXES into the caller's names, and that no step of it is quadratic.

use super::category::{TrackCategory, category_of_text};
use std::collections::HashMap;

/// The instrument families in layout order; the discriminant is the rank.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrackFamily {
    Voice,
    Choir,
    Drums,
    Bass,
    Guitar,
    Piano,
    Synth,
    Sax,
}

/// A track with no prefix, or an unknown one, sorts after every named family.
const UNKNOWN_RANK: usize = 8;

/// The known instrument codes with the family each means and the label a row's tooltip shows (decision 357: a plain name may spell an `Object.prototype` key).
pub const TRACK_PREFIXES: [(&str, TrackFamily, &str); 12] = [
    ("vo", TrackFamily::Voice, "voix"),
    ("ch", TrackFamily::Choir, "choeurs"),
    ("da", TrackFamily::Drums, "batterie acoustique"),
    ("ds", TrackFamily::Drums, "batterie synthétique"),
    ("be", TrackFamily::Bass, "basse électrique"),
    ("ba", TrackFamily::Bass, "basse acoustique"),
    ("bs", TrackFamily::Bass, "basse synth"),
    ("ge", TrackFamily::Guitar, "guitare électrique"),
    ("ga", TrackFamily::Guitar, "guitare acoustique"),
    ("pi", TrackFamily::Piano, "piano"),
    ("sy", TrackFamily::Synth, "synthé"),
    ("sx", TrackFamily::Sax, "saxophone"),
];

impl TrackFamily {
    /// The family's name, as a row's data spells it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            TrackFamily::Voice => "voice",
            TrackFamily::Choir => "choir",
            TrackFamily::Drums => "drums",
            TrackFamily::Bass => "bass",
            TrackFamily::Guitar => "guitar",
            TrackFamily::Piano => "piano",
            TrackFamily::Synth => "synth",
            TrackFamily::Sax => "sax",
        }
    }
}

/// A track name taken apart: `[position.][prefix.]label`, both leading segments optional and independent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParsedName<'a> {
    /// The 1-based slot the author claims, as JavaScript's `Number` reads the digits (past 308 of them it is infinite).
    pub position: Option<f64>,
    pub prefix: Option<(&'static str, TrackFamily)>,
    pub label: &'a str,
}

/// JavaScript's `.` matches anything but these four.
fn line_terminator(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

/// Take a name apart, case-insensitively; an unrecognised leading segment stays in the label.
pub fn parse_track_name(name: &str) -> ParsedName<'_> {
    let mut rest = name;
    let mut position = None;
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    let after = rest.get(digits..).unwrap_or("");
    if digits > 0
        && let Some(tail) = after.strip_prefix('.')
        && tail.chars().next().is_some_and(|c| !line_terminator(c))
    {
        position = rest.get(..digits).and_then(|d| d.parse::<f64>().ok());
        rest = tail;
    }
    let mut prefix = None;
    if let Some(i) = rest.find('.')
        && i > 0
        && let Some(head) = rest.get(..i)
        && let Some(&(code, family, _)) = TRACK_PREFIXES.iter().find(|(p, _, _)| head.eq_ignore_ascii_case(p))
    {
        prefix = Some((code, family));
        rest = rest.get(i + 1..).unwrap_or("");
    }
    ParsedName {
        position,
        prefix,
        label: rest,
    }
}

/// The prefix, family and label a track name declares, or None when it declares none.
#[must_use]
pub fn track_kind(name: &str) -> Option<(&'static str, TrackFamily, &'static str)> {
    let (code, _) = parse_track_name(name).prefix?;
    TRACK_PREFIXES.iter().find(|(p, _, _)| *p == code).copied()
}

/// The name with its metadata stripped, what a view SHOWS; the whole name when nothing would remain.
#[must_use]
pub fn track_label(name: &str) -> &str {
    let label = parse_track_name(name).label;
    if label.is_empty() { name } else { label }
}

/// A category's place among the families, for a name that declares no prefix; `other` has none.
fn family_rank_of(c: TrackCategory) -> usize {
    match c {
        TrackCategory::Vox => TrackFamily::Voice as usize,
        TrackCategory::Drums => TrackFamily::Drums as usize,
        TrackCategory::Bass => TrackFamily::Bass as usize,
        TrackCategory::Guitar => TrackFamily::Guitar as usize,
        TrackCategory::Keys => TrackFamily::Piano as usize,
        TrackCategory::Synth => TrackFamily::Synth as usize,
        TrackCategory::Other => UNKNOWN_RANK,
    }
}

/// Where a track sorts: its family's index, else what the name says, else after every family.
pub fn track_kind_rank(name: &str) -> usize {
    let parsed = parse_track_name(name);
    if let Some((_, family)) = parsed.prefix {
        return family as usize;
    }
    category_of_text(parsed.label).map_or(UNKNOWN_RANK, family_rank_of)
}

/// The first free slot at or after a place, found through a union of taken slots to the next one.
struct NextFree(Vec<usize>);

impl NextFree {
    fn new(n: usize) -> NextFree {
        NextFree((0..=n).collect())
    }

    fn find(&mut self, from: usize) -> usize {
        let mut root = from;
        while self.0.get(root).copied().unwrap_or(root) != root {
            root = self.0.get(root).copied().unwrap_or(root);
        }
        let mut at = from;
        while at != root {
            let next = self.0.get(at).copied().unwrap_or(root);
            if let Some(slot) = self.0.get_mut(at) {
                *slot = root;
            }
            at = next;
        }
        root
    }

    fn take(&mut self, slot: usize) {
        if let Some(p) = self.0.get_mut(slot) {
            *p = slot + 1;
        }
    }
}

/// The indexes of `names` ordered by instrument family, pinned positions honoured first (stable).
pub fn sort_tracks_by_kind(names: &[&str]) -> Vec<usize> {
    let n = names.len();
    let mut slots: Vec<Option<usize>> = vec![None; n];
    let mut pinned: Vec<(usize, usize)> = Vec::new();
    let mut loose: Vec<(usize, usize)> = Vec::new();
    for (i, name) in names.iter().enumerate() {
        match parse_track_name(name).position {
            None => loose.push((track_kind_rank(name), i)),
            // 1-based and clamped: "0." and "1." both mean first, anything past the end means last.
            Some(p) => pinned.push(((p - 1.0).clamp(0.0, (n.saturating_sub(1)) as f64) as usize, i)),
        }
    }
    pinned.sort();
    let mut free = NextFree::new(n);
    for (at, i) in pinned {
        let mut slot = free.find(at);
        if slot >= n {
            slot = free.find(0);
        }
        if let Some(s) = slots.get_mut(slot) {
            *s = Some(i);
        }
        free.take(slot);
    }
    loose.sort();
    let mut next = 0;
    for (_, i) in loose {
        while slots.get(next).is_some_and(Option::is_some) {
            next += 1;
        }
        if let Some(s) = slots.get_mut(next) {
            *s = Some(i);
        }
    }
    slots.into_iter().flatten().collect()
}

/// The order `trackOrder` STATES, or the one the names imply: one rule, never both at once.
pub fn order_tracks<S: AsRef<str>>(names: &[&str], stated: &[S]) -> Vec<usize> {
    // The last repeat of a name wins, as `new Map(pairs)` does.
    let at: HashMap<&str, usize> = stated.iter().enumerate().map(|(i, s)| (s.as_ref(), i)).collect();
    if at.is_empty() {
        return sort_tracks_by_kind(names);
    }
    let mut listed: Vec<(usize, usize)> = Vec::new();
    let mut rest: Vec<usize> = Vec::new();
    for (i, name) in names.iter().enumerate() {
        match at.get(name) {
            Some(&k) => listed.push((k, i)),
            None => rest.push(i),
        }
    }
    listed.sort();
    // A lane the list forgot follows the ones it names, in family order.
    let rest_names: Vec<&str> = rest.iter().filter_map(|&i| names.get(i).copied()).collect();
    let mut out: Vec<usize> = listed.into_iter().map(|(_, i)| i).collect();
    out.extend(
        sort_tracks_by_kind(&rest_names)
            .into_iter()
            .filter_map(|j| rest.get(j).copied()),
    );
    out
}
