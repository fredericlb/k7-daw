// picture_lanes_migrate.rs — a project still holding `video_tracks/` rewritten so each picture lane is a lane node.
// A reader would get wrong that every new text is computed BEFORE anything is written: a lane that will not read leaves the project as it was.

use crate::chain_item_edit::{CHAINS_DIR, is_chain_project, lane_folder};
use crate::confine::Env;
use crate::error::{Res, fail, io_error};
use crate::format_level::{FORMAT_KEY, PICTURE_LANE_LEVEL};
use crate::format_stamp::declared_level;
use crate::fs::Fs;
use crate::graph_schema::GRAPH_BASE;
use crate::ids::NewId;
use crate::js_text::{js_string, js_to_number};
use crate::loader::resolve_all;
use crate::module_io::{entity_file_for, entity_files};
use crate::paths::join;
use crate::workspace_doc::{ws_text, ws_write_text};
use indexmap::IndexMap;
use k7x::edit::{Document, PathSeg, Scope, Value, WriteOptions};
use song::doc::order_tracks;
use song::lanes::lanes_of;
use std::collections::HashSet;

const VIDEO_DIR: &str = "video_tracks";
/// The shot's three keys that take the audio event's spelling.
const RENAMED: [(&str, &str); 3] = [("source_from", "from"), ("fade_in", "fadeIn"), ("fade_out", "fadeOut")];

/// What a migration did: each lane's old id and new one, the files it wrote or removed, what it could not carry.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct PictureMigration {
    pub lanes: IndexMap<String, String>,
    pub files: Vec<String>,
    pub warnings: Vec<String>,
}

/// A `Map` of path to text: a path set twice keeps its first place and its last text.
#[derive(Default)]
pub(crate) struct Writes(pub(crate) Vec<(String, String)>);

impl Writes {
    pub(crate) fn set(&mut self, path: String, text: String) {
        match self.0.iter_mut().find(|(p, _)| *p == path) {
            Some(slot) => slot.1 = text,
            None => self.0.push((path, text)),
        }
    }
}

fn key(s: &str) -> PathSeg {
    PathSeg::Key(s.to_owned())
}

pub(crate) fn list_of(v: Option<&Value>) -> Vec<Value> {
    match v {
        Some(Value::List(l)) => l.clone(),
        _ => vec![],
    }
}

pub(crate) fn strings(v: Option<&Value>) -> Vec<String> {
    list_of(v).iter().map(js_string).collect()
}

fn inline() -> WriteOptions {
    WriteOptions { inline: true }
}

/// Rename the shot keys of every event of the list at `path`, in place, comments kept.
fn rename_shot_keys(doc: &mut Scope, path: &[PathSeg]) -> Res<()> {
    let events = list_of(doc.get_in(path)?.as_ref());
    for (i, ev) in events.iter().enumerate() {
        if !matches!(ev, Value::Map(_)) {
            continue;
        }
        for (from, to) in RENAMED {
            if ev.get(from).is_some() {
                let mut at = path.to_vec();
                at.push(PathSeg::Index(i));
                at.push(key(from));
                doc.rename_key(&at, to)?;
            }
        }
    }
    Ok(())
}

/// `extends("@meta/picture-track")` as the first line under the file's header comment.
fn with_extends(text: &str) -> String {
    let mut lines: Vec<&str> = text.split('\n').collect();
    let mut at = 0;
    while at < lines.len() && lines[at].trim_start().starts_with("//") {
        at += 1;
    }
    lines.insert(at, "extends(\"@meta/picture-track\")");
    lines.join("\n")
}

fn number_or(v: Option<&Value>, fallback: f64) -> f64 {
    match v {
        None | Some(Value::Null) => fallback,
        other => js_to_number(other),
    }
}

/// One `video_tracks/` file's text as a lane node's: what it says, in the lane's words.
fn lane_text(text: &str, viewport: &[f64], warnings: &mut Vec<String>, id: &str) -> Res<String> {
    let mut doc = Document::new(text);
    let data = doc.to_js()?;
    let mut s = doc.at(&[]);
    rename_shot_keys(&mut s, &[key("events")])?;
    let rect: Vec<Option<&Value>> = ["x", "y", "w", "h"].iter().map(|k| data.get(k)).collect();
    if rect.iter().any(Option::is_some) {
        for k in ["x", "y", "w", "h"] {
            s.delete_in(&[key(k)], false)?;
        }
        let frame = [
            number_or(rect[0], 0.0),
            number_or(rect[1], 0.0),
            number_or(rect[2], viewport.first().copied().unwrap_or(0.0)),
            number_or(rect[3], viewport.get(1).copied().unwrap_or(0.0)),
        ];
        let list = Value::List(frame.iter().map(|n| Value::Num(*n)).collect());
        s.set_in(&[key("frame")], &list, inline())?;
    }
    // The mute moves to the graph; the name is the id, as an audio lane's is.
    s.delete_in(&[key("mute")], false)?;
    if let Some(Value::Str(name)) = data.get("name")
        && name != id
    {
        warnings.push(format!(
            "{id}: its name \"{name}\" was dropped — a lane is named by its id"
        ));
    }
    s.delete_in(&[key("name")], false)?;
    Ok(with_extends(doc.text()))
}

/// Every `.k7x` under `dir`, folders included: a chain project's chains may be folder chains.
fn k7x_under(fs: &dyn Fs, dir: &str) -> Res<Vec<String>> {
    if !fs.exists(dir) {
        return Ok(vec![]);
    }
    let mut out = vec![];
    for e in fs.list(dir).map_err(|e| io_error(&e, "scandir", dir))? {
        let abs = join(&[dir, &e.name]);
        if e.kind == song::Kind::Dir {
            out.extend(k7x_under(fs, &abs)?);
        } else if e.name.ends_with(".k7x") {
            out.push(abs);
        }
    }
    Ok(out)
}

/// The paths of every object under `v` carrying a `video_track` item key: a chain project's picture lanes.
fn picture_items(v: &Value, path: &mut Vec<PathSeg>, out: &mut Vec<Vec<PathSeg>>) {
    match v {
        Value::List(l) => {
            for (i, x) in l.iter().enumerate() {
                path.push(PathSeg::Index(i));
                picture_items(x, path, out);
                path.pop();
            }
        }
        Value::Map(m) => {
            if matches!(v.get("video_track"), Some(Value::Str(_))) {
                out.push(path.clone());
            }
            for (k, x) in m {
                path.push(key(k));
                picture_items(x, path, out);
                path.pop();
            }
        }
        _ => {}
    }
}

/// The audio lanes in the order the rows draw them now: what `trackOrder` must keep saying.
fn audio_order(fs: &dyn Fs, env: &Env, dir: &str, stated: Option<&Value>) -> Vec<String> {
    let ids: Vec<String> = match resolve_all(fs, env, dir) {
        Ok(modules) => lanes_of(&modules).sound.keys().map(|k| (*k).to_owned()).collect(),
        Err(_) => {
            let sub = lane_folder(fs, dir);
            entity_files(fs, &join(&[dir, sub]), sub)
                .into_iter()
                .map(|(id, _)| id)
                .collect()
        }
    };
    let names: Vec<&str> = ids.iter().map(String::as_str).collect();
    let order = match stated {
        Some(Value::List(l)) => l.iter().map(js_string).collect(),
        _ => vec![],
    };
    order_tracks(&names, &order)
        .into_iter()
        .filter_map(|i| ids.get(i).cloned())
        .collect()
}

fn str_list(items: &[String]) -> Value {
    Value::List(items.iter().cloned().map(Value::Str).collect())
}

/// Migrate the project at `dir`, or None when it holds no `video_tracks/`.
///
/// # Errors
/// A file that will not read: nothing is written then.
pub fn migrate_picture_lanes(fs: &mut dyn Fs, env: &Env, dir: &str, new_id: NewId) -> Res<Option<PictureMigration>> {
    let video_dir = join(&[dir, VIDEO_DIR]);
    if !fs.exists(&video_dir) {
        return Ok(None);
    }
    let mut report = PictureMigration::default();
    let mut writes = Writes::default();
    let mut removes: Vec<String> = vec![];

    let Some(root_file) = entity_file_for(fs, dir, "", "project") else {
        return fail(format!("{dir}: no project.k7x to migrate"));
    };
    let mut root = Document::new(&ws_text(fs, &root_file.path)?);
    let root_data = root.to_js()?;
    let proj = root_data.get("project");
    let viewport: Vec<f64> = list_of(proj.and_then(|p| p.get("viewport")))
        .iter()
        .map(|v| js_to_number(Some(v)))
        .collect();
    let layers = strings(proj.and_then(|p| p.get("layers")));

    // 1. The lanes, each under an id no audio lane holds.
    let sub = lane_folder(fs, dir);
    let mut taken: HashSet<String> = entity_files(fs, &join(&[dir, sub]), sub)
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    let mut muted: Vec<String> = vec![];
    for (id, file) in entity_files(fs, &video_dir, VIDEO_DIR) {
        let mut to = id.clone();
        let mut n = 1;
        while taken.contains(&to) {
            to = if n == 1 {
                format!("{id}-picture")
            } else {
                format!("{id}-picture-{n}")
            };
            n += 1;
        }
        if to != id {
            report.warnings.push(format!(
                "{id}: an audio lane holds that id, so the picture lane is now '{to}' — a graph entry naming '{id}' stays the audio lane's"
            ));
        }
        taken.insert(to.clone());
        report.lanes.insert(id.clone(), to.clone());
        let text = ws_text(fs, &file.path)?;
        if matches!(Document::new(&text).to_js()?.get("mute"), Some(Value::Bool(true))) {
            muted.push(to.clone());
        }
        let body = lane_text(&text, &viewport, &mut report.warnings, &id)?;
        writes.set(join(&[dir, sub, &format!("{to}.k7x")]), body);
        removes.push(file.path);
    }
    let renamed = |id: &str| report.lanes.get(id).cloned().unwrap_or_else(|| id.to_owned());

    // 2. The graph: a muted lane is bypassed, and a chain project's picture item becomes a lane item.
    let mut warnings_after: Vec<String> = vec![];
    if is_chain_project(fs, dir) {
        let mut held: HashSet<String> = HashSet::new();
        for abs in k7x_under(fs, &join(&[dir, CHAINS_DIR]))? {
            let mut doc = Document::new(&ws_text(fs, &abs)?);
            let mut items = vec![];
            picture_items(&doc.to_js()?, &mut vec![], &mut items);
            if items.is_empty() {
                continue;
            }
            let mut s = doc.at(&[]);
            for path in items {
                let at = |k: &str| {
                    let mut p = path.clone();
                    p.push(key(k));
                    p
                };
                let was = s
                    .get_in(&at("video_track"))?
                    .as_ref()
                    .map(js_string)
                    .unwrap_or_default();
                let lane = renamed(&was);
                s.rename_key(&at("video_track"), "track")?;
                s.set_in(&at("track"), &Value::Str(lane.clone()), WriteOptions::default())?;
                if muted.contains(&lane) {
                    s.set_in(&at("bypass"), &Value::Bool(true), WriteOptions::default())?;
                }
                held.insert(lane);
            }
            writes.set(abs, doc.text().to_owned());
        }
        // A lane exists in a chain project through an item: one no chain holds gets a chain of its own.
        for lane in report.lanes.values() {
            if held.contains(lane) {
                continue;
            }
            let mut name = lane.clone();
            let mut n = 2;
            while fs.exists(&join(&[dir, CHAINS_DIR, &format!("{name}.k7x")])) {
                name = format!("{lane}-{n}");
                n += 1;
            }
            let bypass = if muted.contains(lane) { ".bypass(true)" } else { "" };
            let text = format!("start(true)\nsteps([id(\"{}\").track(\"{lane}\"){bypass}])\n", new_id());
            writes.set(join(&[dir, CHAINS_DIR, &format!("{name}.k7x")]), text);
        }
    } else if !muted.is_empty() {
        if let Some(graph) = entity_file_for(fs, dir, "", GRAPH_BASE) {
            let mut doc = Document::new(&ws_text(fs, &graph.path)?);
            let mut was = strings(doc.to_js()?.get("bypassed"));
            for m in &muted {
                if !was.contains(m) {
                    was.push(m.clone());
                }
            }
            doc.at(&[]).set_in(&[key("bypassed")], &str_list(&was), inline())?;
            writes.set(graph.path, doc.text().to_owned());
        } else {
            let own = if muted.len() > 1 { "their" } else { "its" };
            warnings_after.push(format!("no graph.k7x, so {} lost {own} mute", muted.join(", ")));
        }
    }
    report.warnings.extend(warnings_after);

    // 3. The root: one order for every lane, the picture rows below the audio, top layer first.
    let mut rows: Vec<String> = layers.iter().rev().map(|l| renamed(l)).collect();
    for id in report.lanes.values() {
        if !rows.contains(id) {
            rows.push(id.clone());
        }
    }
    let mut order = audio_order(fs, env, dir, root_data.get("trackOrder"));
    order.extend(rows);
    let mut r = root.at(&[]);
    r.set_in(&[key("trackOrder")], &str_list(&order), inline())?;
    r.delete_in(&[key("project"), key("layers")], false)?;
    let soloed = strings(root_data.get("soloed"));
    if soloed.iter().any(|id| renamed(id) != *id) {
        let next: Vec<String> = soloed.iter().map(|id| renamed(id)).collect();
        r.set_in(&[key("soloed")], &str_list(&next), inline())?;
    }
    if declared_level(fs, &root_file.path)? < PICTURE_LANE_LEVEL {
        r.set_in(
            &[key(FORMAT_KEY)],
            &Value::Num(f64::from(PICTURE_LANE_LEVEL)),
            WriteOptions::default(),
        )?;
    }
    writes.set(root_file.path.clone(), root.text().to_owned());

    // 4. A sounding lane follows its picture lane's new id.
    for (_, file) in entity_files(fs, &join(&[dir, sub]), sub) {
        let mut doc = Document::new(&ws_text(fs, &file.path)?);
        if let Some(Value::Str(src)) = doc.to_js()?.get("video_source")
            && renamed(src) != *src
        {
            let to = Value::Str(renamed(src));
            doc.at(&[])
                .set_in(&[key("video_source")], &to, WriteOptions::default())?;
            writes.set(file.path, doc.text().to_owned());
        }
    }

    // 5. Each timeline's picture lanes keep their own list, in the lane's words.
    for (_, file) in entity_files(fs, &join(&[dir, "timelines"]), "timelines") {
        let mut doc = Document::new(&ws_text(fs, &file.path)?);
        let pics = list_of(doc.to_js()?.get("pictures"));
        if pics.is_empty() {
            continue;
        }
        let mut s = doc.at(&[]);
        for (i, p) in pics.iter().enumerate() {
            if !matches!(p, Value::Map(_)) {
                continue;
            }
            let base = [key("pictures"), PathSeg::Index(i)];
            let mut events = base.to_vec();
            events.push(key("events"));
            rename_shot_keys(&mut s, &events)?;
            let was = p.get("lane").map(js_string).unwrap_or_else(|| "undefined".to_owned());
            let lane = renamed(&was);
            if p.get("lane") != Some(&Value::Str(lane.clone())) {
                let mut at = base.to_vec();
                at.push(key("lane"));
                s.set_in(&at, &Value::Str(lane), WriteOptions::default())?;
            }
        }
        writes.set(file.path, doc.text().to_owned());
    }

    // Everything is computed: write, then remove.
    for (abs, text) in &writes.0 {
        ws_write_text(fs, abs, text)?;
    }
    for abs in &removes {
        fs.remove(abs).map_err(|e| io_error(&e, "rm", abs))?;
    }
    if fs.list(&video_dir).map(|l| l.is_empty()).unwrap_or(false) {
        fs.remove_tree(&video_dir).map_err(|e| io_error(&e, "rm", &video_dir))?;
    }
    report.files = writes.0.into_iter().map(|(p, _)| p).chain(removes).collect();
    Ok(Some(report))
}
