// format_stamp.rs — `format(N)` written into project.k7x the moment the project starts NEEDING level N, never before.
// A reader would get wrong that this is best-effort: a project that will not read still opens, and the next open tries again.

use crate::chain_item_edit::lane_folder;
use crate::format_level::{FORMAT_KEY, SECONDS_START_LEVEL, format_level_of_text};
use crate::fs::Fs;
use crate::module_io::{entity_file_for, entity_files};
use crate::paths::join;
use crate::workspace_doc::{EditOpts, ws_data, ws_edit, ws_text};
use k7x::edit::{PathSeg, Value, WriteOptions};

fn seconds_in(events: Option<&Value>) -> bool {
    matches!(events, Some(Value::List(l)) if l.iter().any(|p| matches!(p, Value::Map(_)) && matches!(p.get("start"), Some(Value::Num(_)))))
}

/// Whether a lane or timeline file's data places an event in seconds: its own list, each alternative's, each timeline lane's.
#[must_use]
pub fn holds_seconds_starts(data: &Value) -> bool {
    if !matches!(data, Value::Map(_)) {
        return false;
    }
    if seconds_in(data.get("events")) {
        return true;
    }
    let nested = |key: &str| match data.get(key) {
        Some(Value::List(l)) => l.clone(),
        _ => vec![],
    };
    nested("alternatives")
        .iter()
        .chain(&nested("lanes"))
        .any(|x| matches!(x, Value::Map(_)) && seconds_in(x.get("events")))
}

/// The level a root file on disk declares, read off its raw text; absent means 1.
///
/// # Errors
/// The read's.
pub fn declared_level(fs: &dyn Fs, path: &str) -> crate::Res<u32> {
    Ok(format_level_of_text(&ws_text(fs, path)?).unwrap_or(1))
}

/// Raise project.k7x's `format(N)` to `level` when it declares less; true when it wrote.
pub fn stamp_format_level(fs: &mut dyn Fs, dir: &str, level: u32) -> bool {
    let Some(file) = entity_file_for(fs, dir, "", "project") else {
        return false;
    };
    // Best-effort: a root that will not read is the compile's to report.
    let Ok(declared) = declared_level(fs, &file.path) else {
        return false;
    };
    if declared >= level {
        return false;
    }
    let key = [PathSeg::Key(FORMAT_KEY.to_owned())];
    let set = |doc: &mut k7x::edit::Document| {
        Ok(doc
            .at(&[])
            .set_in(&key, &Value::Num(f64::from(level)), WriteOptions::default())?)
    };
    ws_edit(
        fs,
        &file.path,
        EditOpts {
            create: false,
            raw: true,
        },
        set,
    )
    .unwrap_or(false)
}

/// Stamp the level seconds need when any lane or timeline of `dir` writes one: what an OPEN runs.
pub fn stamp_format_for_seconds(fs: &mut dyn Fs, dir: &str) -> bool {
    let lanes = lane_folder(fs, dir);
    let mut files = entity_files(fs, &join(&[dir, lanes]), lanes);
    files.extend(entity_files(fs, &join(&[dir, "timelines"]), "timelines"));
    // A file that will not read is reported by the compile, not here.
    let needs = files
        .iter()
        .any(|(_, f)| ws_data(fs, &f.path).is_ok_and(|d| holds_seconds_starts(&d)));
    needs && stamp_format_level(fs, dir, SECONDS_START_LEVEL)
}
