// routing/project.rs — a project kept as `chains/*.k7x`: its chains, the chains set aside, and every node by its name.
// A reader would get wrong that a chain without a start that nothing links is hidden unless it is PLACED (segments.rs).

use super::chain_file::{ChainFile, Item};
use crate::cache::Cache;
use crate::files::{Finding, Severity};
use crate::modules::Module;
use crate::read::{k7x_base, read_family};
use crate::source::path::join;
use crate::source::{Kind, Source, SourceError};
use indexmap::IndexMap;
use std::sync::Arc;

/// The module a picture lane's file extends.
const PICTURE_TRACK_REF: &str = "@meta/picture-track";

/// The folders a chain project keeps its files in.
pub const CHAINS_DIR: &str = "chains";
pub const TRACKS_DIR: &str = "tracks";

/// One node as the loader sees it: where it sits, and what it says.
#[derive(Clone, Debug, PartialEq)]
pub struct ChainNode {
    pub name: String,
    pub id: String,
    pub chain: String,
    /// The subchain of `chain` it sits in, absent for the chain's own steps.
    pub sub: Option<String>,
    pub step: usize,
    /// Its index in a parallel step; none for a step holding one item.
    pub item: Option<usize>,
    /// The lane it IS, of either kind.
    pub track: Option<String>,
    /// Set on a PICTURE lane: it holds no module, and its placements are not in `tracks/`.
    pub picture: Option<bool>,
    pub value: Item,
}

#[derive(Clone, Debug, Default)]
pub struct ChainProject {
    pub chains: IndexMap<String, Arc<ChainFile>>,
    /// The chains marked `hidden`, set aside whole: no node of theirs is in `nodes`.
    pub hidden: IndexMap<String, Arc<ChainFile>>,
    pub nodes: IndexMap<String, ChainNode>,
    pub findings: Vec<Finding>,
}

/// Its presence IS the format: a project with `chains/` is read from it, and never from graph.k7x.
pub fn is_chain_project(source: &dyn Source, project: &str) -> Result<bool, SourceError> {
    match source.stat(&join(project, CHAINS_DIR)) {
        Ok(st) => Ok(st.kind == Kind::Dir),
        Err(SourceError::NotFound | SourceError::Io(_) | SourceError::Outside) => Ok(false),
        Err(e) => Err(e),
    }
}

/// The file a lane's own events live in.
pub fn lane_file(project: &str, lane: &str) -> String {
    join(&join(project, TRACKS_DIR), &format!("{lane}.k7x"))
}

/// Whether `tracks/<lane>.k7x` is a PICTURE lane: it decodes and extends `@meta/picture-track`.
fn is_picture_lane(source: &dyn Source, cache: &mut Cache, project: &str, lane: &str) -> Result<bool, SourceError> {
    match cache.read::<Module>(source, &lane_file(project, lane)) {
        Ok(Ok(m)) => Ok(m.extends.as_deref() == Some(PICTURE_TRACK_REF)),
        Ok(Err(_)) | Err(SourceError::NotFound | SourceError::Io(_) | SourceError::Outside) => Ok(false),
        Err(e) => Err(e),
    }
}

fn warning(file: &str, message: &str) -> Finding {
    Finding {
        file: file.to_owned(),
        line: 1,
        col: 1,
        message: message.to_owned(),
        severity: Severity::Warning,
    }
}

/// Every chain file, decoded; a file that fails is reported and left out rather than half-read.
pub fn read_chain_project(source: &dyn Source, cache: &mut Cache, project: &str) -> Result<ChainProject, SourceError> {
    let mut out = ChainProject::default();
    let fam = match read_family::<ChainFile>(source, cache, project) {
        Ok(f) => f,
        Err(SourceError::Outside) => {
            out.findings
                .push(warning(CHAINS_DIR, "outside the project's folders — skipped"));
            return Ok(out);
        }
        Err(e) => return Err(e),
    };
    // One file, one spelling: `a.K7X` is named `chains/a.k7x` here as everywhere below (own_label, the budget's stop).
    out.findings = fam.findings;
    for f in &mut out.findings {
        if let Some(base) = f.file.strip_prefix("chains/").and_then(k7x_base) {
            f.file = format!("{CHAINS_DIR}/{base}.k7x");
        }
    }
    // `polysynth-copy` sorts before `polysynth` by file name but not by chain name, and lanes follow this order.
    let mut ordered = fam.values;
    ordered.sort_by(|a, b| a.0.cmp(&b.0));
    for (name, value) in ordered {
        if value.hidden == Some(true) {
            out.hidden.insert(name, value);
            continue;
        }
        for (sub, steps) in value.step_lists() {
            for (s, step) in steps.iter().enumerate() {
                let parallel = matches!(step, super::chain_file::Step::Parallel(_));
                for (k, it) in step.items().iter().enumerate() {
                    let n = it.name().to_owned();
                    if let Some(first) = out.nodes.get(&n) {
                        let message = format!("\"{n}\" is already a node of chain \"{}\"", first.chain);
                        out.findings.push(Finding {
                            file: format!("{CHAINS_DIR}/{name}.k7x"),
                            line: 1,
                            col: 1,
                            message,
                            severity: Severity::Error,
                        });
                    }
                    let picture = match it {
                        Item::VideoTrack { .. } => true,
                        Item::Track { track, .. } => is_picture_lane(source, cache, project, track)?,
                        Item::Node(_) => false,
                    };
                    out.nodes.insert(
                        n.clone(),
                        ChainNode {
                            name: n,
                            id: it.id().to_owned(),
                            chain: name.clone(),
                            sub: sub.map(str::to_owned),
                            step: s,
                            item: parallel.then_some(k),
                            track: it.lane().map(str::to_owned),
                            picture: picture.then_some(true),
                            value: it.clone(),
                        },
                    );
                }
            }
        }
        out.chains.insert(name, value);
    }
    Ok(out)
}
