// picture_read_cut.rs — a cut read from another editor, before it becomes this project's files: an `.otio` or an `.edl` as tracks of shots.
// A reader would get wrong a dissolve: it lands as THIS app's, the incoming shot's `fadeIn` at its own start, the outgoing one running on under it.

use serde_json::Value as Json;

/// One shot as it lands on a lane: where, how long, from where in its source, and how it arrives.
#[derive(Clone, Debug, PartialEq)]
pub struct Imported {
    pub name: String,
    /// The media as the file names it: a URL, a path, or (an EDL) only a clip name.
    pub media: String,
    pub start: f64,
    pub length: f64,
    pub source_from: f64,
    pub speed: Option<f64>,
    pub fade_in: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ImportedTrack {
    pub name: String,
    pub shots: Vec<Imported>,
}

/// The cut, bottom track first, and what the reader could not carry.
#[derive(Clone, Debug, PartialEq)]
pub struct Cut {
    pub fps: f64,
    pub tracks: Vec<ImportedTrack>,
    pub skipped: Vec<String>,
}

fn schema_of(o: &Json) -> String {
    o.get("OTIO_SCHEMA")
        .and_then(Json::as_str)
        .map_or(String::new(), |s| s.split('.').next().unwrap_or("").to_owned())
}

fn text(o: &Json, k: &str) -> Option<String> {
    o.get(k).and_then(Json::as_str).map(str::to_owned)
}

fn sec(t: Option<&Json>) -> f64 {
    let rate = t.and_then(|r| r.get("rate")).and_then(Json::as_f64).unwrap_or(0.0);
    if rate == 0.0 {
        return 0.0;
    }
    t.and_then(|r| r.get("value")).and_then(Json::as_f64).unwrap_or(0.0) / rate
}

fn media_of(clip: &Json) -> String {
    let reference = match clip.get("media_references") {
        Some(refs) => {
            let key = text(clip, "active_media_reference_key").unwrap_or_else(|| "DEFAULT_MEDIA".to_owned());
            refs.get(&key)
        }
        None => clip.get("media_reference"),
    };
    reference.and_then(|r| text(r, "target_url")).unwrap_or_default()
}

fn speed_of(clip: &Json) -> Option<f64> {
    for e in clip.get("effects").and_then(Json::as_array).into_iter().flatten() {
        match schema_of(e).as_str() {
            "FreezeFrame" => return Some(0.0),
            "LinearTimeWarp" => return Some(e.get("time_scalar").and_then(Json::as_f64).unwrap_or(1.0)),
            _ => {}
        }
    }
    None
}

/// Turns a dissolve centred on a cut into this app's: B starts `in_offset` earlier and fades over both halves, A runs `out_offset` longer.
fn dissolve_into_fade_in(a: Option<&mut Imported>, b: &mut Imported, in_offset: f64, out_offset: f64) {
    let speed = b.speed.unwrap_or(1.0);
    b.start -= in_offset;
    b.length += in_offset;
    b.source_from -= in_offset * speed;
    b.fade_in = in_offset + out_offset;
    if let Some(a) = a {
        a.length += out_offset;
    }
}

fn read_track(track: &Json, skipped: &mut Vec<String>) -> ImportedTrack {
    let name = text(track, "name").unwrap_or_default();
    let mut shots: Vec<Imported> = Vec::new();
    let mut at = 0.0;
    let mut pending: Option<(f64, f64)> = None;
    for item in track.get("children").and_then(Json::as_array).into_iter().flatten() {
        let kind = schema_of(item);
        let range = item.get("source_range");
        let duration = sec(range.and_then(|r| r.get("duration")));
        match kind.as_str() {
            "Transition" => {
                pending = Some((sec(item.get("in_offset")), sec(item.get("out_offset"))));
                continue;
            }
            "Gap" => {
                at += duration;
                pending = None;
                continue;
            }
            "Clip" => {}
            _ => {
                let what = if kind.is_empty() { "item" } else { &kind };
                skipped.push(format!("{name}: a nested {what} is not carried — flatten it first"));
                at += duration;
                continue;
            }
        }
        let mut shot = Imported {
            name: text(item, "name")
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| format!("shot-{}", shots.len() + 1)),
            media: media_of(item),
            start: at,
            length: duration,
            source_from: sec(range.and_then(|r| r.get("start_time"))),
            speed: speed_of(item),
            fade_in: 0.0,
        };
        if let Some((i, o)) = pending.take() {
            dissolve_into_fade_in(shots.last_mut(), &mut shot, i, o);
        }
        shots.push(shot);
        at += duration;
    }
    ImportedTrack { name, shots }
}

/// Reads an `.otio` file's text into a cut.
///
/// # Errors
/// A text that is no OpenTimelineIO timeline at all.
pub fn from_otio(source: &str) -> Result<Cut, String> {
    let t: Json = serde_json::from_str(source).map_err(|e| e.to_string())?;
    if schema_of(&t) != "Timeline" {
        return Err("not an OpenTimelineIO timeline".to_owned());
    }
    let mut skipped = Vec::new();
    let mut tracks = Vec::new();
    let mut fps = 0.0;
    for tr in t
        .get("tracks")
        .and_then(|s| s.get("children"))
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
    {
        let kind = text(tr, "kind").unwrap_or_else(|| "Video".to_owned());
        if kind != "Video" {
            let name = text(tr, "name").unwrap_or_else(|| "a track".to_owned());
            skipped.push(format!(
                "{name}: an {} track — a shot's sound comes from its own lane here",
                kind.to_lowercase()
            ));
            continue;
        }
        let first = tr.get("children").and_then(Json::as_array).and_then(|c| {
            c.iter().find(|c| {
                c.get("source_range")
                    .and_then(|r| r.get("duration"))
                    .is_some_and(|d| !d.is_null())
            })
        });
        let rate = first
            .and_then(|c| c.get("source_range"))
            .and_then(|r| r.get("duration"))
            .and_then(|d| d.get("rate"))
            .and_then(Json::as_f64)
            .unwrap_or(0.0);
        if fps == 0.0 {
            fps = rate;
        }
        tracks.push(read_track(tr, &mut skipped));
    }
    Ok(Cut {
        fps: if fps == 0.0 { 25.0 } else { fps },
        tracks,
        skipped,
    })
}

fn digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

/// `HH:MM:SS:FF` (or `;` or `.` before the frames) as seconds at `fps`.
fn tc(s: &str, fps: f64) -> Option<f64> {
    let b = s.as_bytes();
    if b.len() != 11 || !matches!(b[8], b':' | b';' | b'.') || b[2] != b':' || b[5] != b':' {
        return None;
    }
    let part = |a: usize| {
        s.get(a..a + 2)
            .filter(|p| digits(p))
            .and_then(|p| p.parse::<f64>().ok())
    };
    Some(part(0)? * 3600.0 + part(3)? * 60.0 + part(6)? + part(9)? / fps)
}

/// The `(kind, frames)` of an event's kind word: `C`, `D030`, `W001`.
fn kind_word(w: &str) -> Option<(&str, Option<&str>)> {
    if let Some(rest) = w.strip_prefix('W') {
        return digits(rest).then_some((w, None));
    }
    let (k, rest) = w.split_at_checked(1)?;
    (matches!(k, "C" | "D") && (rest.is_empty() || digits(rest))).then_some((k, (!rest.is_empty()).then_some(rest)))
}

struct Event<'l> {
    index: &'l str,
    track: &'l str,
    kind: &'l str,
    frames: Option<&'l str>,
    times: [&'l str; 4],
}

fn event_of(line: &str) -> Option<Event<'_>> {
    let w: Vec<&str> = line.split_whitespace().collect();
    if w.len() < 8 || w[0].len() < 3 || !digits(w[0]) {
        return None;
    }
    let (kind, glued) = kind_word(w[3])?;
    let rest = &w[4..];
    let (frames, times) = match rest.len() {
        4 => (glued, rest),
        5 if digits(rest[0]) && glued.is_none() => (Some(rest[0]), &rest[1..]),
        _ => return None,
    };
    Some(Event {
        index: w[0],
        track: w[2],
        kind,
        frames,
        times: [times[0], times[1], times[2], times[3]],
    })
}

/// Reads an EDL's text at `fps` (an EDL does not state its rate) into a cut of one track.
pub fn from_edl(source: &str, fps: f64) -> Cut {
    let mut shots: Vec<Imported> = Vec::new();
    let mut skipped = Vec::new();
    let mut record_zero: Option<f64> = None;
    let mut held = false;
    for raw in source.lines() {
        let line = raw.trim();
        let name = line.strip_prefix('*').map(str::trim_start).and_then(|l| {
            l.strip_prefix("FROM CLIP NAME:")
                .map(|r| (false, r))
                .or_else(|| l.strip_prefix("TO CLIP NAME:").map(|r| (true, r)))
        });
        if let (Some((to, rest)), Some(last)) = (name, shots.last_mut()) {
            let n = rest.trim();
            if !n.is_empty() && (to || last.media.is_empty()) {
                n.clone_into(&mut last.media);
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("M2")
            && let Some(last) = shots.last_mut()
        {
            let mut w = rest.split_whitespace();
            if let (Some(_), Some(rate), Some(_)) = (w.next(), w.next(), w.next())
                && let Ok(r) = rate.parse::<f64>()
            {
                last.speed = Some(r / fps);
                continue;
            }
        }
        let Some(e) = event_of(line) else {
            continue;
        };
        if !e.track.starts_with('V') {
            skipped.push(format!(
                "event {}: track {} — only the picture is read",
                e.index, e.track
            ));
            continue;
        }
        let [s_in, s_out, r_in, r_out] = e.times.map(|t| tc(t, fps));
        let (Some(s_in), Some(_), Some(r_in), Some(r_out)) = (s_in, s_out, r_in, r_out) else {
            skipped.push(format!("event {}: a timecode this cannot read", e.index));
            continue;
        };
        let zero = *record_zero.get_or_insert(r_in);
        if e.kind == "C" && r_out == r_in {
            held = true;
            continue;
        }
        let mut shot = Imported {
            name: format!("shot-{}", shots.len() + 1),
            media: String::new(),
            start: r_in - zero,
            length: r_out - r_in,
            source_from: s_in,
            speed: None,
            fade_in: 0.0,
        };
        if e.kind == "D" && held {
            shot.fade_in = e.frames.and_then(|f| f.parse::<f64>().ok()).unwrap_or(0.0) / fps;
            if let Some(last) = shots.last_mut() {
                last.length += shot.fade_in;
            }
        } else if e.kind.starts_with('W') {
            skipped.push(format!("event {}: a wipe — read as a cut", e.index));
        }
        held = false;
        shots.push(shot);
    }
    for (i, s) in shots.iter_mut().enumerate() {
        let stem = match s.media.rfind('.') {
            Some(at) if at + 1 < s.media.len() => s.media[..at].to_owned(),
            _ => s.media.clone(),
        };
        s.name = if stem.is_empty() {
            format!("shot-{}", i + 1)
        } else {
            stem
        };
    }
    Cut {
        fps,
        tracks: vec![ImportedTrack {
            name: "edl".to_owned(),
            shots,
        }],
        skipped,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_edl_dissolve_pair_becomes_the_incoming_shots_fade() {
        let edl = "TITLE: t\nFCM: NON-DROP FRAME\n\n\
            001  AX       V     C        00:00:00:00 00:00:04:00 01:00:00:00 01:00:04:00\n* FROM CLIP NAME: a.mp4\n\n\
            002  AX       V     C        00:00:04:00 00:00:04:00 01:00:04:00 01:00:04:00\n\
            002  AX       V     D    015 00:00:00:00 00:00:03:00 01:00:04:00 01:00:07:00\n* TO CLIP NAME: b.mp4\n\n";
        let cut = from_edl(edl, 30.0);
        let s = &cut.tracks[0].shots;
        assert_eq!(
            (s.len(), s[0].media.as_str(), s[1].media.as_str()),
            (2, "a.mp4", "b.mp4")
        );
        assert_eq!((s[1].start, s[1].fade_in, s[0].length), (4.0, 0.5, 4.5));
        assert_eq!(s[1].name, "b");
    }

    #[test]
    fn an_otio_track_lays_clips_end_to_end_with_gaps() {
        let otio = r#"{"OTIO_SCHEMA":"Timeline.1","tracks":{"children":[{"OTIO_SCHEMA":"Track.1","kind":"Video","name":"V1","children":[
            {"OTIO_SCHEMA":"Gap.1","source_range":{"duration":{"rate":25,"value":50}}},
            {"OTIO_SCHEMA":"Clip.1","name":"c","source_range":{"start_time":{"rate":25,"value":25},"duration":{"rate":25,"value":100}},
             "media_reference":{"target_url":"a.mp4"},"effects":[{"OTIO_SCHEMA":"LinearTimeWarp.1","time_scalar":2}]}]}]}}"#;
        let cut = from_otio(otio).unwrap_or_else(|e| panic!("{e}"));
        let s = &cut.tracks[0].shots[0];
        assert_eq!(
            (s.start, s.length, s.source_from, s.speed, cut.fps),
            (2.0, 4.0, 1.0, Some(2.0), 25.0)
        );
        assert!(from_otio("{}").is_err());
    }
}
