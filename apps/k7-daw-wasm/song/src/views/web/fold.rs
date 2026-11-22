// views/web/fold.rs — a comped event and a placed timeline folded into ONE clip each, after `foldComps` and `foldNests`.
// A reader would get wrong the folded clip's id: it is the EVENT's, which is also its first piece's.

use super::clip::{WebClip, WebTimelineRef};
use crate::clips::{ClipDoc, ClipKind, CompMark, EventDoc};
use crate::source::SourceError;
use indexmap::IndexMap;

/// JavaScript's `Number(text)` for the text a placement id ends with: empty is 0, anything not finite is none.
fn js_number(text: &str) -> Option<f64> {
    let t = text.trim();
    if t.is_empty() {
        return Some(0.0);
    }
    t.parse::<f64>().ok().filter(|n| n.is_finite())
}

/// `of` up to its last `@` and the text after it; with no `@`, JavaScript's `slice` of -1 drops the last character.
fn split_of(of: &str) -> (&str, &str) {
    match of.rfind('@') {
        Some(i) => (&of[..i], &of[i + 1..]),
        None => {
            let cut = of.char_indices().next_back().map_or(0, |(i, _)| i);
            (&of[..cut], of)
        }
    }
}

fn fold_group(of: &str, mut pieces: Vec<WebClip>) -> WebClip {
    pieces.sort_by(|a, b| a.start.total_cmp(&b.start));
    let first = &pieces[0];
    // The first piece whose end is the greatest.
    let mut last = first;
    for p in &pieces {
        if p.start + p.length > last.start + last.length {
            last = p;
        }
    }
    let (name, at) = split_of(crate::clips::without_repeat(of));
    let start = js_number(at).unwrap_or(first.start);
    let folded = WebClip {
        id: of.to_owned(),
        kind: first.kind,
        name: name.to_owned(),
        start,
        length: last.start + last.length - start,
        offset: None,
        source_from: None,
        source_to: None,
        source_name: None,
        loop_start: None,
        loop_end: None,
        warped: None,
        warp_off: None,
        disabled: pieces.iter().any(|p| p.disabled == Some(true)).then_some(true),
        fade_in: first.fade_in,
        fade_out: last.fade_out,
        frozen: None,
        event: pieces.iter().find_map(|p| p.event.clone()),
        comp: Some(CompMark {
            of: of.to_owned(),
            take: String::new(),
        }),
        pieces: None,
        crossfade: None,
        timeline: None,
        notes: Vec::new(),
    };
    WebClip {
        pieces: Some(pieces),
        ..folded
    }
}

/// The lane's clips with each comped event's pieces folded into one clip spanning them.
pub fn fold_comps(clips: Vec<WebClip>) -> Vec<WebClip> {
    if !clips.iter().any(|c| c.comp.is_some()) {
        return clips;
    }
    let mut groups: IndexMap<String, Vec<WebClip>> = IndexMap::new();
    let mut order: Vec<Result<WebClip, usize>> = Vec::with_capacity(clips.len());
    let mut seen: IndexMap<String, usize> = IndexMap::new();
    for c in clips {
        let Some(of) = c.comp.as_ref().map(|m| m.of.clone()) else {
            order.push(Ok(c));
            continue;
        };
        if !seen.contains_key(&of) {
            seen.insert(of.clone(), seen.len());
            order.push(Err(seen.len() - 1));
        }
        groups.entry(of).or_default().push(c);
    }
    let mut folded: Vec<Option<WebClip>> = groups.into_iter().map(|(of, p)| Some(fold_group(&of, p))).collect();
    order
        .into_iter()
        .filter_map(|s| match s {
            Ok(c) => Some(c),
            Err(i) => folded.get_mut(i).and_then(Option::take),
        })
        .collect()
}

/// The lane's clips as the browser reads them, each timeline event folded into one clip carrying its clips.
pub fn fold_nests(
    clips: &[ClipDoc],
    web: &mut dyn FnMut(&ClipDoc) -> Result<WebClip, SourceError>,
) -> Result<Vec<WebClip>, SourceError> {
    let mut out: Vec<Result<WebClip, usize>> = Vec::new();
    let mut index: IndexMap<String, usize> = IndexMap::new();
    let mut hosts: Vec<(WebClip, Vec<WebClip>)> = Vec::new();
    for c in clips {
        let Some(n) = &c.nest else {
            out.push(Ok(web(c)?));
            continue;
        };
        let at = match index.get(&n.of) {
            Some(i) => *i,
            None => {
                let host = WebClip {
                    id: n.of.clone(),
                    kind: ClipKind::Audio,
                    name: n.ref_.clone(),
                    start: n.start,
                    length: n.length,
                    offset: None,
                    source_from: None,
                    source_to: None,
                    source_name: None,
                    loop_start: None,
                    loop_end: None,
                    warped: None,
                    warp_off: None,
                    disabled: (n.event.as_ref().and_then(|e| e.disabled) == Some(true)).then_some(true),
                    fade_in: None,
                    fade_out: None,
                    frozen: None,
                    event: Some(n.event.clone().unwrap_or_else(EventDoc::default)),
                    comp: None,
                    pieces: None,
                    crossfade: None,
                    timeline: Some(WebTimelineRef {
                        ref_: n.ref_.clone(),
                        clips: Vec::new(),
                    }),
                    notes: Vec::new(),
                };
                hosts.push((host, Vec::new()));
                index.insert(n.of.clone(), hosts.len() - 1);
                out.push(Err(hosts.len() - 1));
                hosts.len() - 1
            }
        };
        let mut own = c.clone();
        own.nest = None;
        let w = web(&own)?;
        hosts[at].1.push(w);
    }
    let mut done: Vec<Option<WebClip>> = hosts
        .into_iter()
        .map(|(mut host, inner)| {
            let folded = fold_comps(inner);
            // MIDI only when all it holds is.
            if !folded.is_empty() && folded.iter().all(|c| c.kind == ClipKind::Midi) {
                host.kind = ClipKind::Midi;
            }
            let name = host.name.clone();
            let ref_ = host.timeline.take().map_or(name, |t| t.ref_);
            host.timeline = Some(WebTimelineRef { ref_, clips: folded });
            Some(host)
        })
        .collect();
    let lane = out
        .into_iter()
        .filter_map(|o| match o {
            Ok(c) => Some(c),
            Err(i) => done.get_mut(i).and_then(Option::take),
        })
        .collect();
    Ok(fold_comps(lane))
}
