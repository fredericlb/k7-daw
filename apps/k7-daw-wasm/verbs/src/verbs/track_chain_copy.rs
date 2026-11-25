// track_chain_copy.rs — `track duplicate` in a chain project: the lane's chain copied from the lane on, under fresh ids, after trackChain.ts.
// A reader would get wrong what is shared: a copy never shares an id, nor a preset only its original names; a linked chain fed by it alone is copied too.

use super::shared::join;
use super::track_dup::Names;
use crate::host::{Host, Thrown};
use k7x::edit::{Document, PathSeg, Value};
use song_edit::chain_data::{Chain, Item, Link, Start, Step, id_of, is_track_item};
use song_edit::chain_item_edit::chain_item_paths;
use song_edit::chain_project::read_chain_project;
use song_edit::confine::confine_to;
use song_edit::format_chain::format_chain;
use song_edit::instance_name::instance_name;
use song_edit::workspace_doc::ws_write_text;
use song_edit::workspace_file::{k7x_data, ws_copy, ws_exists};
use std::collections::HashMap;

const VSTPRESETS: &str = "vstpresets";

/// The links out of a chain's own steps to OTHER chains; one naming its own subchain stays inside it.
fn links_of(c: &Chain) -> Vec<&Link> {
    let own: Vec<&str> = c.subchains.iter().map(|s| s.id.as_str()).collect();
    match c.steps.last() {
        Some(Step::End(l)) => l.iter().filter(|x| !own.contains(&x.chain.as_str())).collect(),
        _ => Vec::new(),
    }
}

fn vstpreset_of(i: &Item) -> Option<&str> {
    match i.get("vstpreset") {
        Some(Value::Str(s)) => Some(s),
        _ => None,
    }
}

/// Copy lane `lane`'s chain for `new_lane`, whose lane file and chain exist already; the files written.
pub fn duplicate_lane_chain(
    h: &mut Host,
    lane: &str,
    new_lane: &str,
    names: &mut Names,
) -> Result<Vec<String>, Thrown> {
    let dir = h.song_path.clone();
    let cp = read_chain_project(&*h.fs, &dir)?;
    let Some(at) = cp.nodes.get(lane).filter(|n| n.track.is_some()) else {
        return Ok(Vec::new());
    };
    if at.item.is_some() {
        return Err(Thrown(format!(
            "\"{lane}\" sits in a side-by-side step — not a lane chain"
        )));
    }
    let mut incoming: HashMap<&str, usize> = HashMap::new();
    for c in cp.chains.values() {
        for l in links_of(c) {
            *incoming.entry(l.chain.as_str()).or_default() += 1;
        }
    }
    let mut copies: Vec<(String, String)> = vec![(at.chain.clone(), new_lane.to_owned())];
    let mut i = 0;
    while i < copies.len() {
        let from = copies[i].0.clone();
        i += 1;
        let Some(c) = cp.chains.get(&from) else { continue };
        for l in links_of(c) {
            let Some(next) = cp.chains.get(&l.chain) else { continue };
            if copies.iter().any(|(f, _)| *f == l.chain) || next.start.is_some() {
                continue;
            }
            if incoming.get(l.chain.as_str()).copied().unwrap_or(0) != 1 {
                continue;
            }
            let n = instance_name(&l.chain, Some(new_lane), &|x| names.taken(&*h.fs, &dir, x));
            let n = names.claim(n);
            copies.push((l.chain.clone(), n));
        }
    }
    let mut users: HashMap<String, usize> = HashMap::new();
    for n in cp.nodes.values() {
        if let Some(f) = vstpreset_of(&n.value) {
            *users.entry(f.to_owned()).or_default() += 1;
        }
    }
    let names_of: HashMap<String, String> = copies.iter().cloned().collect();
    let mut written = Vec::new();
    for (from, to) in &copies {
        let src = &cp.chains[from];
        let first = if *from == at.chain { at.step } else { 0 };
        let mut ids: HashMap<String, String> = HashMap::new();
        let mut fresh = |id: &str, h: &mut Host| {
            ids.insert(id.to_owned(), (h.new_id)());
        };
        for s in src.steps.iter().skip(first) {
            for it in s.items() {
                fresh(id_of(it), h);
            }
        }
        for sc in &src.subchains {
            for s in &sc.steps {
                for it in s.items() {
                    fresh(id_of(it), h);
                }
            }
        }
        let mut doc = Document::new(&cp.texts[from]);
        for (id, path) in chain_item_paths(&doc)? {
            if let Some(f) = ids.get(&id) {
                let mut p = path.clone();
                p.push(PathSeg::Key("id".to_owned()));
                doc.set_in(&p, &Value::Str(f.clone()), Default::default())
                    .map_err(|e| Thrown(e.to_string()))?;
            }
        }
        let text = doc.text().replace(
            &format!("// {lane}'s channel strip"),
            &format!("// {new_lane}'s channel strip"),
        );
        let file = join(&dir, &format!("chains/{from}.k7x"));
        let parsed =
            Chain::from_value(&k7x_data(&file, &text).map_err(|_| Thrown(format!("chains/{from}.k7x: unreadable")))?);
        let own: Vec<String> = src.subchains.iter().map(|s| s.id.clone()).collect();
        let mut copy_item = |i: &Item, old: &Item, h: &mut Host| -> Result<Item, Thrown> {
            if is_track_item(i) {
                return Ok(Value::Map(vec![
                    ("id".to_owned(), Value::Str(id_of(i).to_owned())),
                    ("track".to_owned(), Value::Str(new_lane.to_owned())),
                ]));
            }
            let named = vstpreset_of(old);
            if named.is_some_and(|n| users.get(n).copied().unwrap_or(0) > 1) {
                return Ok(i.clone());
            }
            let file = named.map_or_else(|| format!("{}.vstpreset", id_of(old)), str::to_owned);
            let presets = join(&dir, VSTPRESETS);
            let own_file = format!("{}.vstpreset", id_of(i));
            if let Some(abs) = confine_to(&*h.fs, std::slice::from_ref(&presets), &join(&presets, &file))
                && ws_exists(&*h.fs, &abs)
            {
                ws_copy(h.fs, &abs, &join(&presets, &own_file))?;
                written.push(format!("{VSTPRESETS}/{own_file}"));
            }
            Ok(match i {
                Value::Map(m) => Value::Map(m.iter().filter(|(k, _)| k != "vstpreset").cloned().collect()),
                other => other.clone(),
            })
        };
        let mut copy_steps = |next: &[Step], prev: &[Step], h: &mut Host| -> Result<Vec<Step>, Thrown> {
            next.iter()
                .enumerate()
                .map(|(k, s)| {
                    let old = prev.get(k);
                    Ok(match s {
                        Step::End(links) => Step::End(
                            links
                                .iter()
                                .map(|l| {
                                    let mut l = l.clone();
                                    if !own.contains(&l.chain)
                                        && let Some(n) = names_of.get(&l.chain)
                                    {
                                        l.chain.clone_from(n);
                                    }
                                    l
                                })
                                .collect(),
                        ),
                        Step::Call(c) => Step::Call(c.clone()),
                        Step::Many(items) => {
                            let olds = old.map(Step::items).unwrap_or(&[]);
                            Step::Many(
                                items
                                    .iter()
                                    .enumerate()
                                    .map(|(j, it)| copy_item(it, olds.get(j).unwrap_or(it), h))
                                    .collect::<Result<_, _>>()?,
                            )
                        }
                        Step::One(it) => {
                            let o = old.and_then(|s| s.items().first()).unwrap_or(it);
                            Step::One(copy_item(it, o, h)?)
                        }
                    })
                })
                .collect()
        };
        let steps = copy_steps(
            parsed.steps.get(first..).unwrap_or(&[]),
            src.steps.get(first..).unwrap_or(&[]),
            h,
        )?;
        let mut subchains = Vec::new();
        for (j, sc) in parsed.subchains.iter().enumerate() {
            let prev = src.subchains.get(j).map_or(&[][..], |s| s.steps.as_slice());
            let mut c = sc.clone();
            c.steps = copy_steps(&sc.steps, prev, h)?;
            subchains.push(c);
        }
        let is_lane_chain = *from == at.chain;
        let c = Chain {
            start: is_lane_chain.then_some(Start::Own),
            ports: if is_lane_chain { Vec::new() } else { src.ports.clone() },
            bus_only: src.bus_only,
            hidden: false,
            input: None,
            steps,
            subchains,
        };
        ws_write_text(
            h.fs,
            &join(&dir, &format!("chains/{to}.k7x")),
            &format_chain(&c, &text)?,
        )?;
        written.push(format!("chains/{to}.k7x"));
    }
    Ok(written)
}
