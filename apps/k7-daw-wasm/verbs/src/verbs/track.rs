// track.rs — `track rename` and `track delete`, after verbs/track.ts and trackDelete.ts: a lane's file, its chain and every list naming it.
// A reader would get wrong what a delete keeps: the lane's clips and sequences stay on disk, and the journal undoes the lot.

use super::shared::{Done, join, split_words};
use crate::host::{Host, Thrown};
use k7x::edit::{PathSeg, Value};
use song::Cache;
use song::routing::split_port;
use song::views::surfaces::project_surfaces;
use song_edit::Fs;
use song_edit::assembly_edit::{drop_node_from_assemblies, rename_node_in_assemblies};
use song_edit::chain_item_edit::{is_chain_project, lane_file_for, lane_folder};
use song_edit::chain_project::read_chain_project;
use song_edit::graph_edit::{delete_chain_node, rename_chain_node};
use song_edit::graph_schema::parse_graph;
use song_edit::lane_strip::{is_lane_mixer_name, mixer_name_for};
use song_edit::lane_strip_plan::LaneGraph;
use song_edit::module_io::entity_file_for;
use song_edit::project_root::load_project_root;
use song_edit::workspace_doc::{EditOpts, ws_edit, ws_text};
use song_edit::workspace_file::ws_exists;
use song_edit::yaml_edit::{delete_file_value, set_file_value};

const GRAPH_FILE: &str = "graph.k7x";

/// `surfacesNaming`: the surfaces whose mapping names `module`, which no rename or delete rewrites.
fn surfaces_naming(h: &Host, module: &str) -> Result<Vec<String>, Thrown> {
    let s = project_surfaces(&*h.fs, &mut Cache::new(), &h.song_path).map_err(|e| Thrown(e.to_string()))?;
    Ok(s.surfaces
        .into_iter()
        .filter(|p| {
            p.surface
                .mapping
                .as_ref()
                .is_some_and(|m| m.values().any(|b| b.0.as_ref().is_some_and(|b| b.module == module)))
        })
        .map(|p| p.id)
        .collect())
}

/// `ownLaneStrip`: the strip a lane's chain ends in, when it is the lane's own.
fn own_lane_strip(h: &Host, lane: &str) -> Result<Option<String>, song_edit::Error> {
    let g = LaneGraph::load(&*h.fs, &h.env, &h.song_path)?;
    let chained = is_chain_project(&*h.fs, &h.song_path);
    Ok(g.strip_of(lane)
        .filter(|s| chained || is_lane_mixer_name(s, lane))
        .map(str::to_owned))
}

/// The names already used by a node: the graph's keys, a chain project's nodes, a lane file.
struct Taken {
    graph: Vec<String>,
    nodes: Vec<String>,
}

impl Taken {
    fn read(fs: &dyn Fs, dir: &str) -> Result<Taken, song_edit::Error> {
        let chained = is_chain_project(fs, dir);
        let graph_abs = join(dir, GRAPH_FILE);
        let graph = if !chained && ws_exists(fs, &graph_abs) {
            match parse_graph(&ws_text(fs, &graph_abs)?)
                .value
                .and_then(|v| v.get("modules").cloned())
            {
                Some(Value::Map(m)) => m.into_iter().map(|(k, _)| k).collect(),
                _ => Vec::new(),
            }
        } else {
            Vec::new()
        };
        let nodes = if chained {
            read_chain_project(fs, dir)?.nodes.keys().cloned().collect()
        } else {
            Vec::new()
        };
        Ok(Taken { graph, nodes })
    }

    fn has(&self, fs: &dyn Fs, dir: &str, name: &str) -> bool {
        self.graph.iter().any(|n| n == name)
            || self.nodes.iter().any(|n| n == name)
            || ws_exists(fs, &join(dir, &format!("{}/{name}.k7x", lane_folder(fs, dir))))
    }
}

/// The strip file's first comment line names its lane; a renamed lane's strip is re-labelled.
fn relabel_strip(fs: &mut dyn Fs, abs: &str, lane: &str) -> Result<(), song_edit::Error> {
    let opts = EditOpts {
        create: false,
        raw: false,
    };
    ws_edit(fs, abs, opts, |doc| {
        let header = doc.header();
        let Some(first) = header.first() else {
            return Ok(());
        };
        let Some(at) = first.find("'s channel strip") else {
            return Ok(());
        };
        let next = format!("{lane}{}", &first[at..]);
        if next != *first {
            let mut lines = header.clone();
            lines[0] = next;
            doc.set_header(&lines)?;
        }
        Ok(())
    })?;
    Ok(())
}

/// The strip named after a lane follows its rename: `mix-<old>` becomes `mix-<new>`, its first line re-labelled.
fn follow_strip_name(h: &mut Host, mixer: &str, to: &str, taken: &Taken) -> Result<Option<String>, song_edit::Error> {
    let dir = h.song_path.clone();
    let next = mixer_name_for(to, &|n| n != mixer && taken.has(&*h.fs, &dir, n));
    if next == mixer {
        return Ok(None);
    }
    rename_chain_node(h.fs, &h.env, &dir, mixer, &next)?;
    rename_node_in_assemblies(h.fs, &dir, mixer, &next)?;
    let strip = join(&dir, &format!("nodes/{next}.k7x"));
    if ws_exists(&*h.fs, &strip) {
        relabel_strip(h.fs, &strip, to)?;
    }
    Ok(Some(format!("{mixer} → {next}")))
}

/// `track rename <track> <new name>` — the file, the graph, the boxes, the order, the solo and the strip follow.
pub fn track_rename(a: &str, h: &mut Host) -> Done {
    let w = split_words(a);
    let (Some(id_arg), Some(to)) = (w.first().copied(), w.get(1).copied()) else {
        h.refuse(["usage: track rename <track> <new name> (the whole name, prefix included — e.g. track rename ge.lead vo.lead)".to_owned()]);
        return Ok(());
    };
    let dir = h.song_path.clone();
    let lane = split_port(id_arg).0.to_owned();
    if lane_file_for(&*h.fs, &dir, &lane).is_none() {
        let folder = lane_folder(&*h.fs, &dir);
        h.refuse([format!("no track \"{id_arg}\" — {folder}/{lane}.k7x does not exist")]);
        return Ok(());
    }
    if to == lane {
        h.refuse([format!("\"{lane}\" is already called that")]);
        return Ok(());
    }
    if to.is_empty()
        || !to
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
    {
        h.refuse([format!(
            "invalid track name '{to}' — letters, digits, '.', '_' and '-' only"
        )]);
        return Ok(());
    }
    let mapped = surfaces_naming(h, &lane)?;
    if !mapped.is_empty() {
        h.refuse([format!(
            "\"{lane}\" is named by {} {} — a mapping is not rewritten, so the track keeps its name",
            if mapped.len() > 1 { "surfaces" } else { "surface" },
            mapped.join(", ")
        )]);
        return Ok(());
    }
    if let Err(e) = rename(h, &lane, to) {
        h.refuse([format!("track rename: {e}")]);
    }
    Ok(())
}

fn rename(h: &mut Host, lane: &str, to: &str) -> Result<(), song_edit::Error> {
    let dir = h.song_path.clone();
    let taken = Taken::read(&*h.fs, &dir)?;
    if taken.has(&*h.fs, &dir, to) {
        h.refuse([format!("\"{to}\" is already a node of this project")]);
        return Ok(());
    }
    let mixer = if is_chain_project(&*h.fs, &dir) {
        None
    } else {
        own_lane_strip(h, lane)?
    };
    let (moved, rewired) = rename_chain_node(h.fs, &h.env, &dir, lane, to)?;
    let boxes = rename_node_in_assemblies(h.fs, &dir, lane, to)?;
    let mut notes = moved;
    if !rewired.is_empty() {
        notes.push(format!("rewired {}", rewired.join(", ")));
    }
    if !boxes.is_empty() {
        notes.push(format!("followed in {}", boxes.join(", ")));
    }
    let root = load_project_root(&*h.fs, &dir, false)?;
    let project = entity_file_for(&*h.fs, &dir, "", "project");
    for key in ["trackOrder", "soloed"] {
        let (Some(file), Some(Value::List(list))) = (&project, root.data.get(key)) else {
            continue;
        };
        if !list.iter().any(|x| matches!(x, Value::Str(s) if s == lane)) {
            continue;
        }
        let next: Vec<Value> = list
            .iter()
            .map(|x| match x {
                Value::Str(s) if s == lane => Value::Str(to.to_owned()),
                other => other.clone(),
            })
            .collect();
        set_file_value(h.fs, &file.path, &[PathSeg::Key(key.to_owned())], &Value::List(next))?;
        notes.push(format!("{key} in {}", file.rel.trim_start_matches('/')));
    }
    if let Some(m) = mixer
        && let Some(followed) = follow_strip_name(h, &m, to, &taken)?
    {
        notes.push(followed);
    }
    h.reload();
    h.log([format!("{lane} → {to} ({})", notes.join(", "))]);
    Ok(())
}

/// The nodes a lane takes with it, the lane LAST: each delete rejoins around its node.
fn lane_own_nodes(h: &Host, lane: &str) -> Result<Vec<String>, song_edit::Error> {
    if is_chain_project(&*h.fs, &h.song_path) {
        let cp = read_chain_project(&*h.fs, &h.song_path)?;
        let Some(chain) = cp.nodes.get(lane).map(|n| n.chain.clone()) else {
            return Ok(vec![lane.to_owned()]);
        };
        let mut out: Vec<String> = cp
            .nodes
            .values()
            .filter(|n| n.chain == chain && n.name != lane)
            .map(|n| n.name.clone())
            .collect();
        out.push(lane.to_owned());
        return Ok(out);
    }
    Ok(match own_lane_strip(h, lane)? {
        Some(m) => vec![m, lane.to_owned()],
        None => vec![lane.to_owned()],
    })
}

/// `track delete <track>` — the lane and the chain it starts; three lists of project.k7x forget it.
pub fn track_delete(a: &str, h: &mut Host) -> Done {
    let Some(arg) = split_words(a).first().map(|s| (*s).to_owned()) else {
        h.refuse(["usage: track delete <track>".to_owned()]);
        return Ok(());
    };
    let p = h.project()?;
    let Some(id) = p
        .tracks
        .iter()
        .find(|x| x.id == arg || x.name == arg)
        .map(|t| t.id.clone())
    else {
        h.refuse([format!("no track \"{arg}\"")]);
        return Ok(());
    };
    let mapped = surfaces_naming(h, &id)?;
    if !mapped.is_empty() {
        h.refuse([format!(
            "\"{id}\" is named by {} {} — unmap it there first",
            if mapped.len() > 1 { "surfaces" } else { "surface" },
            mapped.join(", ")
        )]);
        return Ok(());
    }
    match delete(h, &id) {
        Ok(n) => {
            h.log([format!(
                "deleted {id} and the {} node(s) of its chain — its clips stay on disk",
                n - 1
            )]);
            h.reload();
        }
        Err(e) => h.refuse([format!("track delete: {e}")]),
    }
    Ok(())
}

fn delete(h: &mut Host, id: &str) -> Result<usize, song_edit::Error> {
    let dir = h.song_path.clone();
    let nodes = lane_own_nodes(h, id)?;
    for node in &nodes {
        delete_chain_node(h.fs, &h.env, &dir, node)?;
        drop_node_from_assemblies(h.fs, &dir, node)?;
    }
    let root = load_project_root(&*h.fs, &dir, false)?;
    if let Some(file) = entity_file_for(&*h.fs, &dir, "", "project") {
        for key in ["trackOrder", "soloed", "highlighted"] {
            let Some(Value::List(list)) = root.data.get(key) else {
                continue;
            };
            if !list.iter().any(|x| matches!(x, Value::Str(s) if s == id)) {
                continue;
            }
            let next: Vec<Value> = list
                .iter()
                .filter(|x| !matches!(x, Value::Str(s) if s == id))
                .cloned()
                .collect();
            let path = [PathSeg::Key(key.to_owned())];
            if next.is_empty() {
                delete_file_value(h.fs, &file.path, &path)?;
            } else {
                set_file_value(h.fs, &file.path, &path, &Value::List(next))?;
            }
        }
    }
    Ok(nodes.len())
}
