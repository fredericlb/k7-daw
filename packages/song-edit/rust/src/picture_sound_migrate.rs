// picture_sound_migrate.rs — a node project's `video_source:` lanes folded into the picture lanes they sound, as `<lane>#audio`.
// A reader would get wrong that a picture lane sounded by TWO lanes is left as it is: merging two strips is a choice no migration makes.

use crate::assembly_edit::rename_node_in_assemblies;
use crate::chain_item_edit::{is_chain_project, lane_file_for};
use crate::confine::Env;
use crate::error::{Res, io_error};
use crate::format_level::PICTURE_SOUND_LEVEL;
use crate::format_stamp::stamp_format_level;
use crate::fs::Fs;
use crate::graph_schema::GRAPH_BASE;
use crate::js_text::js_string;
use crate::loader::resolve_all;
use crate::module_io::{entity_file_for, entity_files};
use crate::paths::join;
use crate::picture_lanes_migrate::{Writes, list_of, strings};
use crate::workspace_doc::{ws_data, ws_text, ws_write_text};
use indexmap::IndexMap;
use k7x::edit::{Document, PathSeg, Value, WriteOptions};
use song::lanes::lanes_of;
use song::routing::split_port;
use std::collections::HashSet;

/// What a fold did: each sounding lane and the half it became, and what it left alone.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct SoundFold {
    pub folded: IndexMap<String, String>,
    pub warnings: Vec<String>,
}

fn key(s: &str) -> PathSeg {
    PathSeg::Key(s.to_owned())
}

fn str_list(items: Vec<String>) -> Value {
    Value::List(items.into_iter().map(Value::Str).collect())
}

/// JS truthiness of a clip's `sound`.
fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null | Value::Bool(false)) => false,
        Some(Value::Num(n)) => *n != 0.0 && !n.is_nan(),
        Some(Value::Str(s)) => !s.is_empty(),
        Some(_) => true,
    }
}

/// The graph's text after the fold: every ref to a folded lane names its picture lane's half.
fn folded_graph(data: &Value, folded: &IndexMap<String, String>) -> Value {
    let renamed = |n: &str| folded.get(n).cloned().unwrap_or_else(|| n.to_owned());
    let reference = |r: &Value| -> Value {
        let text = js_string(r);
        let (node, _) = split_port(&text);
        match folded.get(node) {
            Some(half) => Value::Str(half.clone()),
            None => r.clone(),
        }
    };
    let Value::Map(entries) = data else { return data.clone() };
    let next = entries
        .iter()
        .map(|(k, v)| {
            let v = match (k.as_str(), v) {
                ("modules", Value::Map(m)) => Value::Map(
                    m.iter()
                        .map(|(name, dests)| {
                            let dests = list_of(Some(dests)).iter().map(reference).collect();
                            (js_string(&reference(&Value::Str(name.clone()))), Value::List(dests))
                        })
                        .collect(),
                ),
                ("bypassed" | "bus_only", Value::List(l)) => {
                    Value::List(l.iter().map(|x| Value::Str(renamed(&js_string(x)))).collect())
                }
                ("positions", Value::Map(m)) => Value::Map(
                    m.iter()
                        .filter(|(n, _)| !folded.contains_key(n.as_str()))
                        .cloned()
                        .collect(),
                ),
                _ => v.clone(),
            };
            (k.clone(), v)
        })
        .collect();
    Value::Map(next)
}

/// Fold the project at `dir`, then stamp it if a picture lane sounds on its own; None when it is a chain project.
///
/// # Errors
/// A file that will not read or write.
pub fn fold_picture_sounds(fs: &mut dyn Fs, env: &Env, dir: &str) -> Res<Option<SoundFold>> {
    if is_chain_project(fs, dir) {
        return Ok(None);
    }
    let modules = resolve_all(fs, env, dir)?;
    let lanes = lanes_of(&modules);
    let pictures = &lanes.picture;
    let sounding: Vec<(&str, &str)> = lanes
        .sound
        .iter()
        .filter_map(|(id, l)| l.video_source.filter(|v| pictures.contains_key(v)).map(|v| (*id, v)))
        .collect();
    let mut fold = SoundFold::default();
    let mut by: IndexMap<&str, Vec<&str>> = IndexMap::new();
    for (id, picture) in &sounding {
        by.entry(picture).or_default().push(id);
    }
    for (picture, ids) in &by {
        if let [only] = ids.as_slice() {
            fold.folded.insert((*only).to_owned(), format!("{picture}#audio"));
        } else {
            fold.warnings.push(format!(
                "{picture}: {} all play its sound — left as they are, delete all but one and open again",
                ids.join(" and ")
            ));
        }
    }
    let renamed = |n: &str| fold.folded.get(n).cloned().unwrap_or_else(|| n.to_owned());
    let mut writes = Writes::default();
    let mut removes: Vec<String> = vec![];

    if !fold.folded.is_empty() {
        if let Some(graph) = entity_file_for(fs, dir, "", GRAPH_BASE) {
            let mut doc = Document::new(&ws_text(fs, &graph.path)?);
            let next = folded_graph(&doc.to_js()?, &fold.folded);
            doc.at(&[]).apply_in(&[], &next)?;
            writes.set(graph.path, doc.text().to_owned());
        }
        if let Some(root) = entity_file_for(fs, dir, "", "project") {
            let mut doc = Document::new(&ws_text(fs, &root.path)?);
            let data = doc.to_js()?;
            let mut r = doc.at(&[]);
            for k in ["soloed", "highlighted"] {
                if matches!(data.get(k), Some(Value::List(_))) {
                    let next = strings(data.get(k)).iter().map(|x| renamed(x)).collect();
                    r.set_in(&[key(k)], &str_list(next), WriteOptions { inline: true })?;
                }
            }
            if matches!(data.get("trackOrder"), Some(Value::List(_))) {
                let next = strings(data.get("trackOrder"))
                    .into_iter()
                    .filter(|x| !fold.folded.contains_key(x))
                    .collect();
                r.set_in(&[key("trackOrder")], &str_list(next), WriteOptions { inline: true })?;
            }
            writes.set(root.path, doc.text().to_owned());
        }
        for lane in fold.folded.keys() {
            if let Some(file) = lane_file_for(fs, dir, lane) {
                removes.push(file.path);
            }
        }
    }

    for (abs, text) in &writes.0 {
        ws_write_text(fs, abs, text)?;
    }
    for abs in &removes {
        fs.remove(abs).map_err(|e| io_error(&e, "rm", abs))?;
    }
    for (lane, half) in &fold.folded {
        rename_node_in_assemblies(fs, dir, lane, half)?;
    }

    // A picture lane whose shots sound, and that no lane sounds, plays on its own: a format a level-3 build misreads.
    let mut clips: IndexMap<String, Value> = IndexMap::new();
    for (id, file) in entity_files(fs, &join(&[dir, "clips"]), "clips") {
        clips.insert(id, ws_data(fs, &file.path)?);
    }
    let still_sounded: HashSet<&str> = sounding
        .iter()
        .filter(|(id, _)| !fold.folded.contains_key(*id))
        .map(|(_, picture)| *picture)
        .collect();
    let on_its_own = pictures.iter().any(|(id, lane)| {
        !still_sounded.contains(id)
            && lane.events.iter().any(|e| {
                let name = e.ref_.as_ref().map_or("", |r| r.0.as_str());
                truthy(clips.get(name).and_then(|c| c.get("sound")))
            })
    });
    if on_its_own {
        stamp_format_level(fs, dir, PICTURE_SOUND_LEVEL);
    }
    Ok(Some(fold))
}
