// timeline_from_range.rs — the events of ONE lane starting in a range, moved into a new timeline and replaced by one event of it.
// A reader would get wrong that the range takes events by START, as `events move` does: one starting before it stays where it is.

use crate::chain_item_edit::{lane_events_at, timeline_lane_of};
use crate::error::{Error, Res, fail, io_error};
use crate::fs::Fs;
use crate::paths::join;
use crate::placement_move::shift_takes;
use crate::song_tempo::song_tempo;
use crate::start_time::{start_as, start_beats, to_position};
use crate::track_edit::edit_track_struct;
use crate::track_file::{add_placement, placement_values, remove_placement_at};
use crate::workspace_doc::{ws_text, ws_write_text};
use crate::workspace_file::ws_exists;
use k7x::edit::{Document, StringifyOptions, Value, stringify};
use song_schema::beats_to_duration_on;

/// Move lane `lane_id`'s events starting in `[from, end)` (beats) into timeline `name`; how many moved, 0 writing nothing.
///
/// # Errors
/// A timeline already there, a lane with no file, or the file system's.
pub fn timeline_from_range(
    fs: &mut dyn Fs,
    dir: &str,
    lane_id: &str,
    from: f64,
    end: f64,
    num: f64,
    den: f64,
    name: &str,
) -> Res<usize> {
    let file = join(&[dir, "timelines", &format!("{name}.k7x")]);
    if ws_exists(fs, &file) {
        return fail(format!("timeline '{name}' already exists"));
    }
    let Some(at) = lane_events_at(fs, dir, lane_id)? else {
        return fail(format!("track \"{lane_id}\" has no file"));
    };
    let tempo = song_tempo(fs, dir)?;
    let start_of = |p: &Value| start_beats(p.get("start"), num, den, tempo, "1:1:1").ok();
    let in_range = |p: &Value| start_of(p).is_some_and(|s| s >= from - 1e-9 && s < end - 1e-9);
    let moved: Vec<Value> = {
        let mut doc = Document::new(&ws_text(fs, &at.path)?);
        placement_values(&doc.at(&at.base))?
            .into_iter()
            .filter(|p| in_range(p))
            .collect()
    };
    if moved.is_empty() {
        return Ok(0);
    }
    // Its lane is named after the one it leaves; a timeline's own lane leaves the part after its `>`.
    let lane: String = timeline_lane_of(lane_id)
        .map_or(lane_id, |(_, l)| l)
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-') {
                c
            } else {
                '-'
            }
        })
        .collect();
    let mut events = vec![];
    for p in &moved {
        let s = start_of(p).unwrap_or(0.0);
        let shifted = shift_takes(p, -from, num, den)?;
        // In the unit it was written in, now counted from the timeline's own start.
        let start = start_as(p.get("start"), s - from, num, den, tempo)?;
        events.push(crate::track_file::with_field(&shifted, "start", Some(start)));
    }
    let folder = join(&[dir, "timelines"]);
    fs.mkdir_all(&folder).map_err(|e| io_error(&e, "mkdir", &folder))?;
    let length = beats_to_duration_on(end - from, num, den).map_err(|e| Error::Msg(e.to_string()))?;
    let body = Value::object(vec![
        ("length".to_owned(), Value::Str(length)),
        (
            "lanes".to_owned(),
            Value::List(vec![Value::object(vec![
                ("lane".to_owned(), Value::Str(lane)),
                ("events".to_owned(), Value::List(events)),
            ])]),
        ),
    ]);
    ws_write_text(fs, &file, &stringify(&body, &StringifyOptions::default())?)?;
    let placed = Value::object(vec![
        ("ref".to_owned(), Value::Str(name.to_owned())),
        ("start".to_owned(), Value::Str(to_position(from, num, den)?)),
    ]);
    let edited = edit_track_struct(
        fs,
        dir,
        lane_id,
        &format!("move a range into timeline \"{name}\""),
        |doc| {
            let all = placement_values(doc)?;
            for i in (0..all.len()).rev() {
                if in_range(&all[i]) {
                    remove_placement_at(doc, i)?;
                }
            }
            add_placement(doc, &placed)
        },
    );
    if let Err(e) = edited {
        // The lane kept its events: a timeline holding copies of them would play them twice once placed.
        let _ = fs.remove(&file);
        return Err(e);
    }
    Ok(moved.len())
}
