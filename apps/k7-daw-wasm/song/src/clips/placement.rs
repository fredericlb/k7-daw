// clips/placement.rs — one placement read into the ClipDoc the engine plays: its window, loops, fades and the words the file said.
// A reader would get wrong the units: `start` and `loop` are 1-based positions, the rest 0-based durations; after compile/clips.ts.

use super::doc::{ClipDoc, ClipKind, EventDoc, NoteDoc};
use super::id::{js_max, js_min, js_number, placement_id};
use super::placer::{Materials, Sink};
use super::warp::read_clip_warp;
use super::{Budget, Grid};
use crate::files::{Clip, Fade, Placement, Sequence, WindowEdge};

/// The keys only a shot reads, which an event that sounds is refused for carrying.
const SHOT_KEYS: [&str; 6] = ["length", "animate", "speed", "effect", "silent", "pitch"];

/// What a placement is read with: the grid, the materials, and where findings and spending go.
pub(crate) struct Cx<'c> {
    pub grid: &'c Grid,
    pub mats: &'c Materials,
    pub sink: &'c mut Sink,
    pub budget: &'c mut Budget,
}

impl Cx<'_> {
    /// Spends `n` of the budget; past it, one error names `file` and the answer is false.
    pub fn spend(&mut self, n: usize, file: &str) -> bool {
        let ok = self.budget.charge(n);
        if !ok {
            self.sink.limit(self.budget, file);
        }
        ok
    }
}

/// An event's start as a message prints it: `4.5s` for seconds, the timecode as written otherwise.
fn start_text(p: &Placement) -> String {
    match &p.start {
        Some(WindowEdge::Seconds(s)) => format!("{}s", js_number(*s)),
        Some(WindowEdge::Timecode(t)) => t.clone(),
        None => "1:1:1".to_owned(),
    }
}

/// The refusal of an event that sounds yet carries what only a shot reads, or has no ref.
fn refuse_shot_fields(p: &Placement, ref_: &str) -> Option<String> {
    if ref_.is_empty() {
        return Some(format!(
            "an event at {} has no ref — only a lane of shapes takes one, to say when it is on",
            start_text(p)
        ));
    }
    let present = [
        p.length.is_some(),
        p.animate.is_some(),
        p.speed.is_some(),
        p.effect.is_some(),
        p.silent.is_some(),
        p.pitch.is_some(),
    ];
    let mut keys: Vec<&str> = SHOT_KEYS
        .iter()
        .zip(present)
        .filter_map(|(k, here)| here.then_some(*k))
        .collect();
    if matches!(p.fade_in, Some(Fade::Map(_))) {
        keys.push("fadeIn");
    }
    if matches!(p.fade_out, Some(Fade::Map(_))) {
        keys.push("fadeOut");
    }
    if keys.is_empty() {
        return None;
    }
    Some(format!(
        "'{ref_}' at {}: {} {} a shot's — a fade on sound is a duration like \"0:1\"",
        start_text(p),
        keys.join(", "),
        if keys.len() > 1 { "are" } else { "is" }
    ))
}

/// The placement's own words, unmerged, holding only what the file said.
fn event_of(p: &Placement, loops: u64, gain: f64) -> EventDoc {
    let win = |e: &Option<WindowEdge>| match e {
        Some(WindowEdge::Timecode(t)) if t.is_empty() => None,
        other => other.clone(),
    };
    let text = |f: &Option<Fade>| match f {
        Some(Fade::Timecode(t)) if !t.0.is_empty() => Some(t.0.clone()),
        _ => None,
    };
    let list = |n: usize| n > 0;
    EventDoc {
        from: win(&p.from),
        to: win(&p.to),
        fade_in: text(&p.fade_in),
        fade_out: text(&p.fade_out),
        loop_: (loops > 1).then_some(loops),
        gain: (gain != 0.0).then_some(gain),
        disabled: (p.disabled == Some(true)).then_some(true),
        warp: (p.warp == Some(false)).then_some(false),
        takes: p.takes.clone().filter(|v| list(v.len())),
        comp: p.comp.clone().filter(|v| list(v.len())),
        crossfades: p.crossfades.clone().filter(|v| list(v.len())),
    }
}

impl Cx<'_> {
    /// A window edge always as seconds; a timecode edge is a position, and an edge at the first beat is no edge.
    fn edge(&self, v: Option<&WindowEdge>) -> Option<f64> {
        match v? {
            WindowEdge::Seconds(s) => (s.is_finite() && *s > 0.0).then_some(*s),
            WindowEdge::Timecode(t) if t.is_empty() => None,
            WindowEdge::Timecode(t) => {
                let beats = self.grid.position_text(t);
                (beats > 0.0).then(|| self.grid.seconds(beats))
            }
        }
    }

    fn dur(&self, f: &Option<Fade>) -> f64 {
        match f {
            Some(Fade::Timecode(t)) if !t.0.is_empty() => self.grid.duration(&t.0),
            _ => 0.0,
        }
    }

    /// One placement as a ClipDoc; None when the event is refused, holds a number that is not finite, or the budget ran out.
    pub fn clip_of(&mut self, p: &Placement, file: &str) -> Option<ClipDoc> {
        let ref_ = p.ref_.as_ref().map_or("", |r| r.0.as_str());
        if let Some(message) = refuse_shot_fields(p, ref_) {
            self.sink.error(file, message);
            return None;
        }
        let mats = self.mats;
        let clip = mats.clips.get(ref_);
        let seq = if clip.is_some() { None } else { mats.sequences.get(ref_) };
        let words = p.takes.as_ref().map_or(0, Vec::len)
            + p.comp.as_ref().map_or(0, Vec::len)
            + p.crossfades.as_ref().map_or(0, Vec::len);
        let material = match (clip.map(|c| &**c), seq) {
            (Some(Clip::Audio(a)), _) => a.warp.as_ref().map_or(0, |w| w.markers.len()),
            (_, Some(s)) => s.notes.0.len(),
            _ => 0,
        };
        if !self.spend(1 + words + material, file) {
            return None;
        }
        let g = self.grid;
        let start = g.position(p.start.as_ref());
        let loops = p.loop_.map_or(1, |l| l.0.max(1));
        let loops_f = loops as f64;
        let gain = js_max(-6.0, js_min(6.0, p.gain.map_or(0.0, |g| g.0)));
        let event = event_of(p, loops, gain);
        let base = ClipDoc {
            id: placement_id(ref_, start),
            kind: ClipKind::Audio,
            name: ref_.to_owned(),
            source: String::new(),
            start,
            length: 0.0,
            offset: 0.0,
            source_from: None,
            source_to: None,
            fade_in: self.dur(&p.fade_in),
            fade_out: self.dur(&p.fade_out),
            gain_db: None,
            loop_start: 0.0,
            loop_end: 0.0,
            notes: Vec::new(),
            warp: None,
            warp_off: None,
            disabled: (p.disabled == Some(true)).then_some(true),
            event: (!event.is_empty()).then_some(event),
            comp: None,
            crossfade: None,
            nest: None,
        };
        let doc = match (clip, seq) {
            (Some(c), _) => self.audio(p, base, c, loops_f, gain, ref_),
            (None, Some(s)) => self.sequence(p, base, s, loops_f),
            // A ref nothing holds is an empty audio clip; the reference check names it (2e).
            (None, None) => base,
        };
        if doc.finite() {
            return Some(doc);
        }
        let message = format!(
            "'{ref_}' at {}: a number in this event is not finite, so it plays nothing",
            start_text(p)
        );
        self.sink.error(file, message);
        None
    }

    fn audio(&self, p: &Placement, base: ClipDoc, cm: &Clip, loops: f64, gain: f64, ref_: &str) -> ClipDoc {
        let g = self.grid;
        let (c_from, c_to, raw) = match cm {
            Clip::Audio(a) => (a.from.as_ref(), a.to.as_ref(), a.warp.as_ref()),
            // A picture or caption placed on a sound lane compiles as an audio clip of its source (decision 163).
            _ => (None, None, None),
        };
        let (map, declined) = read_clip_warp(raw);
        let off = p.warp == Some(false) || declined;
        let warp = if off { None } else { map.clone() };
        let (c_from, c_to) = (self.edge(c_from), self.edge(c_to));
        let (p_from, p_to) = (self.edge(p.from.as_ref()), self.edge(p.to.as_ref()));
        let from = (c_from.is_some() || p_from.is_some()).then(|| js_max(c_from.unwrap_or(0.0), p_from.unwrap_or(0.0)));
        let to = match (c_to, p_to) {
            (Some(a), Some(b)) => Some(js_min(a, b)),
            (a, b) => a.or(b),
        };
        let win_beats = match to {
            Some(t) if t > from.unwrap_or(0.0) => g.beats(t - from.unwrap_or(0.0)),
            _ => 0.0,
        };
        let mut out = base;
        out.source = self.mats.paths.get(ref_).cloned().unwrap_or_default();
        if win_beats > 0.0 {
            out.length = win_beats * loops;
        }
        if gain != 0.0 {
            out.gain_db = Some(gain);
        }
        if loops > 1.0 && win_beats > 0.0 {
            out.loop_start = g.beats(from.unwrap_or(0.0));
            out.loop_end = g.beats(to.unwrap_or(0.0));
        }
        out.source_from = from;
        out.source_to = to;
        out.warp = warp;
        out.warp_off = (off && map.is_some()).then_some(true);
        out
    }

    fn sequence(&self, p: &Placement, base: ClipDoc, seq: &Sequence, loops: f64) -> ClipDoc {
        let g = self.grid;
        let notes: Vec<NoteDoc> = seq
            .notes
            .0
            .iter()
            .map(|n| NoteDoc {
                time: n.t.0,
                duration: n.d.0,
                key: n.k.0,
                vel: n.v.map_or(1.0, |v| v.0),
                channel: 0,
                rel: 1.0,
                repeat: n.repeat.map(|r| r.0),
                skip: n.skip.map(|s| s.0),
            })
            .collect();
        // The window in beats defaults to the notes' extent rounded UP TO THE BAR.
        let bar = g.bar();
        let end = notes.iter().fold(0.0, |m, n| js_max(m, n.time + n.duration));
        let extent = (end / bar).ceil() * bar;
        let text = |e: &Option<crate::files::scalar::SeqEdge>| e.as_ref().map(|t| g.position_text(&t.0));
        let m_from = text(&seq.from).unwrap_or(0.0);
        let m_to_raw = text(&seq.to).unwrap_or(extent);
        let m_to = if m_to_raw > m_from {
            m_to_raw
        } else if extent > m_from {
            extent
        } else {
            m_from
        };
        // The event's window is intersected as an audio placement's is, with no fallback.
        let e_from = p.from.as_ref().map(|e| g.position(Some(e)));
        let e_to = p.to.as_ref().map(|e| g.position(Some(e)));
        let w_from = js_max(m_from, e_from.unwrap_or(0.0));
        let w_to = js_max(w_from, e_to.map_or(m_to, |t| js_min(m_to, t)));
        let win = w_to - w_from;
        let stated_to = (seq.to.is_some() || e_to.is_some()) && (w_to - extent).abs() > 1e-9;
        let mut out = base;
        out.kind = ClipKind::Midi;
        out.notes = notes;
        out.length = win * loops;
        if win > 0.0 && (loops > 1.0 || w_from > 0.0 || stated_to) {
            out.loop_start = w_from;
            out.loop_end = w_to;
        }
        out
    }
}
