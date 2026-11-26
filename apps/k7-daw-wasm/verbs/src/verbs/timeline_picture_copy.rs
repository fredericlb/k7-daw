// timeline_picture_copy.rs — `timeline lane copy` of a PICTURE lane: its shots onto a new picture lane of a timeline, or of the song.
// A reader would get wrong the frame: a timeline's picture lane has none and draws on the song's lane of its name, so only a copy within the song keeps one.

use super::picture_shared::{lane_file, picture_lane_ids, taken_in};
use super::shared::{Done, free_name, join};
use super::timeline::SONG_TARGET;
use super::words::is_name;
use crate::host::{Host, Thrown};
use k7x::edit::{Document, PathSeg, Value, WriteOptions};
use song_edit::chain_item_edit::{lane_file_for, lane_folder, timeline_lane_of};
use song_edit::lane_files::create_picture_lane;
use song_edit::module_io::entity_files;
use song_edit::workspace_doc::{EditOpts, ws_data, ws_edit, ws_text};
use song_edit::workspace_file::ws_exists;

/// The song's own frame and colour, which a copy within the song keeps.
const LAYER_KEYS: [&str; 2] = ["color", "frame"];

fn key(k: &str) -> Vec<PathSeg> {
    vec![PathSeg::Key(k.to_owned())]
}

/// Whether `lane` names a picture lane, the song's or a timeline's, without refusing anything.
pub fn is_picture_lane(h: &Host, lane: &str) -> Result<bool, Thrown> {
    let Some((timeline, tl_lane)) = timeline_lane_of(lane) else {
        return Ok(is_name(lane) && picture_lane_ids(h)?.iter().any(|p| p == lane));
    };
    let file = join(&h.song_path, &format!("timelines/{timeline}.k7x"));
    if !is_name(timeline) || !ws_exists(&*h.fs, &file) {
        return Ok(false);
    }
    Ok(match ws_data(&*h.fs, &file)?.get("pictures") {
        Some(Value::List(l)) => l
            .iter()
            .any(|p| matches!(p.get("lane"), Some(Value::Str(s)) if s == tl_lane)),
        _ => false,
    })
}

/// Copies picture lane `lane_arg` to timeline `to`, or to the song when `to` is `main`.
pub fn copy_picture_lane(h: &mut Host, lane_arg: &str, to: &str, name: Option<&String>) -> Done {
    let Some(from) = lane_file(h, lane_arg)? else {
        return Ok(());
    };
    let mut doc = Document::new(&ws_text(&*h.fs, &from.abs)?);
    let map = doc
        .at(&from.base)
        .to_js()
        .map_err(|e| Thrown(e.to_string()))?
        .unwrap_or(Value::Map(vec![]));
    let events = match map.get("events") {
        Some(Value::List(l)) => l.clone(),
        _ => Vec::new(),
    };
    let label = timeline_lane_of(lane_arg).map_or(lane_arg, |(_, l)| l).to_owned();
    let dir = h.song_path.clone();
    let opts = EditOpts {
        create: false,
        raw: false,
    };
    if to == SONG_TARGET {
        let sub = lane_folder(&*h.fs, &dir);
        let taken: Vec<String> = entity_files(&*h.fs, &join(&dir, sub), sub)
            .into_iter()
            .map(|(i, _)| i)
            .collect();
        let id = name
            .cloned()
            .unwrap_or_else(|| free_name(&label, &|n| taken.iter().any(|t| t == n), true, 2));
        let keep: Vec<(&str, Value)> = if from.timeline.is_some() {
            Vec::new()
        } else {
            LAYER_KEYS
                .iter()
                .filter_map(|k| map.get(k).map(|v| (*k, v.clone())))
                .collect()
        };
        let made = create_picture_lane(h.fs, &h.env, &dir, &id, h.new_id).and_then(|(written, _)| {
            let Some(lane) = lane_file_for(&*h.fs, &dir, &id) else {
                return song_edit::error::fail(format!("lane '{id}' has no file"));
            };
            ws_edit(h.fs, &lane.path, opts, |doc| {
                for (k, v) in &keep {
                    doc.set_in(&key(k), v, WriteOptions::default())?;
                }
                if !events.is_empty() {
                    doc.set_in(&key("events"), &Value::List(events.clone()), WriteOptions::default())?;
                }
                Ok(())
            })?;
            Ok(written)
        });
        match made {
            Ok(written) => h.log([format!(
                "+ picture lane {id}: {} shot(s) of {lane_arg} ({})",
                events.len(),
                written.join(", ")
            )]),
            Err(e) => {
                h.refuse([format!("timeline lane copy failed: {e}")]);
                return Ok(());
            }
        }
        h.reload();
        return Ok(());
    }
    let file = join(&dir, &format!("timelines/{to}.k7x"));
    if !is_name(to) || !ws_exists(&*h.fs, &file) {
        h.refuse([format!(
            "no timeline \"{to}\" — name a timeline of timelines/, or {SONG_TARGET} for the main one"
        )]);
        return Ok(());
    }
    let taken = taken_in(&ws_data(&*h.fs, &file)?);
    let id = name
        .cloned()
        .unwrap_or_else(|| free_name(&label, &|n| taken.iter().any(|t| t == n), true, 2));
    if !is_name(&id) || taken.contains(&id) {
        h.refuse([format!(
            "'{id}' cannot name a new lane of '{to}' — taken, or not a name"
        )]);
        return Ok(());
    }
    let entry = Value::Map(vec![
        ("lane".to_owned(), Value::Str(id.clone())),
        ("events".to_owned(), Value::List(events.clone())),
    ]);
    ws_edit(h.fs, &file, opts, |doc| {
        Ok(doc.add_in(&key("pictures"), &entry, WriteOptions::default())?)
    })?;
    h.log([format!(
        "{to}: + picture lane {id}, {} shot(s) of {lane_arg} — it draws on the song's picture lane '{id}'",
        events.len()
    )]);
    h.reload();
    Ok(())
}
