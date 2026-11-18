// lane_files.rs — creating a LANE's files: its node or its `tracks/` file with a chain, its place in `trackOrder`, and its strip.
// A reader would get wrong that a chain project's lane is two files: the track and the `start: true` chain that plays it.

use crate::chain_create::create_chain_lane;
use crate::chain_item_edit::is_chain_project;
use crate::confine::Env;
use crate::error::{Res, fail, io_error};
use crate::fs::Fs;
use crate::ids::NewId;
use crate::lane_strip_plan::strip_new_lane;
use crate::loader::resolve_all;
use crate::module_file::text_of;
use crate::module_io::entity_file_for;
use crate::node_files::NODES_DIR;
use crate::paths::join;
use crate::project_root::load_project_root;
use crate::workspace_doc::ws_write_text;
use crate::yaml_edit::set_file_value;
use k7x::edit::{PathSeg, StringifyOptions, Value, stringify};
use song::doc::order_tracks;
use song::lanes::lanes_of;
use std::collections::HashSet;

/// The module a picture lane's file extends.
pub const PICTURE_TRACK_REF: &str = "@meta/picture-track";

fn mkdirs(fs: &mut dyn Fs, dir: &str) -> Res<()> {
    fs.mkdir_all(dir).map_err(|e| io_error(&e, "mkdir", dir))
}

fn plain(v: Vec<(String, Value)>) -> Value {
    Value::object(v)
}

fn track_order(fs: &dyn Fs, dir: &str) -> Res<Option<Vec<Value>>> {
    Ok(match load_project_root(fs, dir, true)?.data.get("trackOrder") {
        Some(Value::List(l)) => Some(l.clone()),
        _ => None,
    })
}

/// Create a LANE's file (`nodes/`, or `tracks/` and its chain in a chain project), appending to `trackOrder` when there is one.
///
/// # Errors
/// A name the files cannot carry, a lane already there, or what the writes fail with.
pub fn create_track_file(fs: &mut dyn Fs, dir: &str, id: &str, reference: &str, new_id: NewId) -> Res<Vec<String>> {
    // The filename is the id, and the id is what a chain's key names.
    if id.is_empty()
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
    {
        return fail(format!(
            "invalid track name '{id}' — letters, digits, '.', '_' and '-' only"
        ));
    }
    let body = stringify(
        &plain(vec![
            ("extends".to_owned(), Value::Str(reference.to_owned())),
            ("events".to_owned(), Value::List(vec![])),
        ]),
        &StringifyOptions::default(),
    )?;
    let rel = format!("{NODES_DIR}/{id}.k7x");
    let written = if is_chain_project(fs, dir) {
        Some(create_chain_lane(fs, dir, id, &body, new_id)?)
    } else {
        None
    };
    if written.is_none() {
        if fs.exists(&join(&[dir, &rel])) {
            return fail(format!("track '{id}' already exists ({rel})"));
        }
        mkdirs(fs, &join(&[dir, NODES_DIR]))?;
        ws_write_text(fs, &join(&[dir, &rel]), &body)?;
    }
    if let Some(mut order) = track_order(fs, dir)?
        && let Some(project) = entity_file_for(fs, dir, "", "project")
    {
        order.push(Value::Str(id.to_owned()));
        set_file_value(
            fs,
            &project.path,
            &[PathSeg::Key("trackOrder".to_owned())],
            &Value::List(order),
        )?;
    }
    Ok(written.unwrap_or_else(|| vec![rel]))
}

/// Create a PICTURE lane, listed in `trackOrder` above the picture lanes there are, so it draws in front.
///
/// # Errors
/// What `create_track_file` and the reads fail with.
pub fn create_picture_lane_file(fs: &mut dyn Fs, env: &Env, dir: &str, id: &str, new_id: NewId) -> Res<Vec<String>> {
    let written = create_track_file(fs, dir, id, PICTURE_TRACK_REF, new_id)?;
    let Some(project) = entity_file_for(fs, dir, "", "project") else {
        return Ok(written);
    };
    let resolved = resolve_all(fs, env, dir)?;
    let lanes = lanes_of(&resolved);
    let pictures: HashSet<&str> = lanes.picture.keys().copied().collect();
    let stated: Vec<String> = match track_order(fs, dir)? {
        Some(l) => l.iter().map(|v| text_of(Some(v))).collect(),
        None => vec![],
    };
    // The rows as they draw now, so stating the order for the first time moves nothing else.
    let names: Vec<&str> = lanes
        .sound
        .keys()
        .copied()
        .chain(lanes.picture.keys().copied())
        .filter(|l| *l != id)
        .collect();
    let mut rows: Vec<String> = order_tracks(&names, &stated)
        .into_iter()
        .map(|i| names[i].to_owned())
        .collect();
    let at = rows
        .iter()
        .position(|l| pictures.contains(l.as_str()))
        .unwrap_or(rows.len());
    rows.insert(at, id.to_owned());
    let order = Value::List(rows.into_iter().map(Value::Str).collect());
    set_file_value(fs, &project.path, &[PathSeg::Key("trackOrder".to_owned())], &order)?;
    let mut out = written;
    out.push(project.rel.trim_start_matches('/').to_owned());
    Ok(out)
}

/// A new picture lane WITH its strip: every place a picture lane is made goes through here.
///
/// # Errors
/// What `create_picture_lane_file` fails with; a strip that cannot be wired is said in the second field.
pub fn create_picture_lane(
    fs: &mut dyn Fs,
    env: &Env,
    dir: &str,
    id: &str,
    new_id: NewId,
) -> Res<(Vec<String>, String)> {
    let mut written = create_picture_lane_file(fs, env, dir, id, new_id)?;
    let (strip, no_strip) = strip_new_lane(fs, env, dir, id, new_id);
    for w in strip {
        if !written.contains(&w) {
            written.push(w);
        }
    }
    Ok((written, no_strip))
}
