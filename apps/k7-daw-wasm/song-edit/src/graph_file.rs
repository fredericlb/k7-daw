// graph_file.rs — a project's graph EDITED: bypass a module, insert one into an arrow, remove one, rename one.
// A reader would get wrong that every edit is on the EXPANDED data and put back through the runs short form's rule.

use crate::error::{Res, fail};
use crate::graph_schema::{Data, GRAPH_FILE, PORT_SEP, check};
use crate::sugar::sugar_for;
use k7x::edit::{PathSeg, Scope, StringifyOptions, Value, apply_sugared, stringify};

/// The graph a writer edits, refused early rather than written over when it does not read.
fn read(doc: &Scope) -> Res<Data> {
    let raw = doc.to_js()?.unwrap_or(Value::Map(vec![]));
    check(&raw).or_else(|m| fail(format!("{GRAPH_FILE}: {m}")))
}

/// Write the edited expanded data back, a runs short form kept while its expansion still holds.
fn write(doc: &mut Scope, after: Data) -> Res<()> {
    let value = Value::Map(after);
    if doc.base().is_empty() {
        Ok(apply_sugared(
            doc.document(),
            &value,
            sugar_for(GRAPH_FILE).as_ref().map(|s| s as &dyn k7x::edit::Sugar),
        )?)
    } else {
        Ok(doc.apply_in(&[], &value)?)
    }
}

fn get<'a>(d: &'a Data, k: &str) -> Option<&'a Value> {
    d.iter().find(|(x, _)| x == k).map(|(_, v)| v)
}

/// `{...d, k: v}`: the key keeps its place when the data holds it, else goes last.
fn with(d: &Data, k: &str, v: Value) -> Data {
    let mut out = d.clone();
    match out.iter_mut().find(|(x, _)| x == k) {
        Some(slot) => slot.1 = v,
        None => out.push((k.to_owned(), v)),
    }
    out
}

fn without(d: &Data, k: &str) -> Data {
    d.iter().filter(|(x, _)| x != k).cloned().collect()
}

fn list(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::List(l)) => l
            .iter()
            .filter_map(|x| if let Value::Str(s) = x { Some(s.clone()) } else { None })
            .collect(),
        _ => vec![],
    }
}

fn strs(v: &[String]) -> Value {
    Value::List(v.iter().map(|s| Value::Str(s.clone())).collect())
}

fn modules_of(d: &Data) -> Vec<(String, Value)> {
    match get(d, "modules") {
        Some(Value::Map(m)) => m.clone(),
        _ => vec![],
    }
}

fn dests_of(d: &Data, node: &str) -> Option<Vec<String>> {
    modules_of(d)
        .iter()
        .find(|(k, _)| k == node)
        .map(|(_, v)| list(Some(v)))
}

fn has_node(d: &Data, node: &str) -> bool {
    modules_of(d).iter().any(|(k, _)| k == node)
}

/// The data with one node's destinations set, the `modules` map created when missing.
fn with_dests(d: &Data, node: &str, to: &[String]) -> Data {
    let mut m = modules_of(d);
    match m.iter_mut().find(|(k, _)| k == node) {
        Some(slot) => slot.1 = strs(to),
        None => m.push((node.to_owned(), strs(to))),
    }
    with(d, "modules", Value::object(m))
}

fn without_bypass(d: &Data, module: &str) -> Data {
    let left: Vec<String> = list(get(d, "bypassed")).into_iter().filter(|m| m != module).collect();
    let rest = without(d, "bypassed");
    if left.is_empty() {
        rest
    } else {
        with(&rest, "bypassed", strs(&left))
    }
}

fn without_position(d: &Data, node: &str) -> Data {
    let Some(Value::Map(p)) = get(d, "positions") else {
        return d.clone();
    };
    if !p.iter().any(|(k, _)| k == node) {
        return d.clone();
    }
    let left: Vec<(String, Value)> = p.iter().filter(|(k, _)| k != node).cloned().collect();
    let rest = without(d, "positions");
    if left.is_empty() {
        rest
    } else {
        with(&rest, "positions", Value::Map(left))
    }
}

/// Put `module` in the graph wired to nothing: an empty list is what a PLACED module looks like.
///
/// # Errors
/// A graph that does not read, or the write's.
pub fn add_node(doc: &mut Scope, module: &str) -> Res<()> {
    let file = read(doc)?;
    if has_node(&file, module) {
        return Ok(());
    }
    write(doc, with_dests(&file, module, &[]))
}

/// Bypass (or re-enable) one module; the list disappears when it empties, so the round trip is a no-op.
///
/// # Errors
/// A graph that does not read, or a module it does not name.
pub fn set_module_enabled(doc: &mut Scope, module: &str, enabled: bool) -> Res<()> {
    let file = read(doc)?;
    if !has_node(&file, module) {
        return fail(format!("{GRAPH_FILE} has no module \"{module}\""));
    }
    let listed = list(get(&file, "bypassed")).iter().any(|m| m == module);
    if enabled == !listed {
        return Ok(());
    }
    if enabled {
        return write(doc, without_bypass(&file, module));
    }
    let mut b = list(get(&file, "bypassed"));
    b.push(module.to_owned());
    write(doc, with(&file, "bypassed", strs(&b)))
}

/// Insert `module` into the arrow from `before` to `after`: one arrow, one rewire.
///
/// # Errors
/// A node the graph lacks, an arrow that is not there, or a source that already feeds something.
pub fn insert_between(doc: &mut Scope, before: &str, after: Option<&str>, module: &str) -> Res<()> {
    let file = read(doc)?;
    if !has_node(&file, before) {
        return fail(format!("{GRAPH_FILE} has no node \"{before}\""));
    }
    let mut to = dests_of(&file, before).unwrap_or_default();
    let nothing = |d: &[String]| {
        if d.is_empty() {
            "nothing".to_owned()
        } else {
            d.join(", ")
        }
    };
    match after.filter(|a| !a.is_empty()) {
        Some(a) => {
            let Some(at) = to.iter().position(|d| d == a) else {
                return fail(format!("'{before}' does not feed '{a}' (it feeds: {})", nothing(&to)));
            };
            to[at] = module.to_owned();
        }
        None => {
            if !to.is_empty() {
                return fail(format!(
                    "'{before}' already feeds {} — name which one to insert before",
                    to.join(", ")
                ));
            }
            to.push(module.to_owned());
        }
    }
    let next = with_dests(&file, before, &to);
    let onward: Vec<String> = after.filter(|a| !a.is_empty()).map(str::to_owned).into_iter().collect();
    write(doc, with_dests(&next, module, &onward))
}

/// JS `Math.round`.
fn round(x: f64) -> f64 {
    (x + 0.5).floor()
}

/// Pin (or, with None, release) where `node` sits: whole px.
///
/// # Errors
/// A graph that does not read, or the write's.
pub fn set_node_position(doc: &mut Scope, node: &str, at: Option<(f64, f64)>) -> Res<()> {
    let file = read(doc)?;
    let Some((x, y)) = at else {
        return write(doc, without_position(&file, node));
    };
    let mut p = match get(&file, "positions") {
        Some(Value::Map(m)) => m.clone(),
        _ => vec![],
    };
    let pinned = Value::Map(vec![
        ("x".to_owned(), Value::Num(round(x))),
        ("y".to_owned(), Value::Num(round(y))),
    ]);
    match p.iter_mut().find(|(k, _)| k == node) {
        Some(slot) => slot.1 = pinned,
        None => p.push((node.to_owned(), pinned)),
    }
    write(doc, with(&file, "positions", Value::object(p)))
}

/// Draw a NEW arrow from `from` to `to`: a patch cable rather than a splice.
///
/// # Errors
/// A graph that does not read, or an arrow already there.
pub fn connect_nodes(doc: &mut Scope, from: &str, to: &str) -> Res<()> {
    let file = read(doc)?;
    let mut d = dests_of(&file, from).unwrap_or_default();
    if d.iter().any(|x| x == to) {
        return fail(format!("'{from}' already feeds '{to}'"));
    }
    d.push(to.to_owned());
    write(doc, with_dests(&file, from, &d))
}

/// A whole SEGMENT SET to add in one write.
#[derive(Default)]
pub struct Segments {
    pub modules: Vec<(String, Vec<String>)>,
    pub bypassed: Vec<String>,
    pub bus_only: Vec<String>,
}

/// Add a whole SEGMENT SET in one write: what instancing an assembly's template emits; a node already named is refused.
///
/// # Errors
/// A graph that does not read, or a node already in it.
pub fn merge_segments(doc: &mut Scope, add: &Segments) -> Res<()> {
    let mut file = read(doc)?;
    for (node, _) in &add.modules {
        if has_node(&file, node) {
            return fail(format!("'{node}' is already in the graph"));
        }
    }
    let mut m = modules_of(&file);
    for (node, to) in &add.modules {
        m.push((node.clone(), strs(to)));
    }
    file = with(&file, "modules", Value::object(m));
    for (key, names) in [("bypassed", &add.bypassed), ("bus_only", &add.bus_only)] {
        if names.is_empty() {
            continue;
        }
        let mut have = list(get(&file, key));
        let known = have.clone();
        have.extend(names.iter().filter(|n| !known.contains(n)).cloned());
        file = with(&file, key, strs(&have));
    }
    write(doc, file)
}

/// Take ONE arrow out; a source left feeding nothing keeps its key as `[]`.
///
/// # Errors
/// A graph that does not read, or an arrow that is not there.
pub fn disconnect_nodes(doc: &mut Scope, from: &str, to: &str) -> Res<()> {
    let file = read(doc)?;
    let d = dests_of(&file, from).unwrap_or_default();
    if !d.iter().any(|x| x == to) {
        let fed = if d.is_empty() {
            "nothing".to_owned()
        } else {
            d.join(", ")
        };
        return fail(format!("'{from}' does not feed '{to}' (it feeds: {fed})"));
    }
    write(
        doc,
        with_dests(&file, from, &d.into_iter().filter(|x| x != to).collect::<Vec<_>>()),
    )
}

/// Rejoin what fed `module` to what it fed: the data, and the nodes repointed.
fn splice_out(file: &Data, module: &str) -> (Data, Vec<String>) {
    let onward = dests_of(file, module).unwrap_or_default();
    let mut rewired = vec![];
    let mut next = file.clone();
    for (node, v) in modules_of(file) {
        let d = list(Some(&v));
        if node == module || !d.iter().any(|x| x == module) {
            continue;
        }
        // The module's own destinations take its place, deduped: two branches must not list the survivor twice.
        let mut out: Vec<String> = vec![];
        for x in &d {
            let swapped: Vec<&String> = if x == module { onward.iter().collect() } else { vec![x] };
            for r in swapped {
                if r != module && !out.contains(r) {
                    out.push(r.clone());
                }
            }
        }
        next = with_dests(&next, &node, &out);
        rewired.push(node);
    }
    (next, rewired)
}

/// Take `module` out of the signal path and LEAVE IT IN THE GRAPH, wired to nothing; the nodes repointed.
///
/// # Errors
/// A graph that does not read, or the write's.
pub fn unlink_node(doc: &mut Scope, module: &str) -> Res<Vec<String>> {
    let (file, rewired) = splice_out(&read(doc)?, module);
    write(doc, with_dests(&file, module, &[]))?;
    Ok(rewired)
}

/// Take `module` out ENTIRELY: the entry, its bypass and its pin; the nodes repointed.
///
/// # Errors
/// A graph that does not read, or the write's.
pub fn remove_node(doc: &mut Scope, module: &str) -> Res<Vec<String>> {
    let (file, rewired) = splice_out(&read(doc)?, module);
    let modules: Vec<(String, Value)> = modules_of(&file).into_iter().filter(|(k, _)| k != module).collect();
    let next = with(&file, "modules", Value::Map(modules));
    write(doc, without_position(&without_bypass(&next, module), module))?;
    Ok(rewired)
}

/// Pin MANY nodes in ONE write, leaving existing pins alone.
///
/// # Errors
/// A graph that does not read, or the write's.
pub fn set_node_positions(doc: &mut Scope, at: &[(String, (f64, f64))]) -> Res<()> {
    if at.is_empty() {
        return Ok(());
    }
    let file = read(doc)?;
    let mut p = match get(&file, "positions") {
        Some(Value::Map(m)) => m.clone(),
        _ => vec![],
    };
    for (node, (x, y)) in at {
        if !p.iter().any(|(k, _)| k == node) {
            p.push((
                node.clone(),
                Value::Map(vec![
                    ("x".to_owned(), Value::Num(round(*x))),
                    ("y".to_owned(), Value::Num(round(*y))),
                ]),
            ));
        }
    }
    write(doc, with(&file, "positions", Value::object(p)))
}

/// Drop the whole `positions:` section: `graph organize`'s write.
///
/// # Errors
/// A graph that does not read, or the write's.
pub fn clear_node_positions(doc: &mut Scope) -> Res<()> {
    write(doc, without(&read(doc)?, "positions"))
}

/// `splitPort`: only the LAST `#` is the port.
fn split_port(r: &str) -> (&str, Option<&str>) {
    match r.rfind(PORT_SEP) {
        Some(cut) if cut > 0 && cut + 1 < r.len() => (&r[..cut], Some(&r[cut + 1..])),
        _ => (r, None),
    }
}

fn follow(r: &str, from: &str, to: &str) -> String {
    match split_port(r) {
        (node, Some(p)) if node == from => format!("{to}{PORT_SEP}{p}"),
        (node, None) if node == from => to.to_owned(),
        _ => r.to_owned(),
    }
}

fn rename_list(v: &Value, from: &str, to: &str) -> Value {
    match v {
        Value::List(l) => Value::List(
            l.iter()
                .map(|x| {
                    if let Value::Str(s) = x {
                        Value::Str(follow(s, from, to))
                    } else {
                        x.clone()
                    }
                })
                .collect(),
        ),
        o => o.clone(),
    }
}

/// Rename `from` to `to` everywhere, IN PLACE so the node keeps its place in the file; the nodes repointed.
///
/// # Errors
/// A graph that does not read, or the write's.
pub fn rename_node(doc: &mut Scope, from: &str, to: &str) -> Res<Vec<String>> {
    let file = read(doc)?;
    let rewired: Vec<String> = modules_of(&file)
        .into_iter()
        .filter(|(_, v)| list(Some(v)).iter().any(|d| follow(d, from, to) != *d))
        .map(|(k, _)| k)
        .collect();
    // The keys first, renamed where they stand, so the entry and its comments keep their line.
    let raw = doc.to_js()?.unwrap_or(Value::Map(vec![]));
    for section in ["modules", "positions"] {
        let Some(Value::Map(m)) = raw.get(section) else {
            continue;
        };
        for (key, _) in m {
            let renamed = follow(key, from, to);
            if renamed != *key {
                doc.rename_key(&[PathSeg::Key(section.to_owned()), PathSeg::Key(key.clone())], &renamed)?;
            }
        }
    }
    // Then every ref the RAW data holds, runs included: renaming the short form keeps it.
    let Some(Value::Map(now)) = doc.to_js()? else {
        return Ok(rewired);
    };
    let mut next: Vec<(String, Value)> = now.clone();
    let mut set = |k: &str, v: Value| {
        if let Some(slot) = next.iter_mut().find(|(x, _)| x == k) {
            slot.1 = v;
        }
    };
    if let Some(Value::Map(m)) = now.iter().find(|(k, _)| k == "modules").map(|(_, v)| v) {
        set(
            "modules",
            Value::object(m.iter().map(|(k, d)| (k.clone(), rename_list(d, from, to))).collect()),
        );
    }
    if let Some(Value::List(runs)) = now.iter().find(|(k, _)| k == "runs").map(|(_, v)| v) {
        set(
            "runs",
            Value::List(runs.iter().map(|r| rename_list(r, from, to)).collect()),
        );
    }
    for key in ["bypassed", "bus_only"] {
        if let Some(l @ Value::List(_)) = now.iter().find(|(k, _)| k == key).map(|(_, v)| v) {
            set(key, rename_list(l, from, to));
        }
    }
    doc.apply_in(&[], &Value::Map(next))?;
    Ok(rewired)
}

/// A whole segments file in canonical k7x form, `header` its `//` lines: what the scaffold writes.
///
/// # Errors
/// A value that does not print.
pub fn format_chains(modules: &[(String, Vec<String>)], bypassed: &[String], header: &[String]) -> Res<String> {
    let mut out: Vec<(String, Value)> = vec![];
    if !modules.is_empty() {
        out.push((
            "modules".to_owned(),
            Value::object(modules.iter().map(|(k, v)| (k.clone(), strs(v))).collect()),
        ));
    }
    if !bypassed.is_empty() {
        out.push(("bypassed".to_owned(), strs(bypassed)));
    }
    Ok(stringify(&Value::Map(out), &StringifyOptions { header, inline: None })?)
}
