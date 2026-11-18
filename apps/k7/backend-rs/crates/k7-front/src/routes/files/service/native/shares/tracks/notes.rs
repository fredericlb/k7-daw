//! notes.rs — a track's notes as placed on the timeline: every enabled MIDI clip at its start, its loop and its pads' effects realised.
//! What a reader gets wrong: it follows the engine's own placement (`EditBuilder.cpp`), so a clip's `offset` is not read, as the engine does not.

use serde_json::Value;

/// One note on the timeline, in beats.
#[derive(Clone, Debug, PartialEq)]
pub struct Note {
    pub start: f64,
    pub length: f64,
    pub key: u8,
    pub vel: u8,
    pub channel: u8,
}

/// The engine's passes cap for a looped clip with skipped passes.
const MAX_PASSES: i64 = 4096;

/// Every note of `track`'s enabled MIDI clips, by start then key.
pub fn notes_of(track: &Value) -> Vec<Note> {
    let mut all = Vec::new();
    for clip in super::strip::authored(track) {
        if clip["type"] == "midi" && clip["disabled"] != true {
            clip_notes(clip, &mut all);
        }
    }
    all.sort_by(|a, b| a.start.total_cmp(&b.start).then(a.key.cmp(&b.key)));
    all
}

fn num(v: &Value, key: &str) -> f64 {
    v[key].as_f64().unwrap_or(0.0)
}

fn clip_notes(clip: &Value, out: &mut Vec<Note>) {
    let at = num(clip, "start");
    let length = num(clip, "length");
    let (from, to) = (num(clip, "loopStart"), num(clip, "loopEnd"));
    let window = if to > from { to - from } else { 0.0 };
    let source: Vec<&Value> = clip["notes"].as_array().into_iter().flatten().collect();
    // The engine's clip is four beats long when it has no length.
    let span = if length > 0.0 { length } else { 4.0 };
    let any_skip = source.iter().any(|n| n["skip"].as_u64().unwrap_or(0) >= 2);
    if window <= 0.0 {
        for n in &source {
            place(n, at, n["time"].as_f64().unwrap_or(0.0), span, out);
        }
        return;
    }
    if !any_skip {
        // Tracktion repeats the window to the clip's end, a partial last pass kept.
        let passes = ((span / window).ceil() as i64).clamp(1, MAX_PASSES);
        return passes_of(&source, (at, from, to), span, passes, out);
    }
    let passes = ((span / window).round() as i64).clamp(1, MAX_PASSES);
    passes_of(&source, (at, from, to), span, passes, out);
}

fn passes_of(
    source: &[&Value],
    (at, from, to): (f64, f64, f64),
    span: f64,
    passes: i64,
    out: &mut Vec<Note>,
) {
    let window = to - from;
    for p in 0..passes {
        for n in source {
            let time = num(n, "time");
            if time < from || time >= to {
                continue;
            }
            let skip = n["skip"].as_u64().unwrap_or(0);
            if skip >= 2 && (p as u64 + 1).is_multiple_of(skip) {
                continue;
            }
            place(n, at, (time - from) + p as f64 * window, span, out);
        }
    }
}

/// `note` starting `time` beats into a clip at `at`, cut at the clip's end `length`, its ratchet unrolled.
fn place(note: &Value, at: f64, time: f64, length: f64, out: &mut Vec<Note>) {
    if time >= length {
        return;
    }
    let dur = match num(note, "duration") {
        d if d > 0.0 => d,
        _ => 0.25,
    };
    let dur = dur.min(length - time);
    let hits = note["repeat"].as_u64().filter(|r| *r >= 2).unwrap_or(1);
    let each = dur / hits as f64;
    let vel = (num(note, "vel") * 127.0).round().clamp(1.0, 127.0) as u8;
    let key = num(note, "key").clamp(0.0, 127.0) as u8;
    let channel = num(note, "channel").clamp(0.0, 15.0) as u8;
    for i in 0..hits {
        out.push(Note {
            start: at + time + i as f64 * each,
            length: each,
            key,
            vel,
            channel,
        });
    }
}
