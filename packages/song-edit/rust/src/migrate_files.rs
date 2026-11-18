// migrate_files.rs — a node project (`graph.k7x` + `nodes/`) rewritten as `chains/` + `tracks/` + `vstpresets/` + `assets/` + `layout.k7x`.
// A reader would get wrong that a non-lane node loses its name: it becomes its fresh id, and only a lane keeps its name.

use crate::chain_data::{Chain, Item, is_video_item, lane_of, set_key};
use crate::chain_item_edit::{CHAINS_DIR, TRACKS_DIR, chain_file_path, is_chain_project};
use crate::chain_project::write_layout;
use crate::confine::Env;
use crate::error::{Res, fail, io_error};
use crate::format_chain::format_chain;
use crate::from_segments::{FromSegments, GraphIn, from_segments};
use crate::fs::Fs;
use crate::graph_schema::{GRAPH_FILE, parse_graph};
use crate::ids::NewId;
use crate::loader::{load_module_refs, load_modules, resolve_all};
use crate::paths::join;
use crate::sugar::is_doc_file;
use crate::workspace_doc::{EditOpts, ws_data, ws_edit, ws_text, ws_write_text};
use indexmap::IndexMap;
use k7x::edit::{PathSeg, Value, WriteOptions};
use song::lanes::lanes_of;
use song::modules::IoKind;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

const VSTPRESETS_DIR: &str = "vstpresets";
const ASSETS_DIR: &str = "assets";
const NODES: &str = "nodes";
const SHAPE_CATEGORY: &str = "shapes";

/// What a migration did: each non-lane node's id, the nodes no chain reached, and the chains written.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FilesMigration {
    pub ids: IndexMap<String, String>,
    pub loose: Vec<String>,
    pub chains: Vec<String>,
}

fn is_k7x_or_preset(f: &str) -> bool {
    f.ends_with(".k7x") || f.ends_with(".vstpreset")
}

fn names_of(fs: &dyn Fs, dir: &str) -> Vec<(String, bool)> {
    fs.list(dir)
        .map(|l| l.into_iter().map(|e| (e.name, e.kind == song::Kind::Dir)).collect())
        .unwrap_or_default()
}

/// Whether a surface file binds node params, under a `mapping` key at any depth; a file that does not read counts as one.
fn maps_params(fs: &dyn Fs, path: &str) -> bool {
    fn has(v: &Value) -> bool {
        match v {
            Value::List(l) => l.iter().any(has),
            Value::Map(m) => m.iter().any(|(k, x)| k == "mapping" || has(x)),
            _ => false,
        }
    }
    ws_data(fs, path).map_or(true, |d| has(&d))
}

/// Why this project cannot be migrated yet, or None: each reason names a file that refers to nodes by NAME.
#[must_use]
pub fn migration_blocker(fs: &dyn Fs, dir: &str) -> Option<String> {
    if is_chain_project(fs, dir) {
        return Some("already a chain project".to_owned());
    }
    if !fs.exists(&join(&[dir, GRAPH_FILE])) {
        return Some(format!("no {GRAPH_FILE}"));
    }
    if fs.exists(&join(&[dir, "assemblies"])) {
        return Some("assemblies/ names nodes".to_owned());
    }
    if fs.exists(&join(&[dir, "views"])) {
        return Some("views/ binds node params".to_owned());
    }
    for (f, _) in names_of(fs, &join(&[dir, NODES])) {
        if !is_k7x_or_preset(&f) && f != ASSETS_DIR && f != "base" {
            return Some(format!("nodes/{f} is neither a node, a preset, assets/ nor base/"));
        }
    }
    let surf = join(&[dir, "surfaces"]);
    for (f, _) in names_of(fs, &surf).into_iter().filter(|(f, _)| is_doc_file(f)) {
        if maps_params(fs, &join(&[&surf, &f])) {
            return Some(format!("surfaces/{f} maps node params"));
        }
    }
    None
}

/// `[^A-Za-z0-9_.-]` replaced by `_`, one per UTF-16 unit as the TypeScript's regex counts them.
fn sanitized(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-') {
                c.to_string()
            } else {
                "_".repeat(c.len_utf16())
            }
        })
        .collect()
}

fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null | Value::Bool(false)) => false,
        Some(Value::Str(s)) => !s.is_empty(),
        Some(Value::Num(n)) => *n != 0.0 && !n.is_nan(),
        Some(_) => true,
    }
}

/// The picture lane a chain's own steps hold, if any: a `video_track` item, or a track item of a picture lane.
fn picture_lane_in(c: &Chain, is_picture: &dyn Fn(&str) -> bool) -> Option<String> {
    c.steps.iter().flat_map(|s| s.items()).find_map(|i| {
        lane_of(i)
            .filter(|l| is_video_item(i) || is_picture(l))
            .map(str::to_owned)
    })
}

/// A fade named its stretch by the run's head NODE; in a chain file the stretch is a subchain, named by its id.
fn rename_stretches(
    fs: &mut dyn Fs,
    dir: &str,
    chains: &IndexMap<String, Chain>,
    ids: &IndexMap<String, String>,
    pictures: &HashSet<String>,
) -> Res<()> {
    let by_id: HashMap<&str, &str> = ids.iter().map(|(n, id)| (id.as_str(), n.as_str())).collect();
    let mut renames: HashMap<String, String> = HashMap::new();
    for c in chains.values() {
        if picture_lane_in(c, &|l| pictures.contains(l)).is_none() {
            continue;
        }
        for sc in &c.subchains {
            let head = sc.steps.iter().flat_map(|s| s.items()).next();
            if let Some(was) = head.and_then(|h| by_id.get(crate::chain_data::id_of(h))) {
                renames.insert((*was).to_owned(), sc.id.clone());
            }
        }
    }
    if renames.is_empty() {
        return Ok(());
    }
    // Still in `nodes/`: the lane files move after this, with the sound lanes'.
    for lane in pictures {
        let f = join(&[dir, NODES, &format!("{lane}.k7x")]);
        if !fs.exists(&f) {
            continue;
        }
        ws_edit(fs, &f, EditOpts::default(), |doc| {
            let data = doc.to_js()?;
            let Some(Value::List(events)) = data.get("events") else {
                return Ok(());
            };
            for (i, e) in events.iter().enumerate() {
                for fade in ["fadeIn", "fadeOut"] {
                    let Some(Value::Str(was)) = e.get(fade).and_then(|f| f.get("chain")) else {
                        continue;
                    };
                    if let Some(to) = renames.get(was) {
                        let at = [
                            PathSeg::Key("events".to_owned()),
                            PathSeg::Index(i),
                            PathSeg::Key(fade.to_owned()),
                            PathSeg::Key("chain".to_owned()),
                        ];
                        doc.set_in(&at, &Value::Str(to.clone()), WriteOptions::default())?;
                    }
                }
            }
            Ok(())
        })?;
    }
    Ok(())
}

fn rename_to(fs: &mut dyn Fs, from: &str, to: &str) -> Res<()> {
    fs.rename(from, to).map_err(|e| io_error(&e, "rename", from))
}

fn mkdirs(fs: &mut dyn Fs, dir: &str) -> Res<()> {
    fs.mkdir_all(dir).map_err(|e| io_error(&e, "mkdir", dir))
}

/// Rewrite `dir` in place; `dry_run` answers what would happen and touches nothing.
///
/// # Errors
/// A project it cannot carry over, a graph or node that does not read, or the file system's.
pub fn migrate_to_chain_files(
    fs: &mut dyn Fs,
    env: &Env,
    dir: &str,
    new_id: NewId,
    dry_run: bool,
) -> Res<FilesMigration> {
    if let Some(why) = migration_blocker(fs, dir) {
        return fail(format!("cannot migrate {dir}: {why}"));
    }
    let graph_text = ws_text(fs, &join(&[dir, GRAPH_FILE]))?;
    let parsed = parse_graph(&graph_text);
    let Some(graph_value) = parsed.value else {
        return fail(format!(
            "invalid chains:\n  {GRAPH_FILE}: {}",
            parsed.issues.first().map_or("", String::as_str)
        ));
    };
    let graph = GraphIn::from_value(&graph_value);
    let ro: &dyn Fs = &*fs;
    let modules = load_modules(ro, env, dir)?;
    let refs = load_module_refs(ro, env, dir)?;
    let resolved = resolve_all(ro, env, dir)?;
    let all = lanes_of(&resolved);
    let lanes: Vec<String> = all.sound.keys().map(|k| (*k).to_owned()).collect();
    let pictures_list: Vec<String> = all.picture.keys().map(|k| (*k).to_owned()).collect();
    drop(all);
    let lane_set: HashSet<&str> = lanes.iter().map(String::as_str).collect();
    let pictures: HashSet<String> = pictures_list.iter().cloned().collect();
    // A graph may name a module by its REF, with no node file: that node is the module itself.
    let facts = |n: &str| -> (bool, Option<String>) {
        let (io, category) = if let Some(m) = resolved.get(n) {
            (m.authored.io, m.category.clone())
        } else if modules.contains_key(n) {
            (None, None)
        } else if let Some(m) = refs.modules.get(n) {
            (m.authored.io, m.category.clone())
        } else {
            (None, None)
        };
        (
            matches!(io, Some(IoKind::In | IoKind::File | IoKind::Camera | IoKind::Midi)),
            category,
        )
    };
    let ids: RefCell<IndexMap<String, String>> = RefCell::new(IndexMap::new());
    let make_id = RefCell::new(new_id);
    let id_of = |n: &str| -> String {
        ids.borrow_mut()
            .entry(n.to_owned())
            .or_insert_with(|| (make_id.borrow_mut())())
            .clone()
    };
    let item_of = |n: &str| -> Res<Item> {
        if lane_set.contains(n) || pictures.contains(n) {
            return Ok(Value::object(vec![
                ("id".to_owned(), Value::Str(id_of(n))),
                ("track".to_owned(), Value::Str(n.to_owned())),
            ]));
        }
        let file = join(&[dir, NODES, &format!("{n}.k7x")]);
        let own = if ro.exists(&file) {
            ws_data(ro, &file)?
        } else {
            Value::Map(vec![])
        };
        let preset = format!("{n}.vstpreset");
        let mut item = Value::object(vec![("id".to_owned(), Value::Str(id_of(n)))]);
        if ro.exists(&join(&[dir, NODES, &preset])) {
            set_key(&mut item, "vstpreset", Value::Str(preset));
        }
        // …written as an item extending that ref, which is what the graph meant by naming it.
        if !modules.contains_key(n) && refs.modules.contains_key(n) && !truthy(own.get("extends")) {
            set_key(&mut item, "extends", Value::Str(n.to_owned()));
        }
        if let Value::Map(m) = &own {
            for (k, v) in m {
                set_key(&mut item, k, v.clone());
            }
        }
        Ok(item)
    };
    let is_source = |n: &str| -> Res<bool> {
        let (io, category) = facts(n);
        // A SHAPE is a picture source too, drawn rather than filmed.
        Ok(lane_set.contains(n) || pictures.contains(n) || io || category.as_deref() == Some(SHAPE_CATEGORY))
    };
    let is_pass = |n: &str| -> Res<bool> { Ok(matches!(facts(n).1.as_deref(), Some("shaders" | "video_effects"))) };
    let mut named: HashSet<&str> = graph.modules.keys().map(String::as_str).collect();
    named.extend(
        graph
            .modules
            .values()
            .flatten()
            .map(|d| d.split('#').next().unwrap_or(d)),
    );
    let loose: Vec<String> = modules
        .keys()
        .filter(|n| !named.contains(n.as_str()))
        .cloned()
        .collect();
    // A new chain is named after the leaf of what its head extends, as a graph edit names one.
    let name_for = |head: &str, taken: &HashSet<String>| -> String {
        let ext = if lane_set.contains(head) || pictures.contains(head) {
            None
        } else {
            item_of(head).ok().and_then(|it| match it.get("extends") {
                Some(Value::Str(s)) => Some(s.clone()),
                _ => None,
            })
        };
        let from = ext.as_deref().unwrap_or(head);
        let mut base = sanitized(from.rsplit('/').next().unwrap_or(from));
        if base.is_empty() {
            base = "chain".to_owned();
        }
        let mut n = base.clone();
        let mut i = 2;
        while taken.contains(&n) {
            n = format!("{base}-{i}");
            i += 1;
        }
        n
    };
    let levels = IndexMap::new();
    let is_picture_lane = |n: &str| pictures.contains(n);
    let chains = from_segments(
        &graph,
        &FromSegments {
            item_of: &item_of,
            is_source: &is_source,
            prev: None,
            levels: &levels,
            name_for: Some(&name_for),
            loose: &loose,
            reserved: &[],
            is_picture_pass: Some(&is_pass),
            is_picture_lane: Some(&is_picture_lane),
        },
    )?;
    let ids = ids.into_inner();
    // In memory a non-lane node is its id now, so the layout follows it there.
    let positions = match graph_value.get("positions") {
        Some(Value::Map(m)) => Value::Map(
            m.iter()
                .map(|(k, v)| {
                    let key = match ids.get(k) {
                        Some(id) if !lane_set.contains(k.as_str()) && !pictures.contains(k) => id.clone(),
                        _ => k.clone(),
                    };
                    (key, v.clone())
                })
                .collect(),
        ),
        _ => Value::Map(vec![]),
    };
    let result = FilesMigration {
        ids: ids.clone(),
        loose,
        chains: chains.keys().cloned().collect(),
    };
    if dry_run {
        return Ok(result);
    }
    mkdirs(fs, &join(&[dir, CHAINS_DIR]))?;
    for (name, c) in &chains {
        ws_write_text(fs, &chain_file_path(dir, name), &format_chain(c, "")?)?;
    }
    write_layout(fs, dir, &positions)?;
    rename_stretches(fs, dir, &chains, &ids, &pictures)?;
    mkdirs(fs, &join(&[dir, TRACKS_DIR]))?;
    for n in lanes.iter().chain(&pictures_list) {
        let from = join(&[dir, NODES, &format!("{n}.k7x")]);
        if fs.exists(&from) {
            rename_to(fs, &from, &join(&[dir, TRACKS_DIR, &format!("{n}.k7x")]))?;
        }
    }
    let nodes = join(&[dir, NODES]);
    for (f, _) in names_of(fs, &nodes) {
        if f.ends_with(".vstpreset") {
            mkdirs(fs, &join(&[dir, VSTPRESETS_DIR]))?;
            rename_to(fs, &join(&[&nodes, &f]), &join(&[dir, VSTPRESETS_DIR, &f]))?;
        } else if f == ASSETS_DIR || f == "base" {
            let to = if f == ASSETS_DIR {
                join(&[dir, ASSETS_DIR])
            } else {
                join(&[dir, VSTPRESETS_DIR, "base"])
            };
            mkdirs(fs, &to)?;
            let from = join(&[&nodes, &f]);
            for (a, _) in names_of(fs, &from) {
                rename_to(fs, &join(&[&from, &a]), &join(&[&to, &a]))?;
            }
        }
    }
    fs.remove_tree(&nodes).map_err(|e| io_error(&e, "rm", &nodes))?;
    let graph_file = join(&[dir, GRAPH_FILE]);
    fs.remove(&graph_file).map_err(|e| io_error(&e, "rm", &graph_file))?;
    Ok(result)
}
