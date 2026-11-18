// rack_taps.rs — editing a rack NODE's taps: the note each sound answers to and the library sample it plays.
// A reader would get wrong where a tap is written: the node's own file (or its selected set), never the module's.

use crate::confine::Env;
use crate::error::{Res, fail};
use crate::fs::Fs;
use crate::loader::resolve_node;
use crate::module_file::current_alternative;
use crate::project_edit::edit_module_file;
use k7x::edit::{PathSeg, Scope, Value, WriteOptions};
use song::modules::{ModuleType, Rack, RackType};
use song::{Cache, routing::read_chain_project};

pub const RACK_FIRST_NOTE: f64 = 36.0;

/// What a tap changes: `sample` of `Some(None)` clears it.
#[derive(Default)]
pub struct TapChange {
    pub note: Option<f64>,
    pub sample: Option<Option<String>>,
    pub mute: Option<bool>,
}

/// Where a positioned add lands: next to a tap's pad, or on a note.
pub enum TapNear {
    Note(f64),
    Beside { symbol: String, before: bool },
}

fn key(k: &str) -> PathSeg {
    PathSeg::Key(k.to_owned())
}

fn num(n: f64) -> Value {
    Value::Num(n)
}

/// The node's effective rack taps with their notes, refused when the node is not a rack.
///
/// # Errors
/// A node the project lacks, or one that is not a rack.
pub fn kit_of(fs: &dyn Fs, env: &Env, dir: &str, node: &str) -> Res<Rack> {
    let Some(r) = resolve_node(fs, env, dir, node)? else {
        return fail(format!("no node '{node}'"));
    };
    if r.authored.module_type != Some(ModuleType::Rack) {
        return fail(format!("'{node}' is not a rack — only a rack's node has taps"));
    }
    Ok(r.authored.rack.clone().unwrap_or(Rack {
        rack_type: RackType::Samples,
        source: None,
        symbols: indexmap::IndexMap::new(),
    }))
}

fn notes_of(rack: &Rack) -> Vec<(String, f64)> {
    rack.symbols
        .iter()
        .map(|(s, v)| (s.clone(), f64::from(v.note.0)))
        .collect()
}

fn tap_base(doc: &Scope) -> Res<Vec<PathSeg>> {
    let raw = doc.to_js()?.unwrap_or(Value::Map(vec![]));
    let at = current_alternative(&raw);
    let mut p = if at > 0 {
        vec![key("alternatives"), PathSeg::Index(at - 1)]
    } else {
        vec![]
    };
    p.extend([key("rack"), key("symbols")]);
    Ok(p)
}

fn edit_taps(
    fs: &mut dyn Fs,
    env: &Env,
    dir: &str,
    node: &str,
    change: impl FnOnce(&mut Scope, &[PathSeg]) -> Res<()>,
) -> Res<String> {
    edit_module_file(fs, env, dir, node, |doc| {
        let base = tap_base(doc)?;
        change(doc, &base)
    })
}

fn at_of(base: &[PathSeg], rest: &[&str]) -> Vec<PathSeg> {
    base.iter().cloned().chain(rest.iter().map(|k| key(k))).collect()
}

/// Set one tap's note or sample on the node; the tap is made when it is not there.
///
/// # Errors
/// As `kit_of`, or the document's writers.
pub fn set_tap(fs: &mut dyn Fs, env: &Env, dir: &str, node: &str, symbol: &str, change: &TapChange) -> Res<String> {
    let kit = notes_of(&kit_of(fs, env, dir, node)?);
    let inherited = kit.iter().find(|(s, _)| s == symbol).map(|(_, n)| *n);
    edit_taps(fs, env, dir, node, |doc, base| {
        let at = at_of(base, &[symbol]);
        let note_at = at_of(base, &[symbol, "note"]);
        let first = change.note.or(inherited).unwrap_or(RACK_FIRST_NOTE);
        if !doc.has_in(&at)? {
            doc.set_in(
                &at,
                &Value::Map(vec![("note".to_owned(), num(first))]),
                WriteOptions { inline: true },
            )?;
        } else if !doc.has_in(&note_at)? {
            doc.set_in(&note_at, &num(first), WriteOptions::default())?;
        }
        if let Some(n) = change.note {
            doc.set_in(&note_at, &num(n), WriteOptions::default())?;
        }
        let sample_at = at_of(base, &[symbol, "sample"]);
        match &change.sample {
            Some(None) => {
                doc.delete_in(&sample_at, false)?;
            }
            Some(Some(s)) => doc.set_in(&sample_at, &Value::Str(s.clone()), WriteOptions::default())?,
            None => {}
        }
        // Unmuted is the default, so it is written as nothing rather than `mute: false`.
        let mute_at = at_of(base, &[symbol, "mute"]);
        match change.mute {
            Some(true) => doc.set_in(&mute_at, &Value::Bool(true), WriteOptions::default())?,
            Some(false) => {
                doc.delete_in(&mute_at, false)?;
            }
            None => {}
        }
        Ok(())
    })
}

/// What a tap may be called: one word, since it is a pad's label and a command argument.
fn tap_name_ok(s: &str) -> bool {
    let n = s.chars().count();
    (1..=24).contains(&n) && s.chars().all(|c| c.is_ascii_alphanumeric() || "_.+#-".contains(c))
}

fn bad_name(s: &str) -> String {
    format!("'{s}' is not a tap name — one word, letters, digits, _ . + # -")
}

fn free_note(used: &[f64], from: f64, step: f64) -> Option<f64> {
    let mut n = from + step;
    while (0.0..=127.0).contains(&n) {
        if !used.contains(&n) {
            return Some(n);
        }
        n += step;
    }
    None
}

/// Add a tap: `symbol` or the next free `T<n>`; on a note, beside a tap, or after the kit's highest.
///
/// # Errors
/// A taken or ill-formed name, a note out of range or in use, or an unknown neighbour.
pub fn add_tap(
    fs: &mut dyn Fs,
    env: &Env,
    dir: &str,
    node: &str,
    symbol: Option<&str>,
    near: Option<&TapNear>,
) -> Res<(String, f64)> {
    let kit = notes_of(&kit_of(fs, env, dir, node)?);
    let taken: Vec<&str> = kit.iter().map(|(s, _)| s.as_str()).collect();
    let symbol = symbol.filter(|s| !s.is_empty());
    if let Some(s) = symbol {
        if taken.contains(&s) {
            return fail(format!("'{node}' already has a tap '{s}'"));
        }
        if !tap_name_ok(s) {
            return fail(bad_name(s));
        }
    }
    let name = symbol.map_or_else(
        || {
            (taken.len() + 1..)
                .map(|i| format!("T{i}"))
                .find(|t| !taken.contains(&t.as_str()))
                .unwrap_or_default()
        },
        str::to_owned,
    );
    let notes: Vec<f64> = kit.iter().map(|(_, n)| *n).collect();
    let note = match near {
        Some(TapNear::Note(n)) => {
            if n.fract() != 0.0 || *n < 0.0 || *n > 127.0 {
                return fail(format!("{} is not a note, 0..127", k7x::edit::js_number(*n)));
            }
            if notes.contains(n) {
                return fail(format!(
                    "'{node}' already has a tap on note {}",
                    k7x::edit::js_number(*n)
                ));
            }
            *n
        }
        Some(TapNear::Beside { symbol, before }) => {
            let Some((_, at)) = kit.iter().find(|(s, _)| s == symbol) else {
                return fail(format!("'{node}' has no tap '{symbol}'"));
            };
            match free_note(&notes, *at, if *before { -1.0 } else { 1.0 }) {
                Some(n) => n,
                None => {
                    let side = if *before { "before" } else { "after" };
                    return fail(format!("no free note {side} {symbol}'s {}", k7x::edit::js_number(*at)));
                }
            }
        }
        None => notes
            .iter()
            .copied()
            .reduce(f64::max)
            .map_or(RACK_FIRST_NOTE, |m| (m + 1.0).min(127.0)),
    };
    edit_taps(fs, env, dir, node, |doc, base| {
        let v = Value::Map(vec![("note".to_owned(), num(note))]);
        Ok(doc.set_in(&at_of(base, &[&name]), &v, WriteOptions { inline: true })?)
    })?;
    Ok((name, note))
}

/// Rename a tap the node itself states, in place; an inherited one is refused.
///
/// # Errors
/// An unknown, equal, taken or ill-formed name, or a tap the node does not state.
pub fn rename_tap(fs: &mut dyn Fs, env: &Env, dir: &str, node: &str, from: &str, to: &str) -> Res<String> {
    let kit = notes_of(&kit_of(fs, env, dir, node)?);
    let has = |s: &str| kit.iter().any(|(k, _)| k == s);
    if !has(from) {
        return fail(format!("'{node}' has no tap '{from}'"));
    }
    if from == to {
        return fail(format!("'{from}' is already called that"));
    }
    if has(to) {
        return fail(format!("'{node}' already has a tap '{to}'"));
    }
    if !tap_name_ok(to) {
        return fail(bad_name(to));
    }
    let mut renamed = false;
    let path = edit_taps(fs, env, dir, node, |doc, base| {
        let p = at_of(base, &[from]);
        if !doc.has_in(&p)? {
            return Ok(());
        }
        doc.rename_key(&p, to)?;
        renamed = true;
        Ok(())
    })?;
    if !renamed {
        return fail(format!(
            "'{from}' comes from {node}'s module, not from the node — rename it there, or give the node its own kit"
        ));
    }
    Ok(path)
}

/// Remove a tap the node itself states; one it inherits from its module is refused by name.
///
/// # Errors
/// An unknown tap, or one the node does not state.
pub fn remove_tap(fs: &mut dyn Fs, env: &Env, dir: &str, node: &str, symbol: &str) -> Res<String> {
    let kit = notes_of(&kit_of(fs, env, dir, node)?);
    if !kit.iter().any(|(k, _)| k == symbol) {
        return fail(format!("'{node}' has no tap '{symbol}'"));
    }
    let mut removed = false;
    let path = edit_taps(fs, env, dir, node, |doc, base| {
        let p = at_of(base, &[symbol]);
        if !doc.has_in(&p)? {
            return Ok(());
        }
        doc.delete_in(&p, false)?;
        removed = true;
        Ok(())
    })?;
    if !removed {
        return fail(format!(
            "'{symbol}' comes from {node}'s module, not from the node — remove it there, or give the node its own kit"
        ));
    }
    Ok(path)
}

/// Replace the kit the node itself states with `taps`; a module's own taps still merge in under it.
///
/// # Errors
/// As `kit_of`, or the document's writers.
pub fn set_kit(
    fs: &mut dyn Fs,
    env: &Env,
    dir: &str,
    node: &str,
    taps: &[(String, f64, Option<String>)],
) -> Res<String> {
    kit_of(fs, env, dir, node)?;
    let kit = Value::Map(
        taps.iter()
            .map(|(s, note, sample)| {
                let mut f = vec![("note".to_owned(), num(*note))];
                if let Some(x) = sample.as_ref().filter(|x| !x.is_empty()) {
                    f.push(("sample".to_owned(), Value::Str(x.clone())));
                }
                (s.clone(), Value::Map(f))
            })
            .collect(),
    );
    edit_taps(fs, env, dir, node, |doc, base| {
        if taps.is_empty() {
            doc.delete_in(base, true)?;
        } else if doc.has_in(base)? {
            // Applied, not replaced: a tap the new kit keeps as it was keeps its line and its comment.
            doc.apply_in(base, &kit)?;
        } else {
            doc.set_in(base, &kit, WriteOptions::default())?;
        }
        Ok(())
    })
}

/// A node's taps by name: a rack's sounds, or an SFZ kit's pads; none for any other node.
///
/// # Errors
/// A node the project lacks.
pub fn taps_of(fs: &dyn Fs, env: &Env, dir: &str, node: &str) -> Res<Vec<String>> {
    let Some(r) = resolve_node(fs, env, dir, node)? else {
        return fail(format!("no node '{node}'"));
    };
    if r.authored.module_type == Some(ModuleType::Rack) {
        return Ok(r.authored.rack.iter().flat_map(|k| k.symbols.keys().cloned()).collect());
    }
    Ok(r.authored.kit.iter().flat_map(|k| k.0.keys().cloned()).collect())
}

/// Route a tap to a subchain of the node's chain file; `None` returns it to the main output.
///
/// # Errors
/// A node with no taps, an unknown tap, or a subchain the chain file lacks or runs in place.
pub fn set_tap_out(fs: &mut dyn Fs, env: &Env, dir: &str, node: &str, tap: &str, sub: Option<&str>) -> Res<String> {
    let taps = taps_of(fs, env, dir, node)?;
    if taps.is_empty() {
        return fail(format!("'{node}' has no taps — only a rack or an SFZ kit routes one"));
    }
    if !taps.iter().any(|t| t == tap) {
        return fail(format!("'{node}' has no tap '{tap}'"));
    }
    if let Some(sub) = sub {
        let cp = read_chain_project(fs, &mut Cache::new(), dir).map_err(|e| crate::Error::Msg(e.to_string()))?;
        let at = cp.nodes.get(node);
        let parent = at.and_then(|a| cp.chains.get(&a.chain));
        if !parent.is_some_and(|p| p.subchains.iter().flatten().any(|s| s.id == sub)) {
            let of = at.map_or("the node's", |a| a.chain.as_str());
            return fail(format!("'{sub}' is not a subchain of {of} chain file"));
        }
        if parent.is_some_and(|p| p.called_subchains().contains(sub)) {
            return fail(format!("'{sub}' is run in place as a step — a tap cannot feed it"));
        }
    }
    edit_module_file(fs, env, dir, node, |doc| {
        let outs = [key("tap_outs")];
        let at = [key("tap_outs"), key(tap)];
        match sub {
            None => {
                doc.delete_in(&at, true)?;
            }
            Some(s) if matches!(doc.get_in(&outs)?, Some(Value::Map(_))) => {
                doc.set_in(&at, &Value::Str(s.to_owned()), WriteOptions::default())?;
            }
            Some(s) => {
                let v = Value::Map(vec![(tap.to_owned(), Value::Str(s.to_owned()))]);
                doc.set_in(&outs, &v, WriteOptions { inline: true })?;
            }
        }
        Ok(())
    })
}
