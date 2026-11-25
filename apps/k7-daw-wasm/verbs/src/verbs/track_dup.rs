// track_dup.rs — `track arrange` and `track duplicate [empty]` in a graph project, after verbs/track.ts.
// A reader would get wrong what a duplicate copies: the lane's OWN chain nodes, those with no other feeder; shared ones are named.

use super::shared::{Done, join, split_words};
use super::track_chain_copy::duplicate_lane_chain;
use crate::host::{Host, Thrown};
use k7x::edit::{Document, PathSeg, Value};
use song::Cache;
use song::doc::{TrackCategory, auto_colors, track_category};
use song::graph::{GraphError, load_graph};
use song::lanes::inputs::load_inputs;
use song::lanes::is_builtin_output;
use song::lanes::rig::Rig;
use song::routing::{port_ref, split_port};
use song_edit::Fs;
use song_edit::chain_create::chain_taken;
use song_edit::chain_item_edit::{is_chain_project, lane_file_for, lane_folder};
use song_edit::chain_project::read_chain_project;
use song_edit::graph_edit::read_graph_value;
use song_edit::graph_file::{Segments, merge_segments};
use song_edit::graph_schema::parse_graph;
use song_edit::instance_name::instance_name;
use song_edit::lane_files::create_track_file;
use song_edit::lane_strip::mixer_name_for;
use song_edit::loader::{load_modules, roots_for};
use song_edit::module_io::entity_file_for;
use song_edit::node_files::copy_node_files;
use song_edit::track_edit::edit_track_struct;
use song_edit::track_file::placement_values;
use song_edit::track_mix::add_track_placements;
use song_edit::workspace_doc::{EditOpts, ws_data, ws_edit, ws_text, ws_write_data};
use song_edit::workspace_file::ws_exists;
use song_edit::yaml_edit::set_file_value;
use std::collections::HashMap;

const GRAPH_FILE: &str = "graph.k7x";
const CATEGORIES: [TrackCategory; 7] = [
    TrackCategory::Vox,
    TrackCategory::Drums,
    TrackCategory::Bass,
    TrackCategory::Guitar,
    TrackCategory::Keys,
    TrackCategory::Synth,
    TrackCategory::Other,
];

fn category_name(c: TrackCategory) -> &'static str {
    match c {
        TrackCategory::Vox => "vox",
        TrackCategory::Drums => "drums",
        TrackCategory::Bass => "bass",
        TrackCategory::Guitar => "guitar",
        TrackCategory::Keys => "keys",
        TrackCategory::Synth => "synth",
        TrackCategory::Other => "other",
    }
}

/// What the duplicate and the arrange read of the loaded graph, owned.
#[derive(Default)]
struct Facts {
    lanes: Vec<String>,
    tracks: Vec<String>,
    strips: Vec<String>,
    extends: HashMap<String, String>,
    hints: HashMap<String, Vec<String>>,
}

fn facts(h: &Host) -> Result<Facts, Thrown> {
    let roots = roots_for(&*h.fs, &h.env, &h.song_path);
    let mut cache = Cache::new();
    let inputs = load_inputs(&*h.fs, &mut cache, &roots, Rig::default()).map_err(|e| Thrown(e.to_string()))?;
    let g = load_graph(&*h.fs, &mut cache, &roots, &inputs).map_err(|e| match e {
        GraphError::Source(s) => Thrown(s.to_string()),
        GraphError::Refused(r) => Thrown(
            r.findings
                .first()
                .map_or_else(|| "the graph does not load".to_owned(), |f| f.message.clone()),
        ),
    })?;
    let mut f = Facts {
        tracks: g.tracks.keys().map(|k| (*k).to_owned()).collect(),
        ..Facts::default()
    };
    f.lanes = f
        .tracks
        .iter()
        .cloned()
        .chain(g.video_tracks.keys().map(|k| (*k).to_owned()))
        .chain(g.disabled.iter().map(|k| (*k).to_owned()))
        .collect();
    for (name, m) in &g.modules {
        let plugin = m.m.authored.plugin.as_ref().and_then(|p| p.name.as_deref());
        if plugin == Some(song::resolve::MIXER_PLUGIN)
            || matches!(m.extends, Some("@meta/audio/mixer" | "@meta/shaders/video-mixer"))
        {
            f.strips.push((*name).to_owned());
        }
        if let Some(e) = m.extends {
            f.extends.insert((*name).to_owned(), e.to_owned());
        }
    }
    for c in g.chains.values() {
        let Some(from) = c.from.as_deref() else { continue };
        let lane = split_port(from).0.to_owned();
        for step in &c.use_steps {
            if let Some(m) = g.modules.get(step.module.as_str()) {
                let kind =
                    m.m.authored
                        .module_type
                        .and_then(|t| serde_json::to_value(t).ok())
                        .and_then(|v| v.as_str().map(str::to_owned))
                        .unwrap_or_default();
                let hints = f.hints.entry(lane.clone()).or_default();
                hints.push(m.extends.unwrap_or("").to_owned());
                hints.push(kind);
            }
        }
    }
    Ok(f)
}

/// `track arrange` — every lane coloured by its category and the whole order stated, vox first.
pub fn track_arrange(_a: &str, h: &mut Host) -> Done {
    let dir = h.song_path.clone();
    let f = facts(h)?;
    let category_of = |lane: &str| -> TrackCategory {
        let hints = f.hints.get(lane).cloned().unwrap_or_default();
        track_category(lane, &hints)
    };
    let p = h.project()?;
    let mut lanes: Vec<String> = Vec::new();
    for t in &p.tracks {
        let lane = split_port(&t.id).0.to_owned();
        if !lanes.contains(&lane) && lane_file_for(&*h.fs, &dir, &lane).is_some() {
            lanes.push(lane);
        }
    }
    if lanes.is_empty() {
        h.refuse(["track arrange: this song has no lanes".to_owned()]);
        return Ok(());
    }
    let rank = |c: TrackCategory| CATEGORIES.iter().position(|x| *x == c).unwrap_or(CATEGORIES.len());
    let mut ordered: Vec<(usize, String)> = lanes.into_iter().enumerate().collect();
    ordered.sort_by_key(|(i, l)| (rank(category_of(l)), *i));
    let ordered: Vec<String> = ordered.into_iter().map(|(_, l)| l).collect();
    let cats: Vec<(&str, TrackCategory)> = ordered.iter().map(|l| (l.as_str(), category_of(l))).collect();
    let colors: Vec<(String, String)> = auto_colors(&cats).into_iter().map(|(k, v)| (k.to_owned(), v)).collect();
    if let Err(e) = arrange(h, &ordered, &colors) {
        h.refuse([format!("track arrange: {e}")]);
        return Ok(());
    }
    let said: Vec<String> = CATEGORIES
        .iter()
        .filter_map(|c| {
            let of: Vec<&str> = ordered
                .iter()
                .filter(|l| category_of(l) == *c)
                .map(String::as_str)
                .collect();
            (!of.is_empty()).then(|| format!("{}: {}", category_name(*c), of.join(", ")))
        })
        .collect();
    h.log([format!("arranged {} tracks — {}", ordered.len(), said.join(" · "))]);
    h.reload();
    Ok(())
}

fn arrange(h: &mut Host, ordered: &[String], colors: &[(String, String)]) -> Result<(), song_edit::Error> {
    let dir = h.song_path.clone();
    for lane in ordered {
        if let Some((_, c)) = colors.iter().find(|(l, _)| l == lane) {
            let v = Value::Str(c.clone());
            edit_track_struct(h.fs, &dir, lane, "colour a track by its category", |doc| {
                Ok(doc.set_in(&[PathSeg::Key("color".to_owned())], &v, Default::default())?)
            })?;
        }
    }
    let Some(project) = entity_file_for(&*h.fs, &dir, "", "project") else {
        return Err(song_edit::Error::Msg(
            "no project.k7x — there is nowhere to state a lane order".to_owned(),
        ));
    };
    let order = Value::List(ordered.iter().cloned().map(Value::Str).collect());
    set_file_value(h.fs, &project.path, &[PathSeg::Key("trackOrder".to_owned())], &order)
}

/// The one name space a copy is named in: the graph's or chains' nodes, the modules, the lane files, the chain files.
pub struct Names {
    pub reserved: Vec<String>,
    fixed: Vec<String>,
    chained: bool,
}

impl Names {
    pub fn taken(&self, fs: &dyn Fs, dir: &str, name: &str) -> bool {
        self.reserved.iter().any(|n| n == name)
            || self.fixed.iter().any(|n| n == name)
            || ws_exists(fs, &join(dir, &format!("{}/{name}.k7x", lane_folder(fs, dir))))
            || (self.chained && chain_taken(fs, dir, name))
    }

    pub fn claim(&mut self, name: String) -> String {
        self.reserved.push(name.clone());
        name
    }
}

fn dests_of(v: &Value) -> Vec<String> {
    match v {
        Value::List(l) => l
            .iter()
            .filter_map(|x| if let Value::Str(s) = x { Some(s.clone()) } else { None })
            .collect(),
        _ => Vec::new(),
    }
}

fn list_of(v: &Value, k: &str) -> Vec<String> {
    v.get(k).map(dests_of).unwrap_or_default()
}

/// `duplicateChain`: the lane's exclusive chain nodes copied, the new lane wired in beside it.
fn duplicate_graph_chain(
    h: &mut Host,
    lane: &str,
    new_lane: &str,
    f: &Facts,
    names: &mut Names,
) -> Result<Vec<String>, Thrown> {
    let dir = h.song_path.clone();
    let graph_abs = join(&dir, GRAPH_FILE);
    if !ws_exists(&*h.fs, &graph_abs) {
        return Ok(Vec::new());
    }
    let graph = read_graph_value(&*h.fs, &dir)?;
    let map: Vec<(String, Vec<String>)> = match graph.get("modules") {
        Some(Value::Map(m)) => m.iter().map(|(k, v)| (k.clone(), dests_of(v))).collect(),
        _ => Vec::new(),
    };
    let dests = |k: &str| {
        map.iter()
            .find(|(x, _)| x == k)
            .map(|(_, d)| d.clone())
            .unwrap_or_default()
    };
    let mut incoming: HashMap<String, usize> = HashMap::new();
    for (_, ds) in &map {
        for d in ds {
            *incoming.entry(split_port(d).0.to_owned()).or_default() += 1;
        }
    }
    let boundary = |n: &str| is_builtin_output(n) || f.lanes.iter().any(|l| l == n) || n == lane;
    let should_clone = |n: &str| !boundary(n) && incoming.get(n).copied().unwrap_or(0) == 1;
    let mut clones: Vec<(String, String)> = Vec::new();
    let map_dest = |d: &str, names: &mut Names, fs: &dyn Fs, clones: &mut Vec<(String, String)>| -> String {
        let (node, port) = split_port(d);
        if !should_clone(node) {
            return d.to_owned();
        }
        if let Some((_, to)) = clones.iter().find(|(from, _)| from == node) {
            return port_ref(to, port);
        }
        let to = if f.strips.iter().any(|s| s == node) {
            let n = mixer_name_for(new_lane, &|x| names.taken(fs, &dir, x));
            names.claim(n)
        } else {
            let leaf = match f.extends.get(node) {
                Some(e) => e.rsplit('/').next().unwrap_or(node).to_owned(),
                None if node.starts_with('@') => node.rsplit('/').next().unwrap_or(node).to_owned(),
                None => node.to_owned(),
            };
            let n = instance_name(&leaf, Some(new_lane), &|x| names.taken(fs, &dir, x));
            names.claim(n)
        };
        clones.push((node.to_owned(), to.clone()));
        port_ref(&to, port)
    };
    let mut additions: Vec<(String, Vec<String>)> = Vec::new();
    for (key, ds) in map.iter().filter(|(k, _)| split_port(k).0 == lane) {
        let mapped = ds.iter().map(|d| map_dest(d, names, &*h.fs, &mut clones)).collect();
        additions.push((port_ref(new_lane, split_port(key).1), mapped));
    }
    let mut i = 0;
    while i < clones.len() {
        let (from, to) = clones[i].clone();
        let mapped = dests(&from)
            .iter()
            .map(|d| map_dest(d, names, &*h.fs, &mut clones))
            .collect();
        additions.push((to, mapped));
        i += 1;
    }
    let bypassed = list_of(&graph, "bypassed");
    let bus_only = list_of(&graph, "bus_only");
    let mut seg = Segments {
        modules: additions,
        ..Segments::default()
    };
    let mut written = Vec::new();
    for (from, to) in &clones {
        if bypassed.contains(from) {
            seg.bypassed.push(to.clone());
        }
        if bus_only.contains(from) {
            seg.bus_only.push(to.clone());
        }
        let src = join(&dir, &format!("nodes/{from}.k7x"));
        if ws_exists(&*h.fs, &src) {
            copy_node_files(h.fs, &src, &dir, from, &dir, to)?;
        } else {
            h.fs.mkdir_all(&join(&dir, "nodes"))
                .map_err(|e| Thrown(e.to_string()))?;
            let body = Value::Map(vec![("extends".to_owned(), Value::Str(from.clone()))]);
            ws_write_data(
                h.fs,
                &join(&dir, &format!("nodes/{to}.k7x")),
                &body,
                &Default::default(),
            )?;
        }
        if f.strips.contains(from) {
            relabel(h.fs, &join(&dir, &format!("nodes/{to}.k7x")), new_lane)?;
        }
        written.push(format!("nodes/{to}.k7x"));
    }
    if !seg.modules.is_empty() || !seg.bypassed.is_empty() || !seg.bus_only.is_empty() {
        ws_edit(h.fs, &graph_abs, EditOpts::default(), |doc| {
            merge_segments(&mut doc.at(&[]), &seg)
        })?;
        written.push(GRAPH_FILE.to_owned());
    }
    Ok(written)
}

/// The strip's first comment line names its lane: `<lane>'s channel strip`.
fn relabel(fs: &mut dyn Fs, abs: &str, lane: &str) -> Result<(), song_edit::Error> {
    ws_edit(fs, abs, EditOpts::default(), |doc| {
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

fn duplicate(a: &str, h: &mut Host, events: bool) -> Done {
    let Some(id_arg) = split_words(a).first().map(|s| (*s).to_owned()) else {
        h.refuse([format!(
            "usage: track duplicate{} <track>",
            if events { "" } else { " empty" }
        )]);
        return Ok(());
    };
    let dir = h.song_path.clone();
    let lane = split_port(&id_arg).0.to_owned();
    let chained = is_chain_project(&*h.fs, &dir);
    let src_rel = format!("{}/{lane}.k7x", lane_folder(&*h.fs, &dir));
    let src_abs = join(&dir, &src_rel);
    if !ws_exists(&*h.fs, &src_abs) {
        h.refuse([format!("no track \"{id_arg}\" — {src_rel} does not exist")]);
        return Ok(());
    }
    if let Err(e) = duplicate_lane(h, &lane, &src_abs, chained, events) {
        h.refuse([format!("track duplicate: {}", e.0)]);
    }
    Ok(())
}

fn duplicate_lane(h: &mut Host, lane: &str, src_abs: &str, chained: bool, events: bool) -> Result<(), Thrown> {
    let dir = h.song_path.clone();
    let src_text = ws_text(&*h.fs, src_abs)?;
    let src = ws_data(&*h.fs, src_abs)?;
    if matches!(src.get("tracks"), Some(Value::List(_))) {
        h.refuse([format!("\"{lane}\" is a group — a group has no events to duplicate")]);
        return Ok(());
    }
    if matches!(src.get("strudel"), Some(Value::Str(_))) {
        h.refuse([format!("\"{lane}\" is a strudel lane — duplicate the pattern instead")]);
        return Ok(());
    }
    let f = if chained { Facts::default() } else { facts(h)? };
    let graph_abs = join(&dir, GRAPH_FILE);
    let mut fixed: Vec<String> = Vec::new();
    if chained {
        fixed.extend(read_chain_project(&*h.fs, &dir)?.nodes.keys().cloned());
    } else if ws_exists(&*h.fs, &graph_abs) {
        let parsed = parse_graph(&ws_text(&*h.fs, &graph_abs)?);
        let Some(v) = parsed.value.filter(|_| parsed.issues.is_empty()) else {
            return Err(Thrown(
                parsed
                    .issues
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "graph.k7x unreadable".to_owned()),
            ));
        };
        if let Some(Value::Map(m)) = v.get("modules") {
            fixed.extend(m.iter().map(|(k, _)| k.clone()));
        }
    }
    fixed.extend(load_modules(&*h.fs, &h.env, &dir)?.keys().cloned());
    let mut names = Names {
        reserved: Vec::new(),
        fixed,
        chained,
    };
    let base = format!("{lane}-copy");
    let mut new_lane = base.clone();
    let mut n = 2;
    while names.taken(&*h.fs, &dir, &new_lane) {
        new_lane = format!("{base}-{n}");
        n += 1;
    }
    let new_lane = names.claim(new_lane);
    let made = create_track_file(h.fs, &dir, &new_lane, "@meta/track", h.new_id)?;
    if let Some(Value::Str(color)) = src.get("color") {
        let v = Value::Str(color.clone());
        edit_track_struct(h.fs, &dir, &new_lane, "copy a track's colour", |doc| {
            Ok(doc.set_in(&[PathSeg::Key("color".to_owned())], &v, Default::default())?)
        })?;
    }
    if events {
        let mut doc = Document::new(&src_text);
        let placements = placement_values(&doc.at(&[]))?;
        if !placements.is_empty() {
            add_track_placements(h.fs, &dir, &new_lane, &placements)?;
        }
    }
    let chain_files = if chained {
        duplicate_lane_chain(h, lane, &new_lane, &mut names)?
    } else {
        duplicate_graph_chain(h, lane, &new_lane, &f, &mut names)?
    };
    h.reload();
    let mut files: Vec<String> = Vec::new();
    for x in made.into_iter().chain(chain_files) {
        if !files.contains(&x) {
            files.push(x);
        }
    }
    h.log([format!("+ track {new_lane} ({})", files.join(", "))]);
    Ok(())
}

/// `track duplicate <track>` — the lane, its events and its own chain.
pub fn track_duplicate(a: &str, h: &mut Host) -> Done {
    duplicate(a, h, true)
}

/// `track duplicate empty <track>` — the lane and its chain, no events.
pub fn track_duplicate_empty(a: &str, h: &mut Host) -> Done {
    duplicate(a, h, false)
}
