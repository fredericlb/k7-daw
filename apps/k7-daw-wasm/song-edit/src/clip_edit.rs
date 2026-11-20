// clip_edit.rs — a clip's own file: its onset corrections and its stretch map, found from either of the clip's two names.
// A reader would get wrong which name a caller holds: half hold a PLACEMENT id, whose material is the clip the file is named for.

use crate::error::{Res, fail};
use crate::fs::Fs;
use crate::module_file::set_module_value;
use crate::module_io::{ModuleFile, entity_file_for};
use crate::paths::join;
use crate::workspace_doc::{EditOpts, ws_data, ws_edit};
use k7x::edit::{PathSeg, Value, WriteOptions};
use song::clips::material_ref_of;

fn key(k: &str) -> PathSeg {
    PathSeg::Key(k.to_owned())
}

fn clip_file_of(fs: &dyn Fs, dir: &str, id: &str) -> Option<ModuleFile> {
    entity_file_for(fs, &join(&[dir, "clips"]), "clips", material_ref_of(id))
}

fn missing(id: &str) -> String {
    let r = material_ref_of(id);
    format!("no clip \"{id}\" in this project — clips/{r}.k7x does not exist")
}

/// Where a clip's own file is.
///
/// # Errors
/// A clip the project does not hold.
pub fn clip_file_path(fs: &dyn Fs, dir: &str, id: &str) -> Res<String> {
    match clip_file_of(fs, dir, id) {
        Some(f) => Ok(f.path),
        None => fail(missing(id)),
    }
}

/// Write a clip's onset corrections as given; corrections that cancel out remove the key.
///
/// # Errors
/// A clip the project does not hold, or the write's.
pub fn set_clip_onsets(fs: &mut dyn Fs, dir: &str, id: &str, edits: &Value) -> Res<String> {
    let path = clip_file_path(fs, dir, id)?;
    let has = |k: &str| matches!(edits.get(k), Some(Value::List(l)) if !l.is_empty());
    let empty = !has("added") && !has("removed");
    ws_edit(fs, &path, EditOpts::default(), |doc| {
        set_module_value(&mut doc.at(&[]), &[key("onsets")], (!empty).then_some(edits))
    })?;
    Ok(path)
}

/// A JS `Number(x)` of a marker's two members.
fn number(v: &Value) -> f64 {
    match v {
        Value::Num(n) => *n,
        Value::Bool(b) => f64::from(u8::from(*b)),
        Value::Str(s) => s.trim().parse().unwrap_or(f64::NAN),
        Value::Null => 0.0,
        _ => f64::NAN,
    }
}

/// A clip's `[timelineBeat, sourceSecond]` pairs exactly as the file states them, or None.
///
/// # Errors
/// A clip the project does not hold, or one that does not read.
pub fn clip_warp_markers(fs: &dyn Fs, dir: &str, id: &str) -> Res<Option<Vec<(f64, f64)>>> {
    let raw = ws_data(fs, &clip_file_path(fs, dir, id)?)?;
    let Some(Value::List(m)) = raw.get("warp").and_then(|w| w.get("markers")) else {
        return Ok(None);
    };
    let out: Vec<(f64, f64)> = m
        .iter()
        .filter_map(|p| match p {
            Value::List(l) if l.len() == 2 => Some((number(&l[0]), number(&l[1]))),
            _ => None,
        })
        .collect();
    Ok((!out.is_empty()).then_some(out))
}

/// Write a clip's stretch map; `algorithm` is only used when the block is created.
///
/// # Errors
/// A clip the project does not hold, or the write's.
pub fn set_clip_warp(
    fs: &mut dyn Fs,
    dir: &str,
    id: &str,
    markers: &[[Option<f64>; 2]],
    algorithm: &str,
) -> Res<String> {
    let path = clip_file_path(fs, dir, id)?;
    ws_edit(fs, &path, EditOpts::default(), |doc| {
        let alg = [key("warp"), key("algorithm")];
        let unset = match doc.get_in(&alg)? {
            None | Some(Value::Null) => true,
            Some(Value::Str(s)) => s.is_empty(),
            Some(Value::Num(n)) => n == 0.0 || n.is_nan(),
            Some(Value::Bool(b)) => !b,
            Some(_) => false,
        };
        if unset {
            doc.set_in(&alg, &Value::Str(algorithm.to_owned()), WriteOptions::default())?;
        }
        // Applied rather than replaced: a marker the edit leaves alone keeps its line and its comment.
        let num = |x: Option<f64>| x.map_or(Value::Null, Value::Num);
        let list = Value::List(
            markers
                .iter()
                .map(|m| Value::List(vec![num(m[0]), num(m[1])]))
                .collect(),
        );
        doc.apply_in(&[key("warp"), key("markers")], &list)?;
        Ok(())
    })?;
    Ok(path)
}

/// A clip's onset corrections as its file holds them: an empty map when it has none.
#[must_use]
pub fn clip_onsets(fs: &dyn Fs, dir: &str, id: &str) -> Value {
    let Some(file) = clip_file_of(fs, dir, id) else {
        return Value::Map(vec![]);
    };
    let Ok(raw) = ws_data(fs, &file.path) else {
        return Value::Map(vec![]);
    };
    let mut out = Vec::new();
    for k in ["added", "removed"] {
        if let Some(Value::List(l)) = raw.get("onsets").and_then(|o| o.get(k))
            && !l.is_empty()
        {
            out.push((k.to_owned(), Value::List(l.clone())));
        }
    }
    Value::Map(out)
}
