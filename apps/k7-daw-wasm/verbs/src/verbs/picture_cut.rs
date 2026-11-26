// picture_cut.rs — the picture as another editor reads it: an OpenTimelineIO timeline or a CMX 3600 EDL, from sequential segments.
// A reader would get wrong overlap: the shot later in the file covers the ones before it, so a lane flattens to segments laid end to end.

use super::picture_doc::{PicClip, PicDoc, PicLane};
use serde_json::{Map, Value as Json, json};
use song_edit::start_time::js_round;

struct Segment<'c> {
    clip: &'c PicClip,
    start: f64,
    end: f64,
    source_from: f64,
}

/// A lane flattened: segments, the dissolves INTO them (segment index, seconds), and the shots that could not be placed.
pub struct Flat<'c> {
    segments: Vec<Segment<'c>>,
    dissolves: Vec<(usize, f64)>,
    pub skipped: Vec<String>,
}

fn end_of(clips: &[&PicClip], i: usize, natural: &dyn Fn(&PicClip) -> Option<f64>) -> Option<f64> {
    let c = clips[i];
    if c.length > 0.0 {
        return Some(c.start + c.length);
    }
    if let Some(next) = clips.get(i + 1) {
        return Some(next.start);
    }
    let speed = c.speed.unwrap_or(1.0);
    let total = natural(c)?;
    (speed > 0.0).then(|| c.start + (total - c.source_from) / speed)
}

/// A lane as sequential segments with no overlap, by the lane's own rule; `natural` answers a source's own length.
pub fn flatten<'c>(lane: &'c PicLane, natural: &dyn Fn(&PicClip) -> Option<f64>) -> Flat<'c> {
    let clips: Vec<&PicClip> = lane.clips.iter().filter(|c| !c.id.is_empty() && !c.captions).collect();
    let mut skipped = Vec::new();
    let mut spans: Vec<(&PicClip, f64, f64)> = Vec::new();
    for (i, c) in clips.iter().enumerate() {
        match end_of(&clips, i, natural) {
            None => skipped.push(format!(
                "{}: runs to the end of a source whose length is unknown",
                c.name
            )),
            Some(end) if end > c.start => spans.push((c, c.start, end)),
            Some(_) => {}
        }
    }
    let mut cuts: Vec<f64> = spans.iter().flat_map(|s| [s.1, s.2]).collect();
    cuts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    cuts.dedup();
    let mut segments: Vec<Segment> = Vec::new();
    for w in cuts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let Some(top) = spans.iter().rfind(|s| s.1 <= a && s.2 >= b) else {
            continue;
        };
        match segments.last_mut() {
            Some(last) if std::ptr::eq(last.clip, top.0) && (last.end - a).abs() < 1e-9 => last.end = b,
            _ => segments.push(Segment {
                clip: top.0,
                start: a,
                end: b,
                source_from: top.0.source_from + (a - top.0.start) * top.0.speed.unwrap_or(1.0),
            }),
        }
    }
    let mut dissolves = Vec::new();
    for (index, s) in segments.iter().enumerate() {
        let over = s.clip.fade_in;
        if over > 0.0 && (s.start - s.clip.start).abs() < 1e-9 {
            dissolves.push((index, over.min(s.end - s.start)));
        }
    }
    Flat {
        segments,
        dissolves,
        skipped,
    }
}

/// A whole number as JSON prints it: `25`, not `25.0`.
fn n(x: f64) -> Json {
    if x.fract() == 0.0 && x.abs() < 9e15 {
        Json::from(x as i64)
    } else {
        Json::from(x)
    }
}

fn rt(seconds: f64, rate: f64) -> Json {
    json!({ "OTIO_SCHEMA": "RationalTime.1", "rate": n(rate), "value": n(js_round(seconds * rate)) })
}

fn range(start: f64, duration: f64, rate: f64) -> Json {
    json!({ "OTIO_SCHEMA": "TimeRange.1", "start_time": rt(start, rate), "duration": rt(duration, rate) })
}

fn warp(speed: f64) -> Vec<Json> {
    if speed == 1.0 {
        return Vec::new();
    }
    if speed == 0.0 {
        return vec![
            json!({ "OTIO_SCHEMA": "FreezeFrame.1", "name": "", "effect_name": "FreezeFrame", "time_scalar": 0, "metadata": {} }),
        ];
    }
    vec![
        json!({ "OTIO_SCHEMA": "LinearTimeWarp.1", "name": "", "effect_name": "LinearTimeWarp", "time_scalar": n(speed), "metadata": {} }),
    ]
}

/// The timeline as the JSON an `.otio` file holds, and the shots it had to leave out.
pub fn to_otio(
    doc: &PicDoc,
    name: &str,
    media_url: &dyn Fn(&PicClip) -> String,
    natural: &dyn Fn(&PicClip) -> Option<f64>,
) -> (String, Vec<String>) {
    let rate = doc.fps;
    let mut skipped = Vec::new();
    let mut tracks: Vec<Json> = Vec::new();
    for lane in &doc.lanes {
        let flat = flatten(lane, natural);
        skipped.extend(flat.skipped.iter().map(|s| format!("{}: {s}", lane.id)));
        let mut children: Vec<Json> = Vec::new();
        let mut at = 0.0;
        for (i, s) in flat.segments.iter().enumerate() {
            if s.start - at > 1e-6 {
                children.push(json!({
                    "OTIO_SCHEMA": "Gap.1", "name": "", "source_range": range(0.0, s.start - at, rate),
                    "effects": [], "markers": [], "metadata": {}
                }));
            }
            if let Some((_, over)) = flat.dissolves.iter().find(|d| d.0 == i) {
                children.push(json!({
                    "OTIO_SCHEMA": "Transition.1", "name": "", "transition_type": "SMPTE_Dissolve",
                    "in_offset": rt(0.0, rate), "out_offset": rt(*over, rate), "metadata": {}
                }));
            }
            let metadata = if s.clip.still {
                json!({ "k7": { "still": true } })
            } else {
                json!({})
            };
            children.push(json!({
                "OTIO_SCHEMA": "Clip.1", "name": s.clip.name,
                "source_range": range(s.source_from, s.end - s.start, rate),
                "media_reference": {
                    "OTIO_SCHEMA": "ExternalReference.1", "name": s.clip.name,
                    "target_url": media_url(s.clip), "available_range": null, "metadata": {}
                },
                "effects": warp(s.clip.speed.unwrap_or(1.0)), "markers": [], "metadata": metadata
            }));
            at = s.end;
        }
        tracks.push(json!({
            "OTIO_SCHEMA": "Track.1", "name": lane.name, "kind": "Video", "source_range": null,
            "children": children, "effects": [], "markers": [], "metadata": {}
        }));
    }
    tracks.retain(|t| t["children"].as_array().is_some_and(|c| !c.is_empty()));
    let mut meta = Map::new();
    meta.insert(
        "k7".to_owned(),
        json!({ "viewport": [n(doc.viewport[0]), n(doc.viewport[1])], "fps": n(doc.fps) }),
    );
    let timeline = json!({
        "OTIO_SCHEMA": "Timeline.1", "name": name, "global_start_time": null,
        "tracks": {
            "OTIO_SCHEMA": "Stack.1", "name": "tracks", "source_range": null, "children": tracks,
            "effects": [], "markers": [], "metadata": {}
        },
        "metadata": Json::Object(meta)
    });
    let text = serde_json::to_string_pretty(&timeline).unwrap_or_default();
    (format!("{text}\n"), skipped)
}

/// `HH:MM:SS:FF` at a whole frame rate, non-drop: the EDL's only clock.
pub fn timecode(seconds: f64, fps: f64) -> String {
    let f = js_round(seconds * fps).max(0.0);
    let p = |x: f64| format!("{:02}", x as i64);
    format!(
        "{}:{}:{}:{}",
        p((f / (fps * 3600.0)).floor()),
        p(((f / (fps * 60.0)).floor()) % 60.0),
        p(((f / fps).floor()) % 60.0),
        p(f % fps)
    )
}

/// Record timecodes start at 01:00:00:00, the convention every conform expects.
const RECORD_START: f64 = 3600.0;

/// `x.toFixed(1)`: a tie goes up, where Rust's formatting goes to even.
fn to_fixed1(x: f64) -> String {
    let t = x * 10.0;
    if (t - t.floor()) == 0.5 {
        return format!("{:.1}", (t.floor() + 1.0) / 10.0);
    }
    format!("{x:.1}")
}

/// One lane as EDL text, and the shots it had to leave out.
pub fn to_edl(
    lane: &PicLane,
    title: &str,
    fps_in: f64,
    clip_name: &dyn Fn(&PicClip) -> String,
    natural: &dyn Fn(&PicClip) -> Option<f64>,
) -> (String, Vec<String>) {
    let fps = js_round(fps_in).max(1.0);
    let tc = |s: f64| timecode(s, fps);
    let flat = flatten(lane, natural);
    let mut lines = vec![
        format!("TITLE: {title}"),
        "FCM: NON-DROP FRAME".to_owned(),
        String::new(),
    ];
    let mut count = 0;
    let mut num = || {
        count += 1;
        format!("{count:03}")
    };
    for (i, s) in flat.segments.iter().enumerate() {
        let speed = s.clip.speed.unwrap_or(1.0);
        let (src_in, src_out) = (s.source_from, s.source_from + (s.end - s.start) * speed);
        let (rec_in, rec_out) = (RECORD_START + s.start, RECORD_START + s.end);
        let prev = i.checked_sub(1).map(|p| &flat.segments[p]);
        let dissolve = flat.dissolves.iter().find(|d| d.0 == i);
        match (dissolve, prev) {
            (Some((_, over)), Some(prev)) if (prev.end - s.start).abs() < 1e-9 => {
                let id = num();
                let prev_at = prev.source_from + (prev.end - prev.start) * prev.clip.speed.unwrap_or(1.0);
                lines.push(format!(
                    "{id}  AX       V     C        {} {} {} {}",
                    tc(prev_at),
                    tc(prev_at),
                    tc(rec_in),
                    tc(rec_in)
                ));
                lines.push(format!(
                    "{id}  AX       V     D    {:0>3} {} {} {} {}",
                    js_round(over * fps) as i64,
                    tc(src_in),
                    tc(src_out),
                    tc(rec_in),
                    tc(rec_out)
                ));
                lines.push(format!("* FROM CLIP NAME: {}", clip_name(prev.clip)));
                lines.push(format!("* TO CLIP NAME: {}", clip_name(s.clip)));
            }
            _ => {
                lines.push(format!(
                    "{}  AX       V     C        {} {} {} {}",
                    num(),
                    tc(src_in),
                    tc(src_out),
                    tc(rec_in),
                    tc(rec_out)
                ));
                lines.push(format!("* FROM CLIP NAME: {}", clip_name(s.clip)));
            }
        }
        if speed != 1.0 {
            lines.push(format!(
                "M2   AX       {:0>6}    {}",
                to_fixed1(speed * fps),
                tc(src_in)
            ));
        }
        lines.push(String::new());
    }
    (format!("{}\n", lines.join("\n")), flat.skipped)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(id: &str, start: f64, length: f64) -> PicClip {
        PicClip {
            id: id.to_owned(),
            name: id.to_owned(),
            source: format!("/p/{id}.mp4"),
            still: false,
            captions: false,
            start,
            length,
            source_from: 0.0,
            fade_in: 0.0,
            speed: None,
        }
    }

    #[test]
    fn the_later_shot_covers_the_earlier_one_where_they_overlap() {
        let lane = PicLane {
            id: "main".to_owned(),
            name: "main".to_owned(),
            clips: vec![clip("a", 0.0, 6.0), clip("b", 4.0, 4.0)],
        };
        let flat = flatten(&lane, &|_| None);
        let spans: Vec<(String, f64, f64)> = flat
            .segments
            .iter()
            .map(|s| (s.clip.id.clone(), s.start, s.end))
            .collect();
        assert_eq!(spans, [("a".to_owned(), 0.0, 4.0), ("b".to_owned(), 4.0, 8.0)]);
    }

    #[test]
    fn an_open_last_shot_with_no_known_source_length_is_said_to_be_left_out() {
        let lane = PicLane {
            id: "main".to_owned(),
            name: "main".to_owned(),
            clips: vec![clip("a", 0.0, 0.0)],
        };
        assert_eq!(
            flatten(&lane, &|_| None).skipped,
            ["a: runs to the end of a source whose length is unknown"]
        );
        assert_eq!(flatten(&lane, &|_| Some(3.0)).segments.len(), 1);
    }

    #[test]
    fn a_timecode_counts_frames_from_midnight() {
        assert_eq!(timecode(3600.0 + 61.5, 30.0), "01:01:01:15");
        assert_eq!(to_fixed1(12.25), "12.3");
    }
}
