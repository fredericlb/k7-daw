//! bench/frozen.rs — the account's chains written (3j O10): the bench saved as one, a library chain copied in, one removed.
//! What a reader gets wrong: a step's node file goes into the chain WHOLE, only its id comes from what the bench copied; a name refused writes nothing.

use indexmap::IndexMap;
use k7x::edit::{Value, parse};
use serde_json::Value as Json;
use song::routing::{Item, Step as FileStep, decode_chain};
use song::templates::TemplateStep;
use song_edit::chain_data::{Chain, Link, Step, remove_key};
use song_edit::format_chain::format_chain;
use song_edit::graph_schema::parse_graph;
use song_edit::ids::NewId;
use song_edit::workspace_doc::ws_data;
use song_edit::workspace_file::{ws_exists, ws_list, ws_read};
use song_edit::{Fs, paths::resolve};

use super::source::{read_source, write_source};
use super::{BenchError, Libs, bench_dir, with_chains, workspace_root};
use crate::chains::cold_run::{DEFAULT_LIBRARY, js_trim, library_ref_parts};
use crate::chains::{Chains, frozen_chains};
use crate::edits::{DiskFs, StagedFs, env_of, posix, real};
use crate::libraries::{ACCOUNT_LIBRARY, Home};
use crate::paths::{is_within, join};
use crate::space::Declined;
use song::templates::Layout;
use std::sync::atomic::{AtomicBool, Ordering};

const LIVE_INPUT: &str = "IN";
const LIVE_FADER: &str = "fader";
const RIG_NODES: [&str; 4] = [LIVE_INPUT, LIVE_FADER, "MIX", "OUT"];
/// The fields a node item holds of its own use, which a module's file does not.
const NODE_ITEM_KEYS: [&str; 4] = ["id", "bypass", "vstpreset", "mapping"];

fn src(e: song::SourceError) -> BenchError {
    BenchError::Refused(e.to_string())
}

fn refused<T>(e: impl ToString) -> Result<T, BenchError> {
    Err(BenchError::Refused(e.to_string()))
}

/// A chain's name: one path segment, what its file and its ref are named after.
fn is_chain_name(n: &str) -> bool {
    let b = n.as_bytes();
    !b.is_empty()
        && b.len() <= 64
        && b[0].is_ascii_alphanumeric()
        && b[1..]
            .iter()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-'))
}

fn file_of(home: &Home, name: &str) -> String {
    join(&[&home.account_home(), "chains", &format!("{name}.k7x")])
}

/// What the bench's run came to: the steps as chain items, their presets, the ids kept, what is off the run, the input.
struct Run {
    items: Vec<Value>,
    presets: Vec<(String, String)>,
    ids: IndexMap<String, String>,
    outside: Vec<String>,
    input: Option<Value>,
}

/// An input's `monitor` read the one way the graph and the compile read it.
fn monitor_of(v: Option<&Value>) -> Option<&'static str> {
    match v {
        Some(Value::Bool(b)) => Some(if *b { "on" } else { "off" }),
        Some(Value::Str(s)) => ["off", "auto", "on"].into_iter().find(|m| m == s),
        _ => None,
    }
}

/// The bench's run from its input to its fader, as chain items: one node after another, or a refusal saying why not.
fn bench_run(
    fs: &dyn Fs,
    dir: &str,
    known: &IndexMap<String, String>,
    new_id: NewId,
) -> Result<Run, BenchError> {
    let text = ws_read(fs, &join(&[dir, "graph.k7x"]))?;
    let Some(Value::Map(graph)) = parse_graph(&text).value else {
        return refused("the bench has a graph that does not read");
    };
    let graph = Value::Map(graph);
    let list = |k: &str| -> Vec<String> {
        match graph.get(k) {
            Some(Value::List(l)) => l
                .iter()
                .filter_map(|v| {
                    if let Value::Str(s) = v {
                        Some(s.clone())
                    } else {
                        None
                    }
                })
                .collect(),
            _ => vec![],
        }
    };
    let modules = graph.get("modules").cloned().unwrap_or(Value::Map(vec![]));
    let bypassed = list("bypassed");
    let nodes = join(&[dir, "nodes"]);
    let mut run = Run {
        items: vec![],
        presets: vec![],
        ids: IndexMap::new(),
        outside: vec![],
        input: None,
    };
    let mut seen: Vec<String> = vec![];
    let mut at = LIVE_INPUT.to_owned();
    loop {
        let next = match modules.get(&at) {
            Some(Value::List(l)) => l
                .iter()
                .filter_map(|v| {
                    if let Value::Str(s) = v {
                        Some(s.clone())
                    } else {
                        None
                    }
                })
                .collect(),
            _ => vec![],
        };
        if next.len() != 1 {
            let to = if next.is_empty() {
                "nothing".to_owned()
            } else {
                next.join(" and ")
            };
            return refused(format!(
                "\"{at}\" feeds {to}: a chain is one run from {LIVE_INPUT} to {LIVE_FADER}"
            ));
        }
        let node = next[0].clone();
        if node == LIVE_FADER {
            break;
        }
        if seen.contains(&node) {
            return refused(format!("\"{node}\" loops back on itself"));
        }
        seen.push(node.clone());
        let file = join(&[&nodes, &format!("{node}.k7x")]);
        if !ws_exists(fs, &file) {
            return refused(format!("\"{node}\" has no node file"));
        }
        // An id from the bench source names a file: only a plain segment is kept.
        let id = known
            .get(&node)
            .filter(|k| is_plain_id(k))
            .cloned()
            .unwrap_or_else(&mut *new_id);
        run.ids.insert(node.clone(), id.clone());
        let mut item = vec![("id".to_owned(), Value::Str(id.clone()))];
        if let Value::Map(data) = ws_data(fs, &file)? {
            for (k, v) in data {
                match item.iter_mut().find(|(have, _)| *have == k) {
                    Some(slot) => slot.1 = v,
                    None => item.push((k, v)),
                }
            }
        }
        if bypassed.contains(&node) {
            match item.iter_mut().find(|(k, _)| k == "bypass") {
                Some(slot) => slot.1 = Value::Bool(true),
                None => item.push(("bypass".to_owned(), Value::Bool(true))),
            }
        }
        run.items.push(Value::Map(item));
        let preset = join(&[&nodes, &format!("{node}.vstpreset")]);
        if ws_exists(fs, &preset) {
            run.presets.push((id, preset));
        }
        at = node;
    }
    let mut outside: Vec<String> = ws_list(fs, &nodes)?
        .into_iter()
        .filter_map(|f| f.strip_suffix(".k7x").map(str::to_owned))
        .filter(|n| !RIG_NODES.contains(&n.as_str()) && !seen.contains(n))
        .collect();
    outside.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    run.outside = outside;
    let in_file = join(&[&nodes, &format!("{LIVE_INPUT}.k7x")]);
    let data = if ws_exists(fs, &in_file) {
        ws_data(fs, &in_file)?
    } else {
        Value::Map(vec![])
    };
    if let Some(channel) = data.get("channel").cloned() {
        let mut input = vec![("channel".to_owned(), channel)];
        if let Some(m) = monitor_of(data.get("monitor")) {
            input.push(("monitor".to_owned(), Value::Str(m.to_owned())));
        }
        run.input = Some(Value::Map(input));
    }
    if run.items.is_empty() && run.input.is_none() {
        let n = run.outside.len();
        return refused(if n > 0 {
            let (is, it) = if n == 1 {
                ("is", "it")
            } else {
                ("are", "them")
            };
            format!(
                "nothing sits between {LIVE_INPUT} and {LIVE_FADER}: {} {is} on the bench but not wired between them — insert {it} on the {LIVE_INPUT} → {LIVE_FADER} cable",
                run.outside.join(", ")
            )
        } else {
            format!(
                "nothing to keep yet: pick {LIVE_INPUT}'s input channel, or hover the {LIVE_INPUT} → {LIVE_FADER} cable and click ⊕ to insert a module"
            )
        });
    }
    Ok(run)
}

/// An id that is one path segment of letters, digits, `-` and `_`.
fn is_plain_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// The chain's file text, its steps checked in the TypeScript's order and words; the steps it holds.
fn chain_text(
    c: &Chains<'_>,
    rule: song::PathRule,
    at: &str,
    items: &[Value],
    input: Option<&Value>,
) -> Result<(String, usize), BenchError> {
    let prev = std::fs::read_to_string(at).unwrap_or_default();
    // The items keep the order they were built in: the TypeScript hands them to the formatter as they are.
    let mut steps: Vec<Step> = items.iter().cloned().map(Step::One).collect();
    steps.push(Step::End(vec![Link {
        chain: "@out".to_owned(),
        level: None,
        port: None,
        port_first: false,
    }]));
    let chain = Chain {
        input: input.cloned(),
        steps,
        ..Chain::default()
    };
    let text = format_chain(&chain, &prev).map_err(BenchError::from)?;
    let decoded = decode_chain(at, &text, rule)
        .map_err(|f| BenchError::Refused(format!("{}:{} {}", f.line, f.col, f.message)))?;
    let (mut offline, mut held) = (0, 0);
    for s in &decoded.steps {
        let FileStep::One(Item::Node(n)) = s else {
            continue;
        };
        held += 1;
        match n.module.extends.as_deref() {
            Some(r) if !c.vocab.modules.contains_key(r) => {
                return refused(format!("step {held}: no module \"{r}\""));
            }
            None if n.module.plugin.is_none() => {
                return refused(format!("step {held} names no module and no plugin"));
            }
            _ => {}
        }
        let step = TemplateStep {
            name: n.id.clone(),
            label: n.id.clone(),
            module: n.module.clone(),
            bypass: false,
            preset: None,
        };
        if c.facts(&step)?.offline {
            offline += 1;
        }
    }
    // A separator writes files, so nothing could follow it: the process verbs refuse such a chain.
    if offline > 0 && held > 1 {
        return refused(
            "a chain separates or it treats: a separating module is the chain's only step",
        );
    }
    Ok((text, held))
}

/// `writeAccountChain`: `items` written as the account's chain `name` with each preset beside it; the chain's row, as the page lists it.
fn write_account_chain(
    libs: &Libs<'_>,
    name: &str,
    items: &[Value],
    presets: &[(String, String)],
    input: Option<&Value>,
    wrote: &AtomicBool,
) -> Result<(Json, usize), BenchError> {
    if !is_chain_name(name) {
        return refused(format!(
            "\"{name}\" is not a chain name — letters, digits, '-' and '_', starting with a letter or digit"
        ));
    }
    if items.is_empty() && input.is_none() {
        return refused("a chain holds at least one step, or an input");
    }
    let (home, dir) = (libs.home, bench_dir(libs.home));
    let at = file_of(home, name);
    let (text, held) = with_chains(libs, &dir, |c, roots| {
        chain_text(c, roots.paths, &at, items, input)
    })?;
    let env = env_of(home, libs.code_root);
    // A mount is a folder that is there: a new account has none of its library yet.
    std::fs::create_dir_all(home.account_home()).map_err(|e| BenchError::Refused(e.to_string()))?;
    let reads = [env.meta_root.clone(), home.libraries.clone()];
    let mut fs = StagedFs::reading(&[home.account_home()], &reads).map_err(src)?;
    let (folder, file) = (posix(&join(&[&home.account_home(), "chains"])), posix(&at));
    fs.mkdir_all(&folder).map_err(src)?;
    fs.write_atomic(&file, text.as_bytes()).map_err(src)?;
    let set = fs.planned().map_err(BenchError::Refused)?;
    // The presets go first, keyed by id: an orphan is harmless when the chain then fails to land.
    let held_presets = join(&[&home.account_home(), "vstpresets"]);
    for (id, from) in presets {
        let to = join(&[&held_presets, &format!("{id}.vstpreset")]);
        if !is_plain_id(id) || !is_within(&held_presets, &to) {
            return refused(format!("\"{id}\" is not a step id"));
        }
        let bytes =
            std::fs::read(real(&posix(from))).map_err(|e| BenchError::Refused(e.to_string()))?;
        std::fs::create_dir_all(real(&posix(&held_presets)))
            .map_err(|e| BenchError::Refused(e.to_string()))?;
        crate::atomic::write_atomic(&real(&posix(&to)), &bytes)
            .map_err(|e| BenchError::Refused(e.to_string()))?;
    }
    crate::changes::commit::commit(&crate::atomic::Real, &set, &mut |_| {})
        .map_err(|e| BenchError::Refused(e.what))?;
    // From here the chain is written: nothing is handed back, a decline is a refusal.
    wrote.store(true, Ordering::SeqCst);
    let reference = format!("@{ACCOUNT_LIBRARY}/{name}");
    let listed = with_chains(libs, &dir, |c, _| Ok(frozen_chains(c)?))
        .map_err(|e| BenchError::Refused(format!("the chain \"{name}\" was written: {e:?}")))?;
    let row = listed["chains"]
        .as_array()
        .and_then(|l| l.iter().find(|r| r["ref"] == reference.as_str()).cloned())
        .ok_or_else(|| {
            BenchError::Refused(format!(
                "the chain \"{name}\" was written and is not listed"
            ))
        })?;
    Ok((row, held))
}

/// What a save came to: the chain's ref, its steps, and the bench's modules off the run.
pub struct Saved {
    pub reference: String,
    pub steps: usize,
    pub outside: Vec<String>,
}

/// `chain save [name]` from the folder `from` a verb runs in: only the bench saves, and a bench opened on one of the account's chains names itself.
///
/// # Errors
/// The TypeScript's sentences, or a `Declined` where the libraries cannot be read natively.
pub fn save_bench_chain(
    libs: &Libs<'_>,
    from: &str,
    name: Option<&str>,
    new_id: NewId,
    wrote: &AtomicBool,
) -> Result<Saved, BenchError> {
    let home = libs.home;
    let (ws, dir) = (workspace_root(home), bench_dir(home));
    if resolve(&posix(from)) != resolve(&posix(&dir)) {
        return refused(
            "only the bench saves a chain — open one on the bench from the Frozen chains page",
        );
    }
    let mut fs = DiskFs::new(&[&ws]).map_err(src)?;
    let source = read_source(&fs, &ws);
    let mine = format!("@{ACCOUNT_LIBRARY}/");
    let chosen = name.map(str::to_owned).or_else(|| {
        source
            .as_ref()
            .and_then(|s| s.ref_.as_deref()?.strip_prefix(&mine).map(str::to_owned))
    });
    let Some(chosen) = chosen.filter(|c| !c.is_empty()) else {
        return refused("name the chain: `chain save <name>` writes it as @account/<name>");
    };
    let known = source.map(|s| s.ids).unwrap_or_default();
    let run = bench_run(&fs, &dir, &known, new_id)?;
    let (row, steps) = write_account_chain(
        libs,
        &chosen,
        &run.items,
        &run.presets,
        run.input.as_ref(),
        wrote,
    )?;
    let reference = row["ref"].as_str().unwrap_or_default().to_owned();
    write_source(&mut fs, &ws, &dir, Some(&reference), &run.ids).map_err(BenchError::Refused)?;
    Ok(Saved {
        reference,
        steps,
        outside: run.outside,
    })
}

/// `findLibraryChain`: the chain a bare or library-qualified ref names, exactly.
fn chain_named(c: &Chains<'_>, from: &str) -> Option<usize> {
    let trimmed = js_trim(from).trim_end_matches('/');
    let parts = library_ref_parts(trimmed);
    if trimmed.starts_with('@') && parts.is_none() {
        return None;
    }
    let library = parts.map_or(DEFAULT_LIBRARY, |p| p.0);
    let path = parts.map_or(trimmed, |p| p.1).trim_start_matches('/');
    // A ref is a path under the family folder, so a `..` in it would reach out of the library.
    if !path
        .split('/')
        .all(|s| !s.is_empty() && s != "." && s != "..")
    {
        return None;
    }
    c.dirs
        .iter()
        .position(|d| d.library == library && d.path == path)
}

/// A node item of the file as its module: the item without the fields that are its use's own.
fn module_of(item: &Value) -> Vec<(String, Value)> {
    let mut out = item.clone();
    for k in NODE_ITEM_KEYS {
        remove_key(&mut out, k);
    }
    match out {
        Value::Map(m) => m,
        _ => vec![],
    }
}

type Copied = (Vec<Value>, Vec<(String, String)>, Option<Value>);

/// The steps of file chain `i` as items with fresh ids, the presets they carry, and the input it names.
fn copied_steps(c: &Chains<'_>, i: usize, new_id: NewId) -> Result<Copied, BenchError> {
    let template = c.template(i)?;
    let wrong = || BenchError::Declined(Declined::WouldThrow);
    // A folder chain, an unmigrated clone, keeps its steps in node files: declined.
    if c.dirs[i].layout == Layout::Folder {
        return Err(wrong());
    }
    let raw = std::fs::read_to_string(&c.dirs[i].at).map_err(|_| wrong())?;
    let file = parse(&raw).map_err(|_| wrong())?;
    let nodes: Vec<&Value> = match file.get("steps") {
        Some(Value::List(l)) => l
            .iter()
            .filter(|v| matches!(v, Value::Map(_)) && v.get("to").is_none())
            .collect(),
        _ => vec![],
    };
    if nodes.len() != template.steps.len() {
        return Err(wrong());
    }
    let (mut items, mut presets) = (vec![], vec![]);
    for (step, item) in template.steps.iter().zip(nodes) {
        // Fresh ids: two chains sharing an id would share their presets' file names.
        let id = new_id();
        let mut out = vec![("id".to_owned(), Value::Str(id.clone()))];
        out.extend(module_of(item));
        if step.bypass {
            match out.iter_mut().find(|(k, _)| k == "bypass") {
                Some(slot) => slot.1 = Value::Bool(true),
                None => out.push(("bypass".to_owned(), Value::Bool(true))),
            }
        }
        items.push(Value::Map(out));
        if let Some(p) = &step.preset {
            presets.push((id, p.clone()));
        }
    }
    Ok((items, presets, file.get("input").cloned()))
}

/// `copyFrozenChain`: any library chain as the account's chain `name`, every step whole with a fresh id and its preset.
///
/// # Errors
/// The TypeScript's sentences, or a `Declined` for a folder chain or one that does not read.
pub fn copy_frozen_chain(
    libs: &Libs<'_>,
    from: &str,
    name: &str,
    new_id: NewId,
    wrote: &AtomicBool,
) -> Result<Json, BenchError> {
    if !is_chain_name(name) {
        return refused(format!(
            "\"{name}\" is not a chain name — letters, digits, '-' and '_', starting with a letter or digit"
        ));
    }
    let (home, dir) = (libs.home, bench_dir(libs.home));
    let (items, presets, input) = with_chains(libs, &dir, |c, _| {
        let Some(i) = chain_named(c, from) else {
            return refused(format!("no chain \"{from}\" in the libraries"));
        };
        if std::fs::metadata(file_of(home, name)).is_ok() {
            return refused(format!("you already have a chain \"{name}\""));
        }
        copied_steps(c, i, new_id)
    })?;
    Ok(write_account_chain(libs, name, &items, &presets, input.as_ref(), wrote)?.0)
}

/// `deleteFrozenChain`: one of the account's chains removed; a library's is never this account's to remove.
///
/// # Errors
/// The sentence of a name that is none, or of a chain this account does not have.
pub fn delete_frozen_chain(home: &Home, name: &str) -> Result<(), String> {
    if !is_chain_name(name) {
        return Err(format!("\"{name}\" is not a chain name"));
    }
    let at = file_of(home, name);
    if std::fs::metadata(&at).is_err() {
        return Err(format!("no chain \"{name}\" of yours"));
    }
    std::fs::remove_file(real(&posix(&at))).map_err(|e| e.to_string())
}
