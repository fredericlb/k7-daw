// clips/distinct.rs — one id per placed clip of a track, though a lane may place one material at one beat more than once.
// A reader would get wrong that the suffix follows the CLIP: it follows the EVENT, so a comp's pieces and its `comp.of` stay one event.

use super::doc::ClipDoc;
use std::collections::{HashMap, HashSet};

/// What separates a placement id from the ordinal of its repeat: `<ref>@<start>#2`, as the engine's own safety net spells it.
pub const REPEAT_SEP: char = '#';

/// A lane's clips with every id unique, and the events that repeat an earlier one exactly.
#[derive(Debug, Default)]
pub struct Distinct {
    pub clips: Vec<ClipDoc>,
    /// `(material, start in beats)` of each event placed again where an identical one already plays.
    pub twins: Vec<(String, f64)>,
}

/// `id` without the `#<n>` a repeated placement carries; any other text is returned whole.
pub fn without_repeat(id: &str) -> &str {
    match id.rsplit_once(REPEAT_SEP) {
        Some((head, n)) if !head.is_empty() && !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => head,
        _ => id,
    }
}

/// The first `key#n`, from 2, that `seen` does not hold.
fn next_free(seen: &HashSet<String>, key: &str) -> String {
    (2usize..)
        .map(|n| format!("{key}{REPEAT_SEP}{n}"))
        .find(|k| !seen.contains(k))
        .unwrap_or_default()
}

/// The event a clip belongs to: a comp's piece names it, any other clip is it.
fn event_key(c: &ClipDoc) -> &str {
    c.comp.as_ref().map_or(c.id.as_str(), |m| m.of.as_str())
}

/// Two clips that play the same thing the same way, their ids aside.
fn same_but_ids(a: &ClipDoc, b: &ClipDoc) -> bool {
    let bare = |c: &ClipDoc| ClipDoc {
        id: String::new(),
        comp: c.comp.as_ref().map(|m| super::doc::CompMark {
            of: String::new(),
            take: m.take.clone(),
        }),
        ..c.clone()
    };
    bare(a) == bare(b)
}

/// The events of one track in order, each its clips as placed, flattened with a repeat's ids suffixed `#2`, `#3`…
pub fn distinct_ids(events: impl IntoIterator<Item = Vec<ClipDoc>>) -> Distinct {
    let mut out = Distinct::default();
    let mut keys: HashSet<String> = HashSet::new();
    let mut ids: HashSet<String> = HashSet::new();
    // Where each event key's first placement sits in `out.clips`, to tell a twin from a mere repeat of the id.
    let mut first_at: HashMap<String, (usize, usize)> = HashMap::new();
    for mut clips in events {
        let Some(first) = clips.first() else { continue };
        let key = event_key(first).to_owned();
        if keys.insert(key.clone()) {
            first_at.insert(key, (out.clips.len(), clips.len()));
        } else {
            if let Some(&(at, n)) = first_at.get(&key) {
                let earlier = out.clips.get(at..at + n).unwrap_or(&[]);
                if n == clips.len() && earlier.iter().zip(&clips).all(|(a, b)| same_but_ids(a, b)) {
                    out.twins.push((first.name.clone(), first.start));
                }
            }
            let renamed = next_free(&keys, &key);
            let tag = &renamed[key.len()..];
            for c in &mut clips {
                c.id.push_str(tag);
                if let Some(m) = c.comp.as_mut() {
                    m.of.push_str(tag);
                }
            }
            keys.insert(renamed);
        }
        for mut c in clips {
            if !ids.insert(c.id.clone()) {
                c.id = next_free(&ids, &c.id);
                ids.insert(c.id.clone());
            }
            out.clips.push(c);
        }
    }
    out
}
