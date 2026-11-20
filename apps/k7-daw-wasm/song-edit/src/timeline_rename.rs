// timeline_rename.rs — a timeline renamed: its file, and every event placing it, on the song's lanes and inside other timelines.
// A reader would get wrong that a placement can sit in a lane's ALTERNATIVE as well as its playing `events`: every `events` list is rewritten.

use crate::chain_item_edit::lane_folder;
use crate::error::{Res, fail};
use crate::fs::Fs;
use crate::paths::join;
use crate::workspace_doc::{EditOpts, ws_data, ws_edit};
use crate::workspace_file::{is_doc_name, ws_exists, ws_list, ws_rename};
use k7x::edit::{PathSeg, Value, WriteOptions};

fn key(s: &str) -> PathSeg {
    PathSeg::Key(s.to_owned())
}

/// The paths of every `ref` in an `events` list of `data` that names `name`.
fn ref_paths(data: &Value, name: &str, at: &mut Vec<PathSeg>, out: &mut Vec<Vec<PathSeg>>) {
    match data {
        Value::List(l) => {
            for (i, v) in l.iter().enumerate() {
                at.push(PathSeg::Index(i));
                ref_paths(v, name, at, out);
                at.pop();
            }
        }
        Value::Map(m) => {
            for (k, v) in m {
                at.push(key(k));
                if k == "events"
                    && let Value::List(events) = v
                {
                    for (i, e) in events.iter().enumerate() {
                        if matches!(e.get("ref"), Some(Value::Str(r)) if r == name) {
                            let mut p = at.clone();
                            p.extend([PathSeg::Index(i), key("ref")]);
                            out.push(p);
                        }
                    }
                }
                ref_paths(v, name, at, out);
                at.pop();
            }
        }
        _ => {}
    }
}

/// Rename timeline `from` to `to`; how many placements were repointed.
///
/// # Errors
/// Either name being wrong, a file that does not read, or the file system's.
pub fn rename_timeline(fs: &mut dyn Fs, dir: &str, from: &str, to: &str) -> Res<usize> {
    let folder = join(&[dir, "timelines"]);
    let src = join(&[&folder, &format!("{from}.k7x")]);
    let dst = join(&[&folder, &format!("{to}.k7x")]);
    if !ws_exists(fs, &src) {
        return fail(format!("no timeline \"{from}\""));
    }
    if ws_exists(fs, &dst) {
        return fail(format!("timeline '{to}' already exists"));
    }
    let lanes = lane_folder(fs, dir).to_owned();
    let mut files = Vec::new();
    for sub in [lanes.as_str(), "timelines"] {
        let at = join(&[dir, sub]);
        if ws_exists(fs, &at) {
            files.extend(
                ws_list(fs, &at)?
                    .into_iter()
                    .filter(|f| is_doc_name(f))
                    .map(|f| join(&[&at, &f])),
            );
        }
    }
    let mut repointed = 0;
    for file in files {
        let mut paths = Vec::new();
        ref_paths(&ws_data(fs, &file)?, from, &mut Vec::new(), &mut paths);
        if paths.is_empty() {
            continue;
        }
        ws_edit(fs, &file, EditOpts::default(), |doc| {
            for p in &paths {
                doc.set_in(p, &Value::Str(to.to_owned()), WriteOptions::default())?;
            }
            Ok(())
        })?;
        repointed += paths.len();
    }
    ws_rename(fs, &src, &dst)?;
    Ok(repointed)
}
