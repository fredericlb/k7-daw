// emit.rs — a Song (from any mapper) written out as a node/lane/chain project: project.k7x, graph.k7x, a module per plugin, nodes, clips.
// What a reader gets wrong: nothing here touches a disk, it answers the files to write; the caller lands them (docs/import.md).

use std::collections::HashSet;

use k7x::edit::Value;

use crate::emit_audio::{emit_arrangement, track_value};
use crate::emit_ctx::{DocLayout, EmitCtx, OutDef, Written, number, obj, text};
use crate::emit_graph::{MIX_NODE, OUT_NODE, add_chain, dedupe_outs, drop_dead_buses, write_graph};
use crate::emit_modules::{collapse_sole_inserts, drop_orphan_inserts, order_plugins, write_modules};
use crate::model::{Bytes, ClipKind, Song};

/// What an emit counted.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct EmitStats {
    pub track_files: usize,
    pub clip_files: usize,
    pub sequence_files: usize,
    pub chain_files: usize,
    pub module_nodes: usize,
    pub node_files: usize,
    pub module_preset_files: usize,
    pub missing_audio: Vec<String>,
    pub missing_plugins: Vec<String>,
    pub stand_ins: usize,
}

/// What the workspace already holds that an import must not shadow or contradict.
#[derive(Default)]
pub struct World {
    /// The lowercase names under the shared `_modules/{audio,instruments,transformers}`.
    pub taken_modules: HashSet<String>,
    /// The engine's plugin names, lowercase; none or empty reads as "unknown" and skips the check.
    pub plugins: Option<HashSet<String>>,
}

const SURFACE_NOTE: &str = "The ARRANGEMENT — the timeline, and the transport that rolls through it.\n\n\
Written by the import: what a .dawproject or .als carries is lanes on a ruler, so the\n\
project that comes out of one draws them. Delete this file and the song still compiles\n\
and still plays; you simply have no lanes on screen.";

/// The files of `song` as a project named `project`, below the workspace's root.
pub fn emit(project: &str, song: &Song, assets: &[(String, Bytes)], world: &World) -> (Written, EmitStats) {
    let mut ctx = EmitCtx::new(project, song, assets, &world.taken_modules);
    let master = song.tracks.iter().find(|t| t.role.as_deref() == Some("master"));
    let master_plugins = order_plugins(master.map(|m| m.plugins.as_slice()).unwrap_or_default());
    // Where a lane ends: the mixer's SUM, like every k7 project.
    let master_chain = master
        .filter(|_| !master_plugins.is_empty())
        .and_then(|m| ctx.slug_of.get(&m.id).cloned());

    let mut track_files = 0;
    for t in &song.tracks {
        if t.role.as_deref() == Some("master") {
            continue;
        }
        let slug = ctx.slug_of.get(&t.id).cloned().unwrap_or_default();
        let is_bus = t.role.as_deref() == Some("effect");
        // Dry output goes to the master at full gain; each enabled send fans out to its bus at the send's level.
        let mut terminal = vec![OutDef { to: MIX_NODE.to_owned(), gain: None }];
        for s in t.sends.iter().filter(|s| s.enabled) {
            let to = ctx.slug_of.get(&s.to).cloned().unwrap_or_else(|| s.to.clone());
            if !to.is_empty() {
                terminal.push(OutDef { to, gain: Some(s.level) });
            }
        }
        let terminal = dedupe_outs(terminal);
        // Audio and no MIDI means a bounced track: its instrument and note FX are dropped, the audio FX kept.
        let has_midi = t.clips.iter().any(|c| c.kind == ClipKind::Midi);
        let audio_source = !is_bus && !has_midi && t.clips.iter().any(|c| c.kind != ClipKind::Midi);
        let plugins: Vec<_> = if audio_source {
            t.plugins
                .iter()
                .filter(|p| !matches!(p.role.as_deref(), Some("instrument" | "noteFX")))
                .cloned()
                .collect()
        } else {
            t.plugins.clone()
        };
        // Notes and nothing to play them: the instrument stayed behind in the source DAW.
        let needs_instrument =
            !is_bus && has_midi && !plugins.iter().any(|p| p.role.as_deref() == Some("instrument"));
        add_chain(
            &mut ctx,
            &slug,
            (!is_bus).then_some(slug.as_str()),
            &plugins,
            terminal,
            needs_instrument.then_some(t.drums),
        );
        if is_bus {
            ctx.bus_chains.insert(slug.clone());
        }
        if !is_bus {
            let placements = emit_arrangement(&mut ctx, t, &slug);
            let layout = DocLayout { items_one_line: vec![vec!["events".into()]], ..DocLayout::default() };
            ctx.write_doc(&format!("nodes/{slug}.k7x"), &track_value(t, placements), &layout);
            track_files += 1;
        }
    }

    // The master chain, only when it has plugins: else MIX and OUT are a single wire.
    if let Some(m) = master.filter(|_| !master_plugins.is_empty()) {
        let slug = ctx.slug_of.get(&m.id).cloned().unwrap_or_default();
        add_chain(&mut ctx, &slug, None, &master_plugins, vec![OutDef { to: OUT_NODE.to_owned(), gain: None }], None);
    }

    drop_dead_buses(&mut ctx);
    drop_orphan_inserts(&mut ctx);
    collapse_sole_inserts(&mut ctx);
    write_graph(&mut ctx, master_chain.as_deref());
    let modules = write_modules(&mut ctx, world.plugins.as_ref());

    let header = obj(vec![(
        "project",
        obj(vec![
            ("name", text(&song.name)),
            ("tempo", number(song.tempo)),
            ("sampleRate", number(song.sample_rate)),
            ("timeSignature", text(&song.time_signature)),
        ]),
    )]);
    ctx.write_doc("project.k7x", &header, &DocLayout::default());
    // The surface an imported project needs to be LOOKED at: an arrangement.
    let surface: Value = obj(vec![("type", text("timeline")), ("layout", text("split"))]);
    ctx.write_doc(
        "surfaces/timeline.k7x",
        &surface,
        &DocLayout { note: Some(SURFACE_NOTE), ..DocLayout::default() },
    );

    let stats = EmitStats {
        track_files,
        clip_files: ctx.clip_files,
        sequence_files: ctx.sequence_files,
        chain_files: ctx.chains.len(),
        module_nodes: modules.module_nodes,
        node_files: modules.node_files,
        module_preset_files: modules.module_preset_files,
        missing_audio: std::mem::take(&mut ctx.missing_audio),
        missing_plugins: modules.missing_plugins,
        stand_ins: ctx.stand_ins,
    };
    (ctx.out, stats)
}
