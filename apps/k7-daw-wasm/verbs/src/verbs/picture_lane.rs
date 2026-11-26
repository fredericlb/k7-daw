// picture_lane.rs — `picture lane rename` and `picture lane delete`, a picture lane's `track rename` and `track delete`.
// A reader would get wrong how many places name a picture lane: its file, `trackOrder`, `soloed:`, the graph, a sounding lane's `video_source`, a timeline's lane.

use super::picture_shared::{LaneAt, lane_file, taken_in};
use super::shared::{Done, join, split_words};
use super::track::{Taken, follow_strip_name, lane_own_nodes, own_lane_strip, surfaces_naming};
use super::words::is_name;
use crate::host::{Host, Thrown};
use k7x::edit::{PathSeg, Value};
use song::lanes::lanes_of;
use song::routing::split_port;
use song_edit::assembly_edit::{drop_node_from_assemblies, rename_node_in_assemblies};
use song_edit::chain_graph_edit::edit_project_graph;
use song_edit::chain_item_edit::{is_chain_project, lane_file_for, timeline_lane_of};
use song_edit::graph_edit::{delete_chain_node, read_graph_value, rename_chain_node};
use song_edit::graph_file::remove_node;
use song_edit::loader::{load_modules, resolve_all};
use song_edit::module_io::{entity_file_for, entity_files};
use song_edit::project_root::load_project_root;
use song_edit::workspace_doc::{EditOpts, ws_data, ws_edit};
use song_edit::workspace_file::ws_exists;
use song_edit::yaml_edit::{delete_file_value, set_file_value};

const NAME_HINT: &str = "letters, digits, '.', '_' and '-' only";

/// `lane#audio`, the half of a picture lane that carries its sound.
fn sound_ref(lane: &str) -> String {
    format!("{lane}#audio")
}

/// The song lanes playing `lane`'s sound (`video_source`), by name.
fn sounding_lanes(h: &Host, lane: &str) -> Result<Vec<String>, Thrown> {
    let resolved = resolve_all(&*h.fs, &h.env, &h.song_path)?;
    Ok(lanes_of(&resolved)
        .sound
        .iter()
        .filter(|(_, l)| l.video_source == Some(lane))
        .map(|(n, _)| (*n).to_owned())
        .collect())
}

/// Every timeline's picture lane of name `lane`: its timeline, index in `pictures:` and the names that timeline holds.
/// A timeline's file holding picture lane `lane`: its name, path, index in `pictures:` and the names the file holds.
type Named = (String, String, usize, Vec<String>);

fn timeline_lanes_named(h: &Host, lane: &str) -> Result<Vec<Named>, Thrown> {
    let folder = join(&h.song_path, "timelines");
    let mut out = Vec::new();
    for (id, file) in entity_files(&*h.fs, &folder, "timelines") {
        let data = ws_data(&*h.fs, &file.path)?;
        let at = match data.get("pictures") {
            Some(Value::List(l)) => l
                .iter()
                .position(|p| matches!(p.get("lane"), Some(Value::Str(s)) if s == lane)),
            _ => None,
        };
        if let Some(at) = at {
            out.push((id, file.path.clone(), at, taken_in(&data)));
        }
    }
    Ok(out)
}

/// A name the song lane can take: free among picture lanes, song lanes and modules alike.
fn name_refusal(h: &Host, to: &str) -> Result<Option<String>, Thrown> {
    if !is_name(to) {
        return Ok(Some(format!("invalid lane name '{to}' — {NAME_HINT}")));
    }
    let dir = &h.song_path;
    let taken = lane_file_for(&*h.fs, dir, to).is_some()
        || ws_exists(&*h.fs, &join(dir, &format!("nodes/{to}.k7x")))
        || load_modules(&*h.fs, &h.env, dir)?.contains_key(to);
    Ok(taken.then(|| format!("'{to}' is already a track or a module in this project")))
}

/// `trackOrder`, `soloed:` and `highlighted:` with `lane` (and its `#audio` half) renamed to `to`, or dropped when `to` is None.
fn follow_in_project(h: &mut Host, lane: &str, to: Option<&str>) -> Result<(), Thrown> {
    let dir = h.song_path.clone();
    let Some(file) = entity_file_for(&*h.fs, &dir, "", "project") else {
        return Ok(());
    };
    let root = load_project_root(&*h.fs, &dir, false)?;
    for k in ["trackOrder", "soloed", "highlighted"] {
        let Some(Value::List(list)) = root.data.get(k) else {
            continue;
        };
        let ours = |x: &Value| matches!(x, Value::Str(s) if split_port(s).0 == lane);
        if !list.iter().any(ours) {
            continue;
        }
        let next: Vec<Value> = match to {
            Some(to) => list
                .iter()
                .map(|x| match x {
                    Value::Str(s) if ours(x) => Value::Str(match split_port(s).1 {
                        Some(p) => format!("{to}#{p}"),
                        None => to.to_owned(),
                    }),
                    o => o.clone(),
                })
                .collect(),
            None => list.iter().filter(|x| !ours(x)).cloned().collect(),
        };
        let path = [PathSeg::Key(k.to_owned())];
        if next.is_empty() {
            delete_file_value(h.fs, &file.path, &path)?;
        } else {
            set_file_value(h.fs, &file.path, &path, &Value::List(next))?;
        }
    }
    Ok(())
}

fn rename_timeline_lane(h: &mut Host, lane: &str, to: &str) -> Done {
    let Some(found) = lane_file(h, lane)? else {
        return Ok(());
    };
    let Some(timeline) = found.timeline.clone() else {
        return Ok(());
    };
    let data = ws_data(&*h.fs, &found.abs)?;
    if !is_name(to) || taken_in(&data).iter().any(|t| t == to) {
        h.refuse([format!(
            "'{to}' cannot name a lane of '{timeline}' — taken, or not a name"
        )]);
        return Ok(());
    }
    rename_in_timeline(h, &found, to)?;
    h.log([format!(
        "{lane} → {timeline}>{to} — it now draws on the song's picture lane '{to}'"
    )]);
    h.reload();
    Ok(())
}

fn rename_in_timeline(h: &mut Host, found: &LaneAt, to: &str) -> Result<(), Thrown> {
    let mut at = found.base.clone();
    at.push(PathSeg::Key("lane".to_owned()));
    let opts = EditOpts {
        create: false,
        raw: false,
    };
    ws_edit(h.fs, &found.abs, opts, |doc| {
        Ok(doc.set_in(&at, &Value::Str(to.to_owned()), k7x::edit::WriteOptions::default())?)
    })?;
    Ok(())
}

/// `picture lane rename <lane> <name>` — the file and every place that names it.
pub fn lane_rename(a: &str, h: &mut Host) -> Done {
    let w = split_words(a);
    let (Some(lane), Some(to)) = (w.first().copied(), w.get(1).copied()) else {
        h.refuse(["usage: picture lane rename <lane> <name>  (e.g. picture lane rename layer-1 broll)".to_owned()]);
        return Ok(());
    };
    if timeline_lane_of(lane).is_some() {
        return rename_timeline_lane(h, lane, to);
    }
    if lane_file(h, lane)?.is_none() {
        return Ok(());
    }
    if to == lane {
        h.refuse([format!("'{lane}' is already called that")]);
        return Ok(());
    }
    if let Some(why) = name_refusal(h, to)? {
        h.refuse([why]);
        return Ok(());
    }
    let mapped = surfaces_naming(h, lane)?;
    if !mapped.is_empty() {
        h.refuse([format!(
            "'{lane}' is named by {} {} — a mapping is not rewritten, so the lane keeps its name",
            if mapped.len() > 1 { "surfaces" } else { "surface" },
            mapped.join(", ")
        )]);
        return Ok(());
    }
    let dir = h.song_path.clone();
    if let Err(e) = rename(h, &dir, lane, to) {
        h.refuse([format!("picture lane rename: {e}")]);
        return Ok(());
    }
    h.reload();
    Ok(())
}

fn rename(h: &mut Host, dir: &str, lane: &str, to: &str) -> Result<(), song_edit::Error> {
    let taken = Taken::read(&*h.fs, dir)?;
    let mixer = if is_chain_project(&*h.fs, dir) {
        None
    } else {
        own_lane_strip(h, lane)?
    };
    let (moved, rewired) = rename_chain_node(h.fs, &h.env, dir, lane, to)?;
    let mut notes = moved;
    if !rewired.is_empty() {
        notes.push(format!("rewired {}", rewired.join(", ")));
    }
    if !rename_node_in_assemblies(h.fs, dir, lane, to)?.is_empty() {
        notes.push("assemblies".to_owned());
    }
    if let Some(m) = mixer
        && let Some(followed) = follow_strip_name(h, &m, to, &taken)?
    {
        notes.push(followed);
    }
    follow_in_project(h, lane, Some(to)).map_err(|t| song_edit::error::Error::Msg(t.0))?;
    let sounds = sounding_lanes(h, lane).map_err(|t| song_edit::error::Error::Msg(t.0))?;
    for sound in &sounds {
        if let Some(f) = lane_file_for(&*h.fs, dir, sound) {
            set_file_value(
                h.fs,
                &f.path,
                &[PathSeg::Key("video_source".to_owned())],
                &Value::Str(to.to_owned()),
            )?;
        }
        notes.push(format!("{sound}'s sound"));
    }
    for (timeline, abs, at, names) in timeline_lanes_named(h, lane).map_err(|t| song_edit::error::Error::Msg(t.0))? {
        if names.iter().any(|n| n == to) {
            notes.push(format!("{timeline}>{lane} kept, '{to}' being taken there"));
            continue;
        }
        let path = [
            PathSeg::Key("pictures".to_owned()),
            PathSeg::Index(at),
            PathSeg::Key("lane".to_owned()),
        ];
        let opts = EditOpts {
            create: false,
            raw: false,
        };
        ws_edit(h.fs, &abs, opts, |doc| {
            Ok(doc.set_in(&path, &Value::Str(to.to_owned()), k7x::edit::WriteOptions::default())?)
        })?;
        notes.push(format!("{timeline}>{to}"));
    }
    let tail = if notes.is_empty() {
        String::new()
    } else {
        format!(" ({})", notes.join(", "))
    };
    h.log([format!("{lane} → {to}{tail}")]);
    Ok(())
}

fn graph_mentions(h: &Host, name: &str) -> bool {
    if is_chain_project(&*h.fs, &h.song_path) {
        return false;
    }
    let Ok(g) = read_graph_value(&*h.fs, &h.song_path) else {
        return false;
    };
    let is = |v: &Value| matches!(v, Value::Str(s) if s == name);
    let in_modules = matches!(g.get("modules"), Some(Value::Map(m))
        if m.iter().any(|(k, d)| k == name || matches!(d, Value::List(l) if l.iter().any(is))));
    in_modules || matches!(g.get("bypassed"), Some(Value::List(l)) if l.iter().any(is))
}

/// `picture lane delete <lane>` — its file and every place that names it; its clips stay on disk.
pub fn lane_delete(a: &str, h: &mut Host) -> Done {
    let Some(lane) = split_words(a).first().copied() else {
        h.refuse(["usage: picture lane delete <lane>".to_owned()]);
        return Ok(());
    };
    let tl = timeline_lane_of(lane).map(|(t, l)| (t.to_owned(), l.to_owned()));
    let Some(found) = lane_file(h, lane)? else {
        return Ok(());
    };
    if let Some((timeline, tl_lane)) = tl {
        let opts = EditOpts {
            create: false,
            raw: false,
        };
        ws_edit(h.fs, &found.abs, opts, |doc| {
            doc.delete_in(&found.base, false)?;
            Ok(())
        })?;
        h.log([format!("{timeline}: - picture lane {tl_lane}")]);
        h.reload();
        return Ok(());
    }
    let mapped = surfaces_naming(h, lane)?;
    if !mapped.is_empty() {
        h.refuse([format!(
            "'{lane}' is named by {} {} — unmap it there first",
            if mapped.len() > 1 { "surfaces" } else { "surface" },
            mapped.join(", ")
        )]);
        return Ok(());
    }
    let sounds = sounding_lanes(h, lane)?;
    if !sounds.is_empty() {
        h.refuse([format!(
            "{} plays '{lane}''s sound — delete it first (track delete {})",
            sounds.join(", "),
            sounds[0]
        )]);
        return Ok(());
    }
    match delete(h, lane) {
        Ok(notes) => {
            let orphans: Vec<String> = timeline_lanes_named(h, lane)?
                .into_iter()
                .map(|(t, ..)| format!("{t}>{lane}"))
                .collect();
            let tail = if notes.is_empty() {
                String::new()
            } else {
                format!(" ({})", notes.join(", "))
            };
            let mut lines = vec![format!("deleted picture lane {lane}{tail} — its clips stay on disk")];
            if !orphans.is_empty() {
                lines.push(format!(
                    "{} now draw(s) nowhere — 'picture lane new {lane}' brings a lane back",
                    orphans.join(", ")
                ));
            }
            h.log(lines);
        }
        Err(e) => {
            h.refuse([format!("picture lane delete: {e}")]);
            return Ok(());
        }
    }
    h.reload();
    Ok(())
}

fn delete(h: &mut Host, lane: &str) -> Result<Vec<String>, song_edit::Error> {
    let dir = h.song_path.clone();
    let mut notes = Vec::new();
    for node in lane_own_nodes(h, lane)? {
        let (_, removed) = delete_chain_node(h.fs, &h.env, &dir, &node)?;
        notes.extend(removed);
        drop_node_from_assemblies(h.fs, &dir, &node)?;
    }
    let sound = sound_ref(lane);
    if graph_mentions(h, &sound) {
        edit_project_graph(h.fs, &h.env, &dir, |doc| remove_node(doc, &sound).map(|_| ()))?;
    }
    follow_in_project(h, lane, None).map_err(|t| song_edit::error::Error::Msg(t.0))?;
    Ok(notes)
}
