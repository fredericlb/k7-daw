// routing/load.rs — a project's routing: its chain files, or its graph.k7x, read into ONE segment graph through a Source.
// A reader would get wrong that a project with `chains/` is read only from there, a `graph.k7x` beside it ignored.

use super::layout::read_layout;
use super::project::{ChainProject, is_chain_project, read_chain_project};
use super::segments::to_segments;
use super::types::Segments;
use super::{Budget, Capped};
use crate::cache::Cache;
use crate::files::{Finding, Severity};
use crate::roots::{Roots, spell};
use crate::source::path::join;
use crate::source::{Kind, Source, SourceError};
use indexmap::IndexMap;

/// What a project's routing is: the segments, each link's level, and the chain project they came from.
#[derive(Debug, Default)]
pub struct Routing {
    pub chain_project: bool,
    pub segments: Segments,
    pub levels: IndexMap<String, f64>,
    pub project: Option<ChainProject>,
    pub findings: Vec<Finding>,
    /// Whether the budget stopped the call: the segments are then empty.
    pub capped: bool,
    pub weight: usize,
}

fn capped_finding(c: Capped) -> Finding {
    Finding {
        file: c.file,
        line: 1,
        col: 1,
        message: format!("the routing holds more than {} edges — it is left out", super::BUDGET),
        severity: Severity::Error,
    }
}

fn graph_routing(source: &dyn Source, cache: &mut Cache, project: &str, out: &mut Routing) -> Result<(), SourceError> {
    let path = join(project, "graph.k7x");
    match source.stat(&path) {
        Ok(s) if s.kind == Kind::File => {}
        Ok(_) | Err(SourceError::NotFound | SourceError::Io(_)) => return Ok(()),
        Err(SourceError::Outside) => {
            out.findings.push(Finding {
                file: "graph.k7x".to_owned(),
                line: 1,
                col: 1,
                message: "outside the project's folders — skipped".to_owned(),
                severity: Severity::Warning,
            });
            return Ok(());
        }
        Err(e) => return Err(e),
    }
    match cache.read::<Segments>(source, &path) {
        Ok(Ok(s)) => {
            out.weight = s.modules.iter().flat_map(IndexMap::values).map(|d| d.len() + 1).sum();
            out.segments = (*s).clone();
        }
        Ok(Err(f)) => out.findings.push(f.in_file("graph.k7x")),
        Err(SourceError::NotFound) => {}
        Err(e) => return Err(e),
    }
    Ok(())
}

/// The routing of the project at `roots.project`: a chain project is read from `chains/`, else from `graph.k7x`.
pub fn load_routing(source: &dyn Source, cache: &mut Cache, roots: &Roots) -> Result<Routing, SourceError> {
    let project = spell(&roots.project, roots.paths)?;
    let mut out = Routing::default();
    if !is_chain_project(source, &project)? {
        graph_routing(source, cache, &project, &mut out)?;
        return Ok(out);
    }
    out.chain_project = true;
    let cp = read_chain_project(source, cache, &project)?;
    out.findings.clone_from(&cp.findings);
    let (positions, bad) = read_layout(source, cache, &project)?;
    out.findings.extend(bad);
    let mut budget = Budget::default();
    let picture = |l: &str| cp.nodes.get(l).is_some_and(|n| n.picture == Some(true));
    match to_segments(&cp.chains, &picture, &positions, &mut budget) {
        Ok(built) => {
            out.segments = built.segments;
            out.levels = built.levels;
        }
        Err(c) => {
            out.capped = true;
            out.findings.push(capped_finding(c));
        }
    }
    out.weight = budget.used();
    out.project = Some(cp);
    Ok(out)
}
