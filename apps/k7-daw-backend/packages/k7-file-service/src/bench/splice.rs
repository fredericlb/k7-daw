//! bench/splice.rs — a library chain COPIED between the rig's input and its fader (`spliceChain`), never referenced.
//! What a reader gets wrong: a folder chain keeps its node names and files whole, a file chain's nodes are named afresh from the module each extends.

use std::collections::HashSet;

use indexmap::IndexMap;
use k7x::edit::{PathSeg, StringifyOptions, Value};
use song::templates::Layout;
use song_edit::Fs;
use song_edit::chain_data::{Chain, Step};
use song_edit::graph_file::format_chains;
use song_edit::graph_schema::{GRAPH_FILE, parse_graph};
use song_edit::scaffold_live::{LIVE_FADER, LIVE_INPUT};
use song_edit::workspace_doc::{ws_data, ws_text, ws_write_data, ws_write_text};
use song_edit::workspace_file::ws_copy;
use song_edit::yaml_edit::set_file_value;

use super::BenchError;
use crate::chains::cold_run::{DEFAULT_LIBRARY, library_ref_parts};
use crate::chains::{Chains, refusal_at};
use crate::paths::join;

/// The rig's own node names, which a chain's nodes must not reuse once copied beside them.
const RIG_NODES: [&str; 4] = [LIVE_INPUT, LIVE_FADER, "MIX", "OUT"];
/// The keys of a node item that are not the module's own.
const NODE_ITEM_KEYS: [&str; 4] = ["id", "bypass", "vstpreset", "mapping"];

/// What a splice copied: the chain's ref and each bench node's source step id, in the order the nodes run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spliced {
    pub ref_: String,
    pub ids: IndexMap<String, String>,
}

/// `chainRefParts` and the walk: the chain `ref` names, bare or with its library.
pub fn find_chain(c: &Chains<'_>, ref_: &str) -> Option<usize> {
    let trimmed = ref_.trim().trim_end_matches('/');
    let (library, path) = match library_ref_parts(trimmed) {
        Some((l, r)) => (l.to_owned(), r.trim_start_matches('/').to_owned()),
        None if trimmed.starts_with('@') => return None,
        None => (
            DEFAULT_LIBRARY.to_owned(),
            trimmed.trim_start_matches('/').to_owned(),
        ),
    };
    // A ref is a path under the family folder: an empty, `.` or `..` segment would leave the library.
    if path
        .split('/')
        .any(|s| s.is_empty() || s == "." || s == "..")
    {
        return None;
    }
    (0..c.dirs.len()).find(|&i| c.dirs[i].library == library && c.dirs[i].path == path)
}

/// A bench name for a file chain's node, which has only an id: the module it extends, made unique.
fn bench_name(extends: Option<&str>, taken: &mut HashSet<String>) -> String {
    let last = extends.unwrap_or("").rsplit('/').next().unwrap_or("");
    let mut base = String::new();
    let mut gap = false;
    for ch in last.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            base.push(ch);
            gap = false;
        } else if !gap {
            base.push('-');
            gap = true;
        }
    }
    if base.is_empty() {
        base = "node".to_owned();
    }
    let mut name = base.clone();
    let mut i = 2;
    while taken.contains(&name) {
        name = format!("{base}-{i}");
        i += 1;
    }
    taken.insert(name.clone());
    name
}

fn strs(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::List(l)) => l
            .iter()
            .filter_map(|x| {
                if let Value::Str(s) = x {
                    Some(s.clone())
                } else {
                    None
                }
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// A file chain's node items as raw data, in step order, each reduced to the module's own fields.
fn raw_modules(fs: &dyn Fs, at: &str) -> Result<(Vec<Value>, Option<Value>), BenchError> {
    let chain = Chain::from_value(&ws_data(fs, at)?);
    let mut out = Vec::new();
    for step in &chain.steps {
        if let Step::One(Value::Map(kv)) = step {
            let fields = kv
                .iter()
                .filter(|(k, _)| !NODE_ITEM_KEYS.contains(&k.as_str()));
            out.push(Value::Map(fields.cloned().collect()));
        }
    }
    Ok((out, chain.input))
}

/// Put the chain `ref_` of the library between the bench's input and its fader; refused, with the reason, when it cannot run live.
///
/// # Errors
/// A chain that is not there or cannot run live, a rig that does not read, or the file system's.
pub fn splice_chain(
    fs: &mut dyn Fs,
    bench_dir: &str,
    c: &Chains<'_>,
    ref_: &str,
) -> Result<Spliced, BenchError> {
    let Some(i) = find_chain(c, ref_) else {
        return Err(BenchError::Refused(format!(
            "no chain \"{ref_}\" in the library"
        )));
    };
    if let Some(why) = refusal_at(c, i)? {
        return Err(BenchError::Refused(format!(
            "the chain \"{ref_}\" cannot run live: {why}"
        )));
    }
    let graph = join(&[bench_dir, GRAPH_FILE]);
    let rig = parse_graph(&ws_text(fs, &graph)?).value.ok_or_else(|| {
        BenchError::Refused("the bench has a graph that does not read".to_owned())
    })?;
    let dir = &c.dirs[i];
    let template = c.template(i)?;
    let steps = &template.steps;
    let (raw, input) = if dir.layout == Layout::File {
        raw_modules(fs, &dir.at)?
    } else {
        (Vec::new(), None)
    };
    if dir.layout == Layout::File && raw.len() != steps.len() {
        return Err(BenchError::Refused(
            "the chain changed while it was read".to_owned(),
        ));
    }
    if let Some(input) = &input {
        let in_file = join(&[bench_dir, "nodes", &format!("{LIVE_INPUT}.k7x")]);
        if let Some(channel) = input.get("channel") {
            set_file_value(fs, &in_file, &[PathSeg::Key("channel".into())], channel)?;
        }
        if let Some(monitor) = input.get("monitor") {
            set_file_value(fs, &in_file, &[PathSeg::Key("monitor".into())], monitor)?;
        }
    }
    let rig_modules: Vec<(String, Vec<String>)> = match rig.get("modules") {
        Some(Value::Map(m)) => m.iter().map(|(k, v)| (k.clone(), strs(Some(v)))).collect(),
        _ => Vec::new(),
    };
    let mut taken: HashSet<String> = RIG_NODES.iter().map(|s| (*s).to_owned()).collect();
    taken.extend(rig_modules.iter().map(|(k, _)| k.clone()));
    let names: Vec<String> = steps
        .iter()
        .map(|s| {
            if dir.layout == Layout::Folder {
                s.name.clone()
            } else {
                bench_name(s.module.extends.as_deref(), &mut taken)
            }
        })
        .collect();
    let mut modules: Vec<(String, Vec<String>)> = rig_modules
        .into_iter()
        .filter(|(n, _)| n != LIVE_INPUT)
        .collect();
    // No module: the input runs straight into the fader, as the bare rig's does.
    modules.push((
        LIVE_INPUT.to_owned(),
        vec![names.first().map_or(LIVE_FADER, String::as_str).to_owned()],
    ));
    for (k, n) in names.iter().enumerate() {
        let next = names.get(k + 1).map_or(LIVE_FADER, String::as_str);
        modules.push((n.clone(), vec![next.to_owned()]));
    }
    let nodes = join(&[bench_dir, "nodes"]);
    fs.mkdir_all(&nodes)
        .map_err(|e| BenchError::Refused(e.to_string()))?;
    for (k, s) in steps.iter().enumerate() {
        let at = join(&[&nodes, &names[k]]);
        if dir.layout == Layout::Folder {
            ws_copy(
                fs,
                &join(&[&dir.at, "nodes", &format!("{}.k7x", s.name)]),
                &format!("{at}.k7x"),
            )?;
        } else {
            let opts = StringifyOptions {
                header: &[],
                inline: None,
            };
            ws_write_data(fs, &format!("{at}.k7x"), &raw[k], &opts)?;
        }
        if let Some(preset) = &s.preset {
            fs.copy(preset, &format!("{at}.vstpreset"))
                .map_err(|e| BenchError::Refused(e.to_string()))?;
        }
    }
    let mut bypassed = strs(rig.get("bypassed"));
    bypassed.extend(
        names
            .iter()
            .zip(steps)
            .filter(|(_, s)| s.bypass)
            .map(|(n, _)| n.clone()),
    );
    let header = [
        format!("The bench, with the library chain \"{ref_}\" COPIED between the input and the fader."),
        "Nothing here is saved by itself: `chain save <name>` writes the run into your chains. See apps/k7/backend-rs/docs/banc.md.".to_owned(),
    ];
    ws_write_text(fs, &graph, &format_chains(&modules, &bypassed, &header)?)?;
    Ok(Spliced {
        ref_: dir.ref_.clone(),
        ids: names
            .iter()
            .cloned()
            .zip(steps.iter().map(|s| s.name.clone()))
            .collect(),
    })
}
