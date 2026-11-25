// shared.rs — what several verb groups need: the words of a line, the lane an argument names, the event at a timecode.
// A reader would get wrong `track_of`'s copy: a timeline's lane is made into a lane the way `timelineLaneTrack` makes it.

use crate::host::{Host, Thrown};
use song::clips::{ClipDoc, ClipKind, NestDoc};
use song::doc::{ProjectDoc, TrackDoc};
use song_edit::chain_item_edit::timeline_lane_of;
use song_edit::js_text::is_js_space;
use song_schema::parse_position_on;

/// The result of every verb: a refusal is an `Ok`, only a throw that escapes the verb is an `Err`.
pub type Done = Result<(), Thrown>;

/// A line's words, as `splitWords` splits them.
pub fn split_words(a: &str) -> Vec<&str> {
    a.split(is_js_space).filter(|w| !w.is_empty()).collect()
}

/// The meter as numbers, the way the TypeScript passes `timeSigNumerator`.
pub fn meter(p: &ProjectDoc) -> (f64, f64) {
    (f64::from(p.time_sig_numerator), f64::from(p.time_sig_denominator))
}

/// `parsePosition` in the song's meter, its message being the timecode's own.
pub fn position(tc: &str, p: &ProjectDoc) -> Result<f64, String> {
    let (n, d) = meter(p);
    parse_position_on(tc, n, d).map_err(|e| e.to_string())
}

/// `no track "x" — try: a, b`, the hint only when the song has lanes.
pub fn no_track(p: &ProjectDoc, arg: &str) -> String {
    if p.tracks.is_empty() {
        format!("no track \"{arg}\"")
    } else {
        let ids: Vec<&str> = p.tracks.iter().map(|t| t.id.as_str()).collect();
        format!("no track \"{arg}\" — try: {}", ids.join(", "))
    }
}

fn timeline_lane_track(id: &str, name: &str, color: &str, disabled: bool, clips: &[ClipDoc]) -> TrackDoc {
    let mut t = TrackDoc::new(id);
    t.name = name.to_owned();
    t.content_type = "audio notes".to_owned();
    t.color = color.to_owned();
    t.disabled = disabled;
    t.monitor = "off".to_owned();
    t.clips = clips.to_vec();
    t
}

/// A lane by id or name, or a timeline's lane addressed `<timeline>><lane>`.
pub fn track_of(p: &ProjectDoc, arg: &str) -> Option<TrackDoc> {
    let own = p.tracks.iter().find(|t| t.id == arg || t.name == arg);
    if own.is_some() || timeline_lane_of(arg).is_none() {
        return own.cloned();
    }
    p.timelines
        .iter()
        .flatten()
        .flat_map(|t| t.lanes.iter())
        .find(|l| l.id == arg)
        .map(|l| timeline_lane_track(&l.id, &l.name, &l.color, l.disabled, &l.clips))
}

fn nest_hit(n: &NestDoc) -> ClipDoc {
    ClipDoc {
        id: n.of.clone(),
        kind: ClipKind::Audio,
        name: n.ref_.clone(),
        source: String::new(),
        start: n.start,
        length: n.length,
        offset: 0.0,
        source_from: None,
        source_to: None,
        fade_in: 0.0,
        fade_out: 0.0,
        gain_db: None,
        loop_start: 0.0,
        loop_end: 0.0,
        notes: Vec::new(),
        warp: None,
        warp_off: None,
        disabled: n.event.as_ref().and_then(|e| e.disabled).filter(|d| *d),
        event: n.event.clone(),
        comp: None,
        crossfade: None,
        nest: Some(n.clone()),
    }
}

/// The event starting at `beats`: a timeline event by its own start, never one of the clips it expands into.
pub fn event_at(track: &TrackDoc, beats: f64) -> Option<ClipDoc> {
    let nest = track
        .clips
        .iter()
        .find_map(|c| c.nest.as_ref().filter(|n| (n.start - beats).abs() < 1e-6));
    match nest {
        Some(n) => Some(nest_hit(n)),
        None => track
            .clips
            .iter()
            .find(|c| c.nest.is_none() && (c.start - beats).abs() < 1e-6)
            .cloned(),
    }
}

/// What a log line calls an event: its name, else its id.
pub fn label(c: &ClipDoc) -> &str {
    if c.name.is_empty() { &c.id } else { &c.name }
}

/// Would `ref_` on lane `lane_id` put a timeline inside itself?
pub fn makes_cycle(p: &ProjectDoc, lane_id: &str, ref_: &str) -> bool {
    let Some((owner, _)) = timeline_lane_of(lane_id) else {
        return false;
    };
    fn reaches(p: &ProjectDoc, owner: &str, id: &str, seen: &[String]) -> bool {
        if id == owner {
            return true;
        }
        if seen.iter().any(|s| s == id) {
            return false;
        }
        let mut next = seen.to_vec();
        next.push(id.to_owned());
        p.timelines.iter().flatten().find(|t| t.id == id).is_some_and(|t| {
            t.lanes.iter().any(|l| {
                l.clips
                    .iter()
                    .any(|c| c.nest.as_ref().is_some_and(|n| reaches(p, owner, &n.ref_, &next)))
            })
        })
    }
    reaches(p, owner, ref_, &[])
}

/// `base` when bare and free, else the first `base-N` from `from` that is not taken.
pub fn free_name(base: &str, taken: &dyn Fn(&str) -> bool, bare: bool, from: u32) -> String {
    if bare && !taken(base) {
        return base.to_owned();
    }
    let mut n = from;
    while taken(&format!("{base}-{n}")) {
        n += 1;
    }
    format!("{base}-{n}")
}

/// A lane id not yet used: `base-1`, `base-2`, …
pub fn unique_id(base: &str, p: &ProjectDoc) -> String {
    free_name(base, &|n| p.tracks.iter().any(|t| t.id == n), false, 1)
}

/// The lane and event a `<track> <timecode>` pair names, or None with the refusal said.
pub struct Found {
    pub track: TrackDoc,
    pub at_beats: f64,
    pub hit: ClipDoc,
    pub num: f64,
    pub den: f64,
    pub tempo: f64,
}

/// `resolveEvent`: the track, the beat and the event at `at_tc`, refusing a timeline's or, when asked, a comped one.
pub fn resolve_event(
    h: &mut Host,
    track_arg: &str,
    at_tc: &str,
    refuse_comp: bool,
    timeline: bool,
) -> Result<Option<Found>, Thrown> {
    let p = h.project()?;
    let Some(track) = track_of(&p, track_arg) else {
        h.refuse([no_track(&p, track_arg)]);
        return Ok(None);
    };
    let (num, den) = meter(&p);
    let at_beats = match position(at_tc, &p) {
        Ok(b) => b,
        Err(m) => {
            h.refuse([format!("bad timecode: {m}")]);
            return Ok(None);
        }
    };
    let hit = event_at(&track, at_beats);
    if let Some(nest) = hit.as_ref().and_then(|c| c.nest.as_ref())
        && !timeline
    {
        h.refuse([format!(
            "the event at {at_tc} is timeline '{}', whose length is fixed — edit the timeline itself",
            nest.ref_
        )]);
        return Ok(None);
    }
    let Some(hit) = hit else {
        h.refuse([format!("no event on '{}' at {at_tc}", track.id)]);
        return Ok(None);
    };
    if refuse_comp && hit.comp.is_some() {
        h.refuse([format!(
            "the event at {at_tc} is comped from its takes — edit it in its takes view"
        )]);
        return Ok(None);
    }
    Ok(Some(Found {
        track,
        at_beats,
        hit,
        num,
        den,
        tempo: p.tempo,
    }))
}

/// `<project>/<rel>` in the `Fs`'s spelling.
pub fn join(dir: &str, rel: &str) -> String {
    song_edit::paths::join(&[dir, rel])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_split_on_any_space() {
        assert_eq!(split_words("  a \t b\u{a0}c "), vec!["a", "b", "c"]);
    }

    #[test]
    fn a_free_name_counts_from_the_first_number() {
        let taken = |n: &str| n == "track-1" || n == "x";
        assert_eq!(free_name("track", &taken, false, 1), "track-2");
        assert_eq!(free_name("x", &taken, true, 2), "x-2");
        assert_eq!(free_name("y", &taken, true, 2), "y");
    }
}
