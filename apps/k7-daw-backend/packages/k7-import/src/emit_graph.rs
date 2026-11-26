// emit_graph.rs — the ROUTING an import writes: one chain per source track, a strip last on every lane, a stand-in instrument.
// What a reader gets wrong: graph.k7x holds SEGMENTS, so a send's LEVEL is lost on the way (docs/import.md).

use std::collections::HashSet;

use indexmap::IndexMap;
use k7x::edit::Value;

use crate::emit_ctx::{ChainDef, DocLayout, EmitCtx, OutDef, Step, obj, text};
use crate::emit_modules::{module_ref_for, order_plugins};
use crate::model::PluginOut;

pub const MIX_NODE: &str = "MIX";
pub const OUT_NODE: &str = "OUT";
/// The library's own mixer: every mixer node of every project is a use of it.
pub const SHARED_STRIP: &str = "@meta/audio/mixer";
/// What a MIDI lane plays when its own instrument could not cross over: the SFZ kit for drums, else the synth.
const STAND_IN_KIT: &str = "@k7-core/instruments/k7-rack";
const STAND_IN_SYNTH: &str = "@k7-core/instruments/synth/surge-xt";

/// The first mention of each destination: a track's dry output and its sends.
#[must_use]
pub fn dedupe_outs(outs: Vec<OutDef>) -> Vec<OutDef> {
    let mut seen = HashSet::new();
    outs.into_iter()
        .filter(|o| seen.insert(o.to.clone()))
        .collect()
}

type Segments = IndexMap<String, Vec<String>>;

/// The chains as SEGMENTS: who feeds whom, which is how the imported sends survive.
fn chain_segments(chains: &IndexMap<String, ChainDef>) -> (Segments, Segments, Vec<String>) {
    let mut tracks = Segments::new();
    let mut modules = Segments::new();
    let mut bypassed = Vec::new();

    // A sink (OUT) or a module is its own entry; a chain's is where it starts.
    fn entry_of(
        chains: &IndexMap<String, ChainDef>,
        name: &str,
        seen: &mut HashSet<String>,
    ) -> Option<String> {
        let Some(d) = chains.get(name) else {
            return Some(name.to_owned());
        };
        if !seen.insert(name.to_owned()) {
            return Some(name.to_owned());
        }
        if let Some(first) = d.uses.first() {
            return Some(first.module.clone());
        }
        let next = d.out.first()?;
        entry_of(chains, &next.to, seen)
    }
    fn link(into: &mut Segments, node: &str, tos: &[Option<String>]) {
        let list = into.entry(node.to_owned()).or_default();
        for to in tos.iter().flatten() {
            if !list.contains(to) {
                list.push(to.clone());
            }
        }
    }

    for d in chains.values() {
        let path: Vec<&str> = d.uses.iter().map(|s| s.module.as_str()).collect();
        let ends: Vec<Option<String>> = d
            .out
            .iter()
            .map(|o| entry_of(chains, &o.to, &mut HashSet::new()))
            .collect();
        if let Some(from) = &d.from {
            let first = path.first().map(|p| vec![Some((*p).to_owned())]);
            link(&mut tracks, from, first.as_deref().unwrap_or(&ends));
        } else if let Some(first) = path.first() {
            link(&mut modules, first, &[]);
        }
        for (i, m) in path.iter().enumerate() {
            let next: Vec<Option<String>> = match path.get(i + 1) {
                Some(n) => vec![Some((*n).to_owned())],
                None => ends.clone(),
            };
            link(&mut modules, m, &next);
        }
        for s in &d.uses {
            if s.enabled == Some(false) || d.enabled == Some(false) {
                bypassed.push(s.module.clone());
            }
        }
    }
    (tracks, modules, bypassed)
}

fn taken(ctx: &EmitCtx<'_>, name: &str) -> bool {
    ctx.used_chain_names.contains(&name.to_lowercase())
        || ctx.used_node_names.contains(&name.to_lowercase())
}

/// A lane's strip, last in its run: a node on the LIBRARY's mixer.
fn add_track_mixer(ctx: &mut EmitCtx<'_>, entry: &str) -> String {
    let mut name = format!("mix-{entry}");
    let mut n = 2;
    while taken(ctx, &name) {
        name = format!("mix-{entry}-{n}");
        n += 1;
    }
    ctx.used_chain_names.insert(name.to_lowercase());
    ctx.used_node_names.insert(name.to_lowercase());
    let note = format!(
        "{entry}'s strip: volume, pan and three bands, and the one place its level lives.\n\n\
         Written by the import, because a lane without one plays at unity with nowhere to put a\n\
         level — see apps/k7-daw-frontend/packages/song/docs/trackMixer.md."
    );
    ctx.write_doc(
        &format!("nodes/{name}.k7x"),
        &obj(vec![("extends", text(SHARED_STRIP))]),
        &DocLayout {
            note: Some(&note),
            ..DocLayout::default()
        },
    );
    name
}

/// A library instrument standing in for one that did not cross over.
fn add_stand_in(ctx: &mut EmitCtx<'_>, entry: &str, drums: bool) -> String {
    let mut name = format!("{entry}-instrument");
    let mut n = 2;
    while taken(ctx, &name) {
        name = format!("{entry}-instrument-{n}");
        n += 1;
    }
    ctx.used_node_names.insert(name.to_lowercase());
    let note = format!(
        "{entry}'s instrument, a STAND-IN: the source project's could not be carried over, so the\n\
         import put a library one here — replace it with the instrument this part was written for."
    );
    ctx.write_doc(
        &format!("nodes/{name}.k7x"),
        &obj(vec![(
            "extends",
            text(if drums { STAND_IN_KIT } else { STAND_IN_SYNTH }),
        )]),
        &DocLayout {
            note: Some(&note),
            ..DocLayout::default()
        },
    );
    ctx.stand_ins += 1;
    name
}

/// One chain per source: its ordered plugins become `use` steps, each naming a module node, routed to `terminal`.
pub fn add_chain(
    ctx: &mut EmitCtx<'_>,
    entry: &str,
    from_track: Option<&str>,
    plugins: &[PluginOut],
    terminal: Vec<OutDef>,
    stand_in: Option<bool>,
) {
    ctx.used_chain_names.insert(entry.to_lowercase());
    let mut uses: Vec<Step> = Vec::new();
    for p in order_plugins(plugins) {
        uses.push(Step {
            module: module_ref_for(ctx, &p, entry),
            enabled: (!p.enabled).then_some(false),
        });
    }
    // The stand-in leads, where the instrument the source had would have stood.
    if let Some(drums) = stand_in {
        uses.insert(
            0,
            Step {
                module: add_stand_in(ctx, entry, drums),
                enabled: None,
            },
        );
    }
    // A LANE gets its strip, last; a return bus and the master chain are not lanes and get none.
    if from_track.is_some() {
        uses.push(Step {
            module: add_track_mixer(ctx, entry),
            enabled: None,
        });
    }
    ctx.chains.insert(
        entry.to_owned(),
        ChainDef {
            from: from_track.map(str::to_owned),
            uses,
            enabled: None,
            out: terminal,
        },
    );
}

/// Drops dead return buses: a bus nothing routes into is unfed. Cascades, since one may feed another.
pub fn drop_dead_buses(ctx: &mut EmitCtx<'_>) {
    loop {
        let routed: HashSet<String> = ctx
            .chains
            .values()
            .flat_map(|d| d.out.iter().map(|o| o.to.clone()))
            .collect();
        let dead: Vec<String> = ctx
            .bus_chains
            .iter()
            .filter(|b| !routed.contains(*b))
            .cloned()
            .collect();
        if dead.is_empty() {
            return;
        }
        for name in dead {
            ctx.chains.shift_remove(&name);
            ctx.bus_chains.remove(&name);
        }
    }
}

fn list(items: &[String]) -> Value {
    Value::List(items.iter().map(|s| text(s)).collect())
}

/// graph.k7x, read AFTER the collapse, which may have renamed the master run's first step onto its module.
pub fn write_graph(ctx: &mut EmitCtx<'_>, master_chain: Option<&str>) {
    let (tracks, modules, bypassed) = chain_segments(&ctx.chains);
    let weighted: usize = ctx
        .chains
        .values()
        .map(|d| {
            d.out
                .iter()
                .filter(|o| o.gain.is_some_and(|g| g != 1.0))
                .count()
        })
        .sum();
    // What the sum runs into: the master run's first module, or OUT when nothing treats the whole.
    let mix_sink = master_chain
        .and_then(|m| ctx.chains.get(m))
        .and_then(|c| c.uses.first())
        .map_or_else(|| OUT_NODE.to_owned(), |s| s.module.clone());
    let mut nodes: Vec<(String, Value)> = Vec::new();
    for (k, v) in tracks.iter().chain(modules.iter()) {
        nodes.push((k.clone(), list(v)));
    }
    nodes.push((MIX_NODE.to_owned(), list(&[mix_sink])));
    let node_map = Value::object(nodes);
    let node_keys: Vec<String> = match &node_map {
        Value::Map(m) => m.iter().map(|(k, _)| k.clone()).collect(),
        _ => Vec::new(),
    };
    let mut seg = vec![("modules", node_map)];
    // A lane the import found MUTED is bypassed, which is where a lane's mute lives.
    let song = ctx.song;
    let muted: Vec<String> = song
        .tracks
        .iter()
        .filter(|t| t.mute)
        .filter_map(|t| ctx.slug_of.get(&t.id))
        .filter(|s| !ctx.bus_chains.contains(*s))
        .cloned()
        .collect();
    let all: Vec<String> = bypassed.into_iter().chain(muted).collect();
    if !all.is_empty() {
        seg.push(("bypassed", list(&all)));
    }
    let note = (weighted > 0).then(|| {
        format!(
            "NOTE: {weighted} weighted send(s) arrived at full level — a destination says\n\
             WHERE the signal goes, not how much of it. Trim them on the bus's own module."
        )
    });
    let mut one_line: Vec<Vec<String>> = node_keys
        .into_iter()
        .map(|k| vec!["modules".to_owned(), k])
        .collect();
    one_line.push(vec!["bypassed".to_owned()]);
    ctx.write_doc(
        "graph.k7x",
        &obj(seg),
        &DocLayout {
            note: note.as_deref(),
            one_line,
            items_one_line: Vec::new(),
        },
    );
}
