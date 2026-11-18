// doc/compile.rs — the whole compile: the inputs read once, a live pass, the automatic freezes, a second pass when a render stands in.
// A reader would get wrong that the compile writes nothing: a stale graph is a finding, a render is a stat, and the host touches what it used.

use super::ProjectDoc;
use super::build::{Built, Pass, build_doc, new_freeze_budget};
use super::lane_tracks::read_patterns;
use super::lanes::Halves;
use super::notes::{Notes, finding};
use super::recorder::{records_output, with_output_recorder};
use crate::cache::Cache;
use crate::clips::{Budget, Grid, Materials};
use crate::console::console_of;
use crate::files::{Finding, Severity, time_sig_of};
use crate::freeze::{AutoFreeze, AutoInput, Edges, auto_freezes};
use crate::jsorder::own_key_order;
use crate::lanes::inputs::{Inputs, load_inputs};
use crate::lanes::live::live_inputs_of;
use crate::lanes::ownership::chain_ownership;
use crate::lanes::references::{check_references, finding_of};
use crate::lanes::rig::Rig;
use crate::modules::assets::Places;
use crate::read::read_head;
use crate::roots::Roots;
use crate::source::path::join;
use crate::source::{Kind, Source, SourceError};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Whether the engine of this compile hosts VSTs: a share's engine has none (docs/autoFreeze.md).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Vsts {
    #[default]
    Scan,
    None,
}

/// What a compile is asked: where to read, the rig, the engine's VSTs, and the modules the host holds thawed (decision 303).
#[derive(Clone, Debug, Default)]
pub struct Request {
    pub roots: Roots,
    pub rig: Rig,
    pub vsts: Vsts,
    pub thawed: Vec<String>,
}

/// Why no document was made: the findings that stop the compile.
#[derive(Clone, Debug, PartialEq)]
pub struct Failure {
    pub diagnostics: Vec<Finding>,
    pub format_too_new: bool,
}

/// What each budget of the compile spent, for the gate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Spent {
    pub doc: usize,
    pub placer: usize,
    pub baker: usize,
    pub freeze: usize,
}

impl Spent {
    fn max(self, o: Spent) -> Spent {
        Spent {
            doc: self.doc.max(o.doc),
            placer: self.placer.max(o.placer),
            baker: self.baker.max(o.baker),
            freeze: self.freeze.max(o.freeze),
        }
    }
}

/// A compile's answer.
#[derive(Clone, Debug)]
pub struct Compiled {
    pub engine: Result<ProjectDoc, Failure>,
    /// Every finding the compile made, stopping or not.
    pub diagnostics: Vec<Finding>,
    pub freezes: Vec<AutoFreeze>,
    pub renders_used: Vec<String>,
    pub spent: Spent,
}

fn failed(diagnostics: Vec<Finding>, format_too_new: bool, spent: Spent) -> Compiled {
    Compiled {
        engine: Err(Failure {
            diagnostics: diagnostics.clone(),
            format_too_new,
        }),
        diagnostics,
        freezes: Vec::new(),
        renders_used: Vec::new(),
        spent,
    }
}

/// The refusal of a project root that is no folder holding a `project.k7x` and no `.k7x` file, as the TypeScript says it, with no file to name (decision 811).
fn unsupported(source: &dyn Source, project: &str) -> Result<Option<Finding>, SourceError> {
    let refused = match source.stat(project) {
        Err(SourceError::NotFound) => true,
        Ok(st) if st.kind == Kind::File => !project.to_ascii_lowercase().ends_with(".k7x"),
        Ok(_) => match source.stat(&join(project, "project.k7x")) {
            Err(SourceError::NotFound) => true,
            Err(SourceError::Missing) => return Err(SourceError::Missing),
            _ => false,
        },
        Err(SourceError::Missing) => return Err(SourceError::Missing),
        Err(_) => false,
    };
    let name = project.rsplit('/').next().unwrap_or(project);
    Ok(refused.then(|| {
        finding(
            "",
            Severity::Error,
            format!("unsupported song '{name}' — use a .k7x file or a folder project"),
        )
    }))
}

/// Whether the folder is a graph project: a `graph.k7x`, `modules/` or `chains/` (decision 305).
fn is_project(source: &dyn Source, project: &str) -> Result<bool, SourceError> {
    for name in ["graph.k7x", "modules", "chains"] {
        match source.stat(&join(project, name)) {
            Ok(_) => return Ok(true),
            Err(SourceError::Missing) => return Err(SourceError::Missing),
            Err(_) => {}
        }
    }
    Ok(false)
}

/// Every finding the reads made, errors and warnings, once.
fn read_findings(i: &Inputs) -> Vec<Finding> {
    let lists = [
        &i.head.findings,
        &i.head.compile_findings,
        &i.routing.findings,
        &i.nodes.findings,
        &i.nodes.warnings,
        &i.vocabulary.findings,
        &i.local.findings,
        &i.extra.findings,
        &i.clips.findings,
        &i.sequences.findings,
        &i.timelines.findings,
    ];
    lists.into_iter().flatten().cloned().collect()
}

/// Dedupes by file, severity and message, keeping the first.
fn once(list: Vec<Finding>) -> Vec<Finding> {
    let mut seen: HashSet<(String, bool, String)> = HashSet::new();
    list.into_iter()
        .filter(|f| seen.insert((f.file.clone(), f.severity == Severity::Error, f.message.clone())))
        .collect()
}

/// Compiles a project to the engine's document (decisions 300 to 308); `Err` is `Missing` alone.
pub fn compile(source: &dyn Source, cache: &mut Cache, req: &Request) -> Result<Compiled, SourceError> {
    compile_keeping(source, cache, req).map(|(compiled, _)| compiled)
}

/// `compile`, and the reads it made, which a view of the same project takes as they are (decision 642); none for a folder that is no project.
pub fn compile_keeping(
    source: &dyn Source,
    cache: &mut Cache,
    req: &Request,
) -> Result<(Compiled, Option<Inputs>), SourceError> {
    let project = crate::roots::spell(&req.roots.project, req.roots.paths)?;
    if let Some(f) = unsupported(source, &project)? {
        return Ok((failed(vec![f], false, Spent::default()), None));
    }
    if !is_project(source, &project)? {
        let f = finding(
            "project.k7x",
            Severity::Error,
            "not a k7 project: it has no graph.k7x, modules or chains",
        );
        let too_new = read_head(source, cache, &project)?.format_too_new;
        return Ok((failed(vec![f], too_new, Spent::default()), None));
    }
    let mut inputs = load_inputs(source, cache, &req.roots, req.rig.clone())?;
    // Object.entries lists integer-like names first, and the lanes, chains and buses follow it (decision 392).
    own_key_order(&mut inputs.local.modules);
    own_key_order(&mut inputs.chains);
    let compiled = compile_from(source, cache, req, &inputs)?;
    Ok((compiled, Some(inputs)))
}

fn compile_from(
    source: &dyn Source,
    cache: &mut Cache,
    req: &Request,
    inputs: &Inputs,
) -> Result<Compiled, SourceError> {
    let lanes = inputs.lanes();
    let checked = check_references(source, inputs, &lanes)?;
    let own = chain_ownership(inputs, &lanes);
    let mut all = read_findings(inputs);
    let mut stops: Vec<Finding> = inputs.stops().into_iter().cloned().collect();
    stops.extend(checked.findings.iter().cloned());
    stops.extend(own.faults.iter().map(|f| finding_of(inputs, f)));
    let system = inputs
        .head
        .root
        .as_deref()
        .and_then(|r| r.project.as_ref())
        .and_then(|f| f.project.as_ref())
        .and_then(|p| p.console.as_ref())
        .map(|c| c.0.as_str());
    let console = console_of(source, &req.roots, &inputs.vocabulary, system)?;
    all.extend(console.findings.iter().cloned());
    stops.extend(console.fault.iter().map(|f| finding_of(inputs, f)));
    let file = inputs.head.root.as_deref().and_then(|r| r.project.as_ref());
    let grid = match file {
        Some(f) => Grid::of_project(f),
        None => time_sig_of(None).map(|s| Grid::new(s, 120.0)),
    };
    let Ok(grid) = grid else {
        all.extend(stops.iter().cloned());
        let refused = failed(once(stops), inputs.head.format_too_new, Spent::default());
        return Ok(refused.with_all(once(all)));
    };
    if inputs.head.format_too_new {
        all.extend(stops.iter().cloned());
        return Ok(failed(once(stops), true, Spent::default()).with_all(once(all)));
    }
    let patterns = read_patterns(source, &req.roots, &inputs.project, &lanes)?;
    all.extend(patterns.findings.iter().cloned());
    let live = live_inputs_of(inputs, &lanes, &own);
    let edges = Edges::of(&inputs.chains);
    let names: Vec<&str> = inputs
        .local
        .modules
        .keys()
        .chain(inputs.extra.modules.keys())
        .map(String::as_str)
        .collect();
    let halves = Halves::of(&inputs.chains);
    let materials = Materials::new(
        &inputs.clips,
        &inputs.sequences,
        &inputs.timelines,
        inputs.audio_paths.clone(),
    );
    let places = Places::of(&req.roots, true)?;
    let pass = Pass {
        source,
        inputs,
        lanes: &lanes,
        own: &own,
        edges: &edges,
        live: &live,
        console: &console,
        materials: &materials,
        patterns: &patterns,
        halves: &halves,
        grid,
        names: &names,
        places: &places,
        seqs: inputs.sequences.values.iter().map(|(id, _)| id.as_str()).collect(),
        clip_ids: inputs.clips.values.iter().map(|(id, _)| id.as_str()).collect(),
    };
    let mut freeze_budget = new_freeze_budget();
    let first = build_doc(&pass, cache, &HashMap::new(), &mut freeze_budget)?;
    let mut spent = first.spent;
    stops.extend(first.notes.stop.iter().cloned());
    all.extend(first.notes.stop.iter().chain(&first.notes.note).cloned());
    if !stops.is_empty() {
        all.extend(stops.iter().cloned());
        return Ok(Compiled {
            ..failed(once(stops), false, spent)
        }
        .with_all(once(all)));
    }
    let mut notes = Notes::default();
    let (freezes, capped) = auto_pass(source, cache, req, &pass, &first, &mut freeze_budget)?;
    spent.freeze = spent.freeze.max(freeze_budget.spent());
    if capped {
        notes.push(finding(
            "project.k7x",
            Severity::Warning,
            "the automatic freezes of this project need more work than one compile does, so it plays live",
        ));
    }
    let ready: HashMap<&str, &str> = freezes
        .iter()
        .filter(|f| f.plays_render())
        .map(|f| (f.module.as_str(), f.render.as_str()))
        .collect();
    let renders_used: Vec<String> = freezes
        .iter()
        .filter(|f| f.plays_render())
        .map(|f| f.render.clone())
        .collect();
    let mut built: Built = first;
    if !ready.is_empty() {
        let mut second_budget = new_freeze_budget();
        let second = build_doc(&pass, cache, &ready, &mut second_budget)?;
        spent = spent.max(second.spent);
        stops.extend(second.notes.stop.iter().cloned());
        all.extend(second.notes.stop.iter().chain(&second.notes.note).cloned());
        built = second;
    }
    all.extend(notes.note);
    if !stops.is_empty() {
        all.extend(stops.iter().cloned());
        return Ok(Compiled {
            ..failed(once(stops), false, spent)
        }
        .with_all(once(all)));
    }
    let mut doc = built.doc;
    doc.record_dir = join(&inputs.project, "clips");
    let mut rec = Notes::default();
    let records = records_output(source, &inputs.project, cache, &inputs.chains, &mut rec)?;
    all.extend(rec.note);
    with_output_recorder(&mut doc, records);
    Ok(Compiled {
        engine: Ok(doc),
        diagnostics: once(all),
        freezes,
        renders_used,
        spent,
    })
}

impl Compiled {
    fn with_all(mut self, all: Vec<Finding>) -> Compiled {
        self.diagnostics = all;
        self
    }
}

/// The automatic freezes against the live document; `capped` says the budget stopped them (decision 348).
fn auto_pass(
    source: &dyn Source,
    cache: &mut Cache,
    req: &Request,
    pass: &Pass<'_>,
    live: &Built,
    budget: &mut Budget,
) -> Result<(Vec<AutoFreeze>, bool), SourceError> {
    let thawed: HashSet<String> = req.thawed.iter().cloned().collect();
    let module = |n: &str| pass.inputs.module(n).map(|r| &**r);
    let is_lane = |n: &str| pass.lanes.sound.get(n).is_some_and(|l| !l.disabled);
    let is_strudel = |n: &str| {
        pass.lanes
            .sound
            .get(n)
            .is_some_and(|l| l.kind == crate::lanes::LaneKind::Strudel)
    };
    let input = AutoInput {
        project: &pass.inputs.project,
        renders: req.roots.renders.as_deref(),
        edges: pass.edges,
        names: pass.names,
        module: &module,
        is_lane: &is_lane,
        is_strudel: &is_strudel,
        manual: &live.plans,
        thawed: &thawed,
        scan: &pass.inputs.vocabulary.scan,
        no_vsts: req.vsts == Vsts::None,
        live: &live.doc,
        rule: req.roots.paths,
    };
    let found = auto_freezes(source, cache, &input, budget)?;
    Ok((found.freezes, found.capped))
}
