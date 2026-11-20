// assembly_edit.rs — making a box, taking one apart, and keeping its record honest when a node is renamed or deleted.
// A reader would get wrong what instancing writes: ordinary nodes and graph segments, plus one record saying which of them are the box.

use crate::chain_item_edit::is_chain_project;
use crate::confine::Env;
use crate::error::{Error, Res, fail};
use crate::fs::Fs;
use crate::graph_file::{Segments, merge_segments, set_node_position};
use crate::graph_schema::{GRAPH_FILE, parse_graph};
use crate::loader::{NODES_DIR, roots_for};
use crate::module_io::entity_files;
use crate::node_files::copy_node_files;
use crate::paths::join;
use crate::workspace_doc::{EditOpts, ws_edit, ws_text, ws_write_text};
use k7x::edit::{Document, PathSeg, StringifyOptions, Value, WriteOptions, stringify};
use song::assemblies::{ASSEMBLIES_DIR, Loaded, ProjectAssembly, decode_assembly, load_template, project_assemblies};
use song::{Cache, routing::split_port};

fn file_for(dir: &str, id: &str) -> String {
    join(&[dir, ASSEMBLIES_DIR, &format!("{id}.k7x")])
}

/// The name a member takes in the project: the box's, then the template's own word for it.
#[must_use]
pub fn member_name(assembly: &str, member: &str) -> String {
    format!("{assembly}_{member}")
}

fn src(e: song::SourceError) -> Error {
    Error::Msg(e.to_string())
}

fn strings(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::List(l)) => l
            .iter()
            .filter_map(|x| if let Value::Str(s) = x { Some(s.clone()) } else { None })
            .collect(),
        _ => vec![],
    }
}

/// Instance a template into this project: its node files, its segments, and the record.
///
/// # Errors
/// A chain project, a bad or taken name, a template that is missing, empty or unreadable, or a node already in the graph.
pub fn create_assembly(
    fs: &mut dyn Fs,
    env: &Env,
    dir: &str,
    template: &str,
    id: &str,
    at: Option<(f64, f64)>,
) -> Res<(String, Vec<(String, String)>)> {
    if is_chain_project(fs, dir) {
        return fail("a chain project cannot hold assemblies yet — a box names its nodes, and these are known by id");
    }
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return fail(format!(
            "invalid assembly name '{id}' — letters, digits, '_' and '-' only"
        ));
    }
    if fs.exists(&file_for(dir, id)) {
        return fail(format!("assembly '{id}' already exists"));
    }
    let roots = roots_for(fs, env, dir);
    let folder = match load_template(fs, &mut Cache::new(), &roots, template).map_err(src)? {
        Loaded::Template { folder, .. } => folder,
        Loaded::Missing => {
            return fail(format!(
                "no assembly template '{template}' in the account's libraries (a library's {ASSEMBLIES_DIR}/, named @<library>/<path>)"
            ));
        }
    };
    let files = entity_files(fs, &join(&[&folder, NODES_DIR]), NODES_DIR);
    if files.is_empty() {
        return fail(format!("assembly template '{template}' holds no nodes/ to instance"));
    }
    let members: Vec<(String, String)> = files.iter().map(|(m, _)| (m.clone(), member_name(id, m))).collect();
    let graph_path = join(&[dir, GRAPH_FILE]);
    let mut graph = Document::new(&ws_text(fs, &graph_path)?);
    let taken: Vec<String> = match parse_graph(graph.text()).value.as_ref().and_then(|g| g.get("modules")) {
        Some(Value::Map(m)) => m.iter().map(|(k, _)| k.clone()).collect(),
        _ => vec![],
    };
    for (member, node) in &members {
        if taken.contains(node) {
            return fail(format!(
                "'{node}' is already in the graph — rename the box, or the member '{member}'"
            ));
        }
    }
    let local = |r: &str| -> String {
        let (node, port) = split_port(r);
        match members.iter().find(|(m, _)| m == node) {
            None => r.to_owned(),
            Some((_, mapped)) => port.map_or_else(|| mapped.clone(), |p| format!("{mapped}#{p}")),
        }
    };
    let source = parse_graph(&ws_text(fs, &join(&[&folder, GRAPH_FILE]))?);
    let Some(source_value) = source.value else {
        return fail(format!(
            "assembly template '{template}': its {GRAPH_FILE} will not read — {}",
            source.issues.first().map_or("unreadable", String::as_str)
        ));
    };
    let mut modules: Vec<(String, Vec<String>)> = match source_value.get("modules") {
        Some(Value::Map(m)) => m
            .iter()
            .map(|(k, v)| (local(k), strings(Some(v)).iter().map(|d| local(d)).collect()))
            .collect(),
        _ => vec![],
    };
    for (_, node) in &members {
        if !modules.iter().any(|(k, _)| k == node) {
            modules.push((node.clone(), vec![]));
        }
    }
    fs.mkdir_all(&join(&[dir, NODES_DIR])).map_err(src)?;
    for (member, file) in &files {
        let to = members
            .iter()
            .find(|(m, _)| m == member)
            .map_or("", |(_, n)| n.as_str());
        copy_node_files(fs, &file.path, &folder, member, dir, to)?;
    }
    let add = Segments {
        modules,
        bypassed: strings(source_value.get("bypassed")).iter().map(|d| local(d)).collect(),
        bus_only: strings(source_value.get("bus_only")).iter().map(|d| local(d)).collect(),
    };
    merge_segments(&mut graph.at(&[]), &add)?;
    if let Some(p) = at {
        set_node_position(&mut graph.at(&[]), id, Some(p))?;
    }
    ws_write_text(fs, &graph_path, graph.text())?;
    fs.mkdir_all(&join(&[dir, ASSEMBLIES_DIR])).map_err(src)?;
    let record = Value::Map(vec![
        ("uses".to_owned(), Value::Str(template.to_owned())),
        (
            "members".to_owned(),
            Value::Map(
                members
                    .iter()
                    .map(|(m, n)| (m.clone(), Value::Str(n.clone())))
                    .collect(),
            ),
        ),
    ]);
    let header = [format!(
        "{id} — one {template}. Its members are ordinary nodes; this file is what draws them as a box."
    )];
    let text = stringify(
        &record,
        &StringifyOptions {
            header: &header,
            inline: None,
        },
    )?;
    ws_write_text(fs, &file_for(dir, id), &text)?;
    Ok((format!("{ASSEMBLIES_DIR}/{id}.k7x"), members))
}

fn assemblies_of(fs: &dyn Fs, dir: &str) -> Res<Vec<ProjectAssembly>> {
    Ok(project_assemblies(fs, &mut Cache::new(), dir).map_err(src)?.assemblies)
}

/// The members of a box, as node names.
///
/// # Errors
/// A box the project does not hold.
pub fn assembly_members(fs: &dyn Fs, dir: &str, id: &str) -> Res<Vec<String>> {
    match assemblies_of(fs, dir)?.into_iter().find(|a| a.id == id) {
        Some(a) => Ok(a.assembly.members.values().cloned().collect()),
        None => fail(format!("no assembly '{id}' in this project")),
    }
}

/// Drop the RECORD and nothing else: the nodes stay exactly where they are, wired as they were.
///
/// # Errors
/// A box the project does not hold.
pub fn dissolve_assembly(fs: &mut dyn Fs, dir: &str, id: &str) -> Res<String> {
    let path = file_for(dir, id);
    if !fs.exists(&path) {
        return fail(format!("no assembly '{id}' in this project"));
    }
    crate::workspace_file::ws_remove(fs, &path)?;
    Ok(format!("{ASSEMBLIES_DIR}/{id}.k7x"))
}

fn naming(fs: &dyn Fs, dir: &str, node: &str) -> Res<Vec<ProjectAssembly>> {
    Ok(assemblies_of(fs, dir)?
        .into_iter()
        .filter(|a| a.assembly.members.values().any(|n| n == node))
        .collect())
}

/// A node was RENAMED: every box holding it follows; the ids of the boxes that moved.
///
/// # Errors
/// What the document's writers fail with.
pub fn rename_node_in_assemblies(fs: &mut dyn Fs, dir: &str, from: &str, to: &str) -> Res<Vec<String>> {
    let mut touched = vec![];
    for a in naming(fs, dir, from)? {
        let mut renamed = false;
        ws_edit(fs, &join(&[dir, &a.file]), EditOpts::default(), |doc| {
            let text = doc.text().to_owned();
            if let Err(f) = decode_assembly(ASSEMBLIES_DIR, &text) {
                return fail(format!("{ASSEMBLIES_DIR}: {}", f.message));
            }
            let members = match doc.get_in(&[PathSeg::Key("members".to_owned())])? {
                Some(Value::Map(m)) => m,
                _ => vec![],
            };
            for (key, node) in members {
                if node == Value::Str(from.to_owned()) {
                    let at = [PathSeg::Key("members".to_owned()), PathSeg::Key(key)];
                    doc.set_in(&at, &Value::Str(to.to_owned()), WriteOptions::default())?;
                    renamed = true;
                }
            }
            Ok(())
        })?;
        if renamed {
            touched.push(a.id);
        }
    }
    Ok(touched)
}

/// …and a node DELETED: the member goes, and the box with it once it has no members left.
///
/// # Errors
/// What the document's writers fail with.
pub fn drop_node_from_assemblies(fs: &mut dyn Fs, dir: &str, node: &str) -> Res<Vec<String>> {
    let mut touched = vec![];
    for a in naming(fs, dir, node)? {
        let path = join(&[dir, &a.file]);
        let gone: Vec<&String> = a
            .assembly
            .members
            .iter()
            .filter(|(_, n)| *n == node)
            .map(|(m, _)| m)
            .collect();
        if gone.len() == a.assembly.members.len() {
            crate::workspace_file::ws_remove(fs, &path)?;
        } else {
            ws_edit(fs, &path, EditOpts::default(), |doc| {
                for m in &gone {
                    doc.delete_in(&[PathSeg::Key("members".to_owned()), PathSeg::Key((*m).clone())], false)?;
                }
                Ok(())
            })?;
        }
        touched.push(a.id);
    }
    Ok(touched)
}
