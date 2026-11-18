// clips/timelines.rs — a timeline placed as one event, expanded into the events it holds, each shifted and cut at its end.
// A reader would get wrong that the expansion happens BEFORE an event is routed to a half: one timeline feeds both of its lane's outputs.

use super::doc::{ClipDoc, ClipKind, EventDoc, NestDoc, TimelineDoc, TimelineLaneDoc};
use super::id::{NEST_SEP, js_max, js_min, placement_id, timeline_lane_id};
use super::placer::{Leaf, Placer, Within};
use super::{MAX_NEST, error_in};
use crate::files::Placement;
use std::sync::Arc;

/// The placement fields a timeline event may not carry: its extent is its timeline's, fixed.
pub const FIXED_FIELDS: [&str; 9] = [
    "from",
    "to",
    "loop",
    "fadeIn",
    "fadeOut",
    "warp",
    "takes",
    "comp",
    "crossfades",
];

/// The fields `p` carries that a timeline event may not.
pub fn fixed_fields_of(p: &Placement) -> Vec<&'static str> {
    let here = [
        p.from.is_some(),
        p.to.is_some(),
        p.loop_.is_some(),
        p.fade_in.is_some(),
        p.fade_out.is_some(),
        p.warp.is_some(),
        p.takes.is_some(),
        p.comp.is_some(),
        p.crossfades.is_some(),
    ];
    FIXED_FIELDS
        .iter()
        .zip(here)
        .filter_map(|(k, h)| h.then_some(*k))
        .collect()
}

/// An expanded event's clips moved to where its timeline plays, and cut at its end.
pub(super) fn place_within(clips: Vec<ClipDoc>, w: &Within) -> Vec<ClipDoc> {
    clips
        .into_iter()
        .filter_map(|c| {
            let start = c.start + w.offset;
            let room = w.end - start;
            if room <= 1e-9 {
                return None;
            }
            // A length of 0 is the source's own, which only the engine knows: the room left is an upper bound it clamps.
            let length = if c.length > 0.0 { js_min(c.length, room) } else { room };
            let db = if c.kind == ClipKind::Audio {
                c.gain_db.unwrap_or(0.0) + w.gain
            } else {
                0.0
            };
            let gain_db = js_max(-6.0, js_min(6.0, db));
            let prefix = |s: &str| format!("{}{s}", w.prefix);
            let mut out = ClipDoc {
                id: prefix(&c.id),
                start,
                length,
                fade_out: js_min(c.fade_out, length),
                gain_db: (gain_db != 0.0 && !gain_db.is_nan()).then_some(gain_db),
                nest: Some(w.nest.clone()),
                ..c
            };
            if w.disabled {
                out.disabled = Some(true);
            }
            if let Some(m) = out.comp.as_mut() {
                m.of = prefix(&m.of);
            }
            Some(out)
        })
        .collect()
}

fn ref_of(p: &Placement) -> &str {
    p.ref_.as_ref().map_or("", |r| r.0.as_str())
}

impl<'a> Placer<'a> {
    /// A timeline's fixed length in beats: its `length`, else its content's end rounded up to the bar.
    pub fn timeline_length(&mut self, id: &str) -> f64 {
        self.length_in(id, &mut Vec::new()).0
    }

    /// The length and whether anything below it was cut by the path or the depth, which keeps it out of the memo (decision 159).
    fn length_in(&mut self, id: &str, path: &mut Vec<String>) -> (f64, bool) {
        if let Some(hit) = self.lengths.get(id) {
            return (*hit, false);
        }
        let mats = self.mats;
        let Some(t) = mats.timelines.get(id) else {
            return (0.0, false);
        };
        // A cycle has no length; the reference check (2e) is what reports it.
        if path.iter().any(|s| s == id) {
            return (0.0, true);
        }
        let file = mats.timeline_file(id);
        self.last_file = Arc::clone(&file);
        // The expansion names a nest cut by the depth; a length cut by it is only left out of the memo.
        if path.len() >= MAX_NEST {
            return (0.0, true);
        }
        if !self.charge(1, &file) {
            return (0.0, true);
        }
        let g = self.grid;
        let mut cut = false;
        let len = if let Some(stated) = &t.length {
            js_max(0.0, g.duration(&stated.0))
        } else {
            let mut end = 0.0;
            path.push(id.to_owned());
            for lane in &t.lanes {
                // Every lane, event and picture of the walk is paid for, a memo hit and a refusal included (decision 157).
                if !self.charge(1, &file) {
                    path.pop();
                    return (0.0, true);
                }
                for p in lane.events.iter().flatten() {
                    if !self.charge(1, &file) {
                        path.pop();
                        return (0.0, true);
                    }
                    let r = ref_of(p);
                    let extent = if mats.timelines.contains_key(r) {
                        let (l, c) = self.length_in(r, path);
                        cut |= c;
                        l
                    } else {
                        self.placement_to_clip(p, &file).map_or(0.0, |c| c.length)
                    };
                    end = js_max(end, g.position(p.start.as_ref()) + extent);
                }
            }
            path.pop();
            // A shot with no length counts as nothing past its start, as an audio clip with no `to` does.
            let beats_per_sec = g.tempo / 60.0;
            for lane in t.pictures.iter().flatten() {
                for s in lane.events.iter().flatten() {
                    if !self.charge(1, &file) {
                        return (0.0, true);
                    }
                    let len = s.length.map_or(0.0, |l| l.0);
                    end = js_max(end, g.position(s.start.as_ref()) + len * beats_per_sec);
                }
            }
            js_max(1.0, (end / g.bar() - 1e-9).ceil()) * g.bar()
        };
        if !cut {
            self.lengths.insert(id.to_owned(), len);
        }
        (len, cut)
    }

    /// `events` with every timeline event replaced by the events it holds, recursively; `file` is the lane's.
    pub fn expand_events<'e>(&mut self, events: &'e [Placement], file: &str) -> Vec<Leaf<'e>>
    where
        'a: 'e,
    {
        let mut out = Vec::new();
        self.expand_into(events, &Arc::from(file), None, &mut Vec::new(), &mut out);
        out
    }

    fn expand_into<'e>(
        &mut self,
        events: &'e [Placement],
        file: &Arc<str>,
        within: Option<&Arc<Within>>,
        path: &mut Vec<String>,
        out: &mut Vec<Leaf<'e>>,
    ) where
        'a: 'e,
    {
        let mats: &'a super::Materials = self.mats;
        self.last_file = Arc::clone(file);
        for p in events {
            if !self.charge(1, file) {
                return;
            }
            let r = ref_of(p);
            let Some(t) = mats.timelines.get(r) else {
                out.push(Leaf {
                    p,
                    within: within.cloned(),
                    file: Arc::clone(file),
                });
                continue;
            };
            // A cycle is cut silently; the reference check (2e) reports it.
            if path.iter().any(|s| s == r) {
                continue;
            }
            if path.len() >= MAX_NEST {
                self.sink.error(&mats.timeline_file(r), nest_message());
                continue;
            }
            let g = self.grid;
            let at = g.position(p.start.as_ref());
            let start = within.map_or(0.0, |w| w.offset) + at;
            let length = self.timeline_length(r);
            let gain = p.gain.map_or(0.0, |g| g.0);
            let gain = if gain.is_nan() { 0.0 } else { gain };
            let event = EventDoc {
                gain: (gain != 0.0).then_some(gain),
                disabled: (p.disabled == Some(true)).then_some(true),
                ..EventDoc::default()
            };
            let prefix = format!(
                "{}{}{NEST_SEP}",
                within.map_or("", |w| w.prefix.as_str()),
                placement_id(r, at)
            );
            let inner = Within {
                offset: start,
                end: js_min(start + length, within.map_or(f64::INFINITY, |w| w.end)),
                gain: within.map_or(0.0, |w| w.gain) + gain,
                disabled: within.is_some_and(|w| w.disabled) || p.disabled == Some(true),
                prefix,
                nest: within.map_or_else(
                    || NestDoc {
                        of: placement_id(r, start),
                        ref_: r.to_owned(),
                        start,
                        length,
                        event: (!event.is_empty()).then_some(event),
                    },
                    |w| w.nest.clone(),
                ),
            };
            let inner = Arc::new(inner);
            let tfile = mats.timeline_file(r);
            path.push(r.to_owned());
            for lane in &t.lanes {
                if !self.charge(1, &tfile) {
                    path.pop();
                    return;
                }
                let w = if lane.disabled == Some(true) && !inner.disabled {
                    Arc::new(Within {
                        disabled: true,
                        ..(*inner).clone()
                    })
                } else {
                    Arc::clone(&inner)
                };
                self.expand_into(lane.events.as_deref().unwrap_or(&[]), &tfile, Some(&w), path, out);
            }
            path.pop();
        }
    }

    /// Spends `n`, and past the budget lets the one error name `file`.
    fn charge(&mut self, n: usize, file: &str) -> bool {
        let ok = self.budget.charge(n);
        if !ok {
            self.sink.limit(&mut self.budget, file);
        }
        ok
    }

    /// Every timeline of the project compiled for the view that edits it: its lanes, its events at its own positions.
    pub fn timeline_docs(&mut self) -> Vec<TimelineDoc> {
        let mats = self.mats;
        let mut out = Vec::new();
        for (id, t) in &mats.timelines {
            let file = mats.timeline_file(id);
            let length = self.timeline_length(id);
            if !length.is_finite() {
                self.sink.push(error_in(
                    &file,
                    "this timeline's length is not a finite number of beats, so it has no view",
                ));
                continue;
            }
            let mut lanes = Vec::new();
            for lane in &t.lanes {
                let events = lane.events.as_deref().unwrap_or(&[]);
                let mut leaves = Vec::new();
                self.expand_into(events, &file, None, &mut vec![id.clone()], &mut leaves);
                let clips = leaves.iter().flat_map(|l| self.place(l)).collect();
                lanes.push(TimelineLaneDoc {
                    id: timeline_lane_id(id, &lane.lane.0),
                    name: lane.lane.0.clone(),
                    color: lane.color.clone().unwrap_or_default(),
                    disabled: lane.disabled == Some(true),
                    clips,
                });
            }
            out.push(TimelineDoc {
                id: id.clone(),
                length,
                lanes,
            });
        }
        out
    }
}

pub(crate) fn nest_message() -> String {
    format!("timelines are placed inside timelines more than {MAX_NEST} deep here — the rest is left out")
}
