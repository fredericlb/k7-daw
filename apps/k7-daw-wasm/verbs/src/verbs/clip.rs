// clip.rs — a clip's own file: its stretch map's points and its onset corrections; after verbs/clip.ts and song-schema's warpPoints.ts.
// A reader would get wrong the units: a warp point pins a SOURCE second to a timeline beat, and onsets are file seconds.
// A clip with no map is seeded from its placed length and its wav's header, where the TypeScript asks the engine's envelope.

use super::shared::{Done, split_words};
use crate::host::Host;
use k7x::edit::{Value, js_number};
use song::clips::material_ref_of;
use song_edit::clip_edit::{clip_onsets, clip_warp_markers, set_clip_onsets, set_clip_warp};
use song_edit::js_text::str_to_number;
use song_edit::onsets::{OnsetEdits, add_onset, move_onset, remove_onset};

/// One point of the map: `(timelineBeat, sourceSecond)`.
type Marker = (f64, f64);

/// How close two source seconds have to be to mean the same point.
const SAME_SEC: f64 = 1e-3;

fn non_monotonic(m: &[Marker]) -> Option<String> {
    m.windows(2).find_map(|w| {
        let (p, n) = (w[0], w[1]);
        if n.1 <= p.1 {
            Some(format!(
                "warp points must rise through the source: {}s is not after {}s",
                js_number(n.1),
                js_number(p.1)
            ))
        } else if n.0 <= p.0 {
            Some(format!(
                "a warp point must stay between its neighbours: beat {} is not after beat {}",
                js_number(n.0),
                js_number(p.0)
            ))
        } else {
            None
        }
    })
}

/// `setWarpPoint`: moves the point at `sec` to `beat`, or places one there; neighbours stay put.
fn set_warp_point(markers: &[Marker], sec: f64, beat: f64) -> Result<Vec<Marker>, String> {
    let mut out = markers.to_vec();
    match out.iter().position(|m| (m.1 - sec).abs() <= SAME_SEC) {
        Some(at) => out[at].0 = beat,
        None => {
            let i = out.iter().position(|m| m.1 > sec).unwrap_or(out.len());
            out.insert(i, (beat, sec));
        }
    }
    non_monotonic(&out).map_or(Ok(out), Err)
}

/// `removeWarpPoint`: drops the point at `sec`, never below two.
fn remove_warp_point(markers: &[Marker], sec: f64) -> Result<Vec<Marker>, String> {
    let Some(at) = markers.iter().position(|m| (m.1 - sec).abs() <= SAME_SEC) else {
        return Err(format!("no warp point at {}s", js_number(sec)));
    };
    if markers.len() <= 2 {
        return Err("a stretch map needs two points — remove the whole `warp:` block instead".to_owned());
    }
    let mut out = markers.to_vec();
    out.remove(at);
    Ok(out)
}

fn write_warp(h: &mut Host, clip: &str, next: &[Marker]) -> Result<(), String> {
    let dir = h.song_path.clone();
    let rows: Vec<[Option<f64>; 2]> = next.iter().map(|m| [Some(m.0), Some(m.1)]).collect();
    set_clip_warp(h.fs, &dir, clip, &rows, "elastique").map_err(|e| e.to_string())?;
    Ok(())
}

/// The map seeded for a clip with none: `[0, 0]` to its placed length and its file's seconds.
fn seed(h: &mut Host, clip: &str) -> Result<Option<Vec<Marker>>, crate::host::Thrown> {
    let doc = h.project()?;
    let found = doc
        .tracks
        .iter()
        .flat_map(|t| t.clips.iter())
        .find(|c| c.id == clip || material_ref_of(&c.id) == clip);
    let seconds = found
        .filter(|c| !c.source.is_empty())
        .and_then(|c| h.seconds(&c.source))
        .unwrap_or(0.0);
    // An unwindowed clip's length is the engine's to measure: unwarped, its file's seconds at the song's tempo.
    let beats = found.map_or(0.0, |c| {
        if c.length > 0.0 {
            c.length
        } else {
            seconds * doc.tempo / 60.0
        }
    });
    Ok((beats > 0.0 && seconds > 0.0).then(|| vec![(0.0, 0.0), (beats, seconds)]))
}

/// `clip warp <clip> <sec> <beat>` — pin a moment of the source to a beat; one verb places and moves.
pub fn clip_warp(a: &str, h: &mut Host) -> Done {
    let w = split_words(a);
    if w.len() < 3 {
        h.refuse(["usage: clip warp <clip> <sec> <beat>  (e.g. clip warp break 1.94 4)".to_owned()]);
        return Ok(());
    }
    let (clip, sec, beat) = (w[0], str_to_number(w[1]), str_to_number(w[2]));
    if !sec.is_finite() || !beat.is_finite() || sec < 0.0 || beat < 0.0 {
        h.refuse(["sec and beat must be numbers, counted from zero".to_owned()]);
        return Ok(());
    }
    let dir = h.song_path.clone();
    let markers = match clip_warp_markers(&*h.fs, &dir, clip) {
        Ok(Some(m)) => m,
        Ok(None) => match seed(h, clip)? {
            Some(m) => m,
            None => {
                h.refuse([
                    format!("{clip}: no stretch map, and no length to seed one from"),
                    "a clip has to be placed on a lane, and its file read, before it can be warped".to_owned(),
                ]);
                return Ok(());
            }
        },
        Err(e) => {
            h.refuse([format!("clip warp: {e}")]);
            return Ok(());
        }
    };
    match set_warp_point(&markers, sec, beat).and_then(|next| write_warp(h, clip, &next).map(|()| next.len())) {
        Ok(n) => {
            h.log([format!(
                "{clip}: {}s → beat {} ({n} points)",
                js_number(sec),
                js_number(beat)
            )]);
            h.reload();
        }
        Err(e) => h.refuse([format!("clip warp: {e}")]),
    }
    Ok(())
}

/// `clip warp remove <clip> <sec>` — drop one stretch point, never below two.
pub fn clip_warp_remove(a: &str, h: &mut Host) -> Done {
    let w = split_words(a);
    if w.len() < 2 {
        h.refuse(["usage: clip warp remove <clip> <sec>".to_owned()]);
        return Ok(());
    }
    let (clip, sec) = (w[0], str_to_number(w[1]));
    if !sec.is_finite() {
        h.refuse(["sec must be a number — seconds into the source file".to_owned()]);
        return Ok(());
    }
    let dir = h.song_path.clone();
    let markers = match clip_warp_markers(&*h.fs, &dir, clip) {
        Ok(Some(m)) => m,
        Ok(None) => {
            h.refuse([format!("{clip}: no stretch map to remove a point from")]);
            return Ok(());
        }
        Err(e) => {
            h.refuse([format!("clip warp remove: {e}")]);
            return Ok(());
        }
    };
    match remove_warp_point(&markers, sec).and_then(|next| write_warp(h, clip, &next).map(|()| next.len())) {
        Ok(n) => {
            h.log([format!("{clip}: dropped the point at {}s ({n} left)", js_number(sec))]);
            h.reload();
        }
        Err(e) => h.refuse([format!("clip warp remove: {e}")]),
    }
    Ok(())
}

fn secs(v: Option<&Value>) -> Option<Vec<f64>> {
    match v {
        Some(Value::List(l)) => Some(
            l.iter()
                .filter_map(|x| match x {
                    Value::Num(n) => Some(*n),
                    _ => None,
                })
                .collect(),
        ),
        _ => None,
    }
}

fn edits_value(e: &OnsetEdits) -> Value {
    let list = |x: &Vec<f64>| Value::List(x.iter().map(|n| Value::Num(*n)).collect());
    let mut out = Vec::new();
    if let Some(a) = &e.added {
        out.push(("added".to_owned(), list(a)));
    }
    if let Some(r) = &e.removed {
        out.push(("removed".to_owned(), list(r)));
    }
    Value::Map(out)
}

/// `clip onset add|remove|move <clip> <sec…>` — a correction to the detected onsets, in FILE seconds.
fn onset(a: &str, h: &mut Host, op: &str) -> Done {
    let w = split_words(a);
    let want = if op == "move" { 2 } else { 1 };
    let times: Vec<f64> = w.iter().skip(1).take(want).map(|t| str_to_number(t)).collect();
    if w.is_empty() || times.len() < want || times.iter().any(|s| !s.is_finite()) {
        h.refuse([if op == "move" {
            "usage: clip onset move <clip> <from-sec> <to-sec>".to_owned()
        } else {
            format!("usage: clip onset {op} <clip> <sec>")
        }]);
        return Ok(());
    }
    let clip = w[0];
    let dir = h.song_path.clone();
    let raw = clip_onsets(&*h.fs, &dir, clip);
    let before = OnsetEdits {
        added: secs(raw.get("added")),
        removed: secs(raw.get("removed")),
    };
    let after = match op {
        "add" => add_onset(Some(&before), times[0]),
        "remove" => remove_onset(Some(&before), &[], times[0]),
        _ => move_onset(Some(&before), &[], times[0], times[1]),
    };
    if let Err(e) = set_clip_onsets(h.fs, &dir, clip, &edits_value(&after)) {
        h.refuse([format!("clip onset {op}: {e}")]);
        return Ok(());
    }
    let (added, removed) = (
        after.added.as_ref().map_or(0, Vec::len),
        after.removed.as_ref().map_or(0, Vec::len),
    );
    h.log([if added + removed == 0 {
        format!("{clip}: onsets back to what the engine detects")
    } else {
        format!("{clip}: {added} added, {removed} removed")
    }]);
    h.reload();
    Ok(())
}

pub fn onset_add(a: &str, h: &mut Host) -> Done {
    onset(a, h, "add")
}

pub fn onset_remove(a: &str, h: &mut Host) -> Done {
    onset(a, h, "remove")
}

pub fn onset_move(a: &str, h: &mut Host) -> Done {
    onset(a, h, "move")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_point_is_placed_by_source_second_and_the_map_stays_monotonic() {
        let m = vec![(0.0, 0.0), (4.0, 2.0)];
        assert_eq!(
            set_warp_point(&m, 1.0, 2.5).unwrap(),
            vec![(0.0, 0.0), (2.5, 1.0), (4.0, 2.0)]
        );
        assert_eq!(set_warp_point(&m, 2.0005, 5.0).unwrap(), vec![(0.0, 0.0), (5.0, 2.0)]);
        assert!(
            set_warp_point(&m, 1.0, 5.0)
                .unwrap_err()
                .contains("between its neighbours")
        );
        assert!(remove_warp_point(&m, 2.0).unwrap_err().contains("needs two points"));
        assert!(remove_warp_point(&m, 3.0).unwrap_err().contains("no warp point at 3s"));
    }
}
