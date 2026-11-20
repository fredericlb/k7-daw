// views/errors.rs — everything wrong with a project: a refusal's findings, else the warnings a clean compile still earns.
// A reader would get wrong that a refusal lists NOTHING but its own findings: the warnings below it are for a compile that passed.

use super::dead::{dead_automations, dead_project_fields};
use super::surfaces::{project_surfaces, surface_issues};
use super::{Diagnostic, MAX_DIAGNOSTICS};
use crate::assemblies::assembly_issues;
use crate::cache::Cache;
use crate::doc::{Compiled, Request, jsdsp_finding};
use crate::files::{FORMAT_LEVEL, Finding, Severity};
use crate::jsdsp::{JsDspGraph, jsdsp_graph};
use crate::lanes::inputs::{Inputs, load_inputs};
use crate::read::read_head;
use crate::resolve::plugin_findings;
use crate::roots::spell;
use crate::source::{Source, SourceError};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

/// The bytes a diagnostic's file or message keeps.
const TEXT_CUT: usize = 300;

/// A project written by a newer k7: the two levels the "update k7" message names.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", ts(rename_all = "camelCase"))]
pub struct FormatTooNew {
    pub file_level: u32,
    pub build_level: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", ts(rename_all = "camelCase"))]
pub struct ProjectErrors {
    pub errors: Vec<String>,
    pub diagnostics: Vec<Diagnostic>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub format_too_new: Option<FormatTooNew>,
}

/// A string cut to `TEXT_CUT` bytes, on a character.
fn bounded(s: &str) -> String {
    if s.len() <= TEXT_CUT {
        return s.to_owned();
    }
    // The ellipsis is three bytes, and the whole stays within the cut.
    let mut at = TEXT_CUT - 3;
    while !s.is_char_boundary(at) {
        at -= 1;
    }
    format!("{}…", &s[..at])
}

fn diagnostic(f: &Finding) -> Diagnostic {
    Diagnostic {
        file: bounded(&f.file),
        line: f.line,
        col: f.col,
        message: bounded(&f.message),
        severity: f.severity,
    }
}

/// Each finding once by (file, line, col, message), in order, with the flat `<file>: <message>` list when `flat`; the cut keeps the first `MAX_DIAGNOSTICS` and says how many it left out.
fn listed(all: &[Finding], flat: bool) -> (Vec<Diagnostic>, Vec<String>) {
    let mut seen: HashSet<(&str, u32, u32, &str)> = HashSet::new();
    let mut diagnostics = Vec::new();
    let mut errors = Vec::new();
    let mut left = 0usize;
    for f in all {
        if !seen.insert((f.file.as_str(), f.line, f.col, f.message.as_str())) {
            continue;
        }
        if diagnostics.len() >= MAX_DIAGNOSTICS {
            left += 1;
            continue;
        }
        let d = diagnostic(f);
        if flat {
            // A refusal that names no file is its sentence alone, as the TypeScript lists it.
            errors.push(if d.file.is_empty() {
                d.message.clone()
            } else {
                format!("{}: {}", d.file, d.message)
            });
        }
        diagnostics.push(d);
    }
    if left > 0 {
        let message = format!("{left} more findings are not listed");
        if flat {
            errors.push(message.clone());
        }
        diagnostics.push(Diagnostic {
            file: "project.k7x".to_owned(),
            line: 1,
            col: 1,
            message,
            severity: Severity::Warning,
        });
    }
    (diagnostics, errors)
}

/// The levels of a format refusal, read off the head through the cache (decision 641).
fn too_new(source: &dyn Source, cache: &mut Cache, project: &str) -> Result<Option<FormatTooNew>, SourceError> {
    let head = read_head(source, cache, project)?;
    let level = head.root.as_deref().and_then(|r| r.too_new.as_ref()).map(|t| t.level);
    let clamp = |n: u64| u32::try_from(n).unwrap_or(u32::MAX);
    Ok(level.map(|l| FormatTooNew {
        file_level: clamp(l),
        build_level: clamp(FORMAT_LEVEL),
    }))
}

/// The graph finding of every resolved project module with a jsdsp source, built into the document or not (decision 639).
fn jsdsp_findings(source: &dyn Source, inputs: &Inputs) -> Result<Vec<Finding>, SourceError> {
    let mut graphs: HashMap<&str, JsDspGraph> = HashMap::new();
    let mut out = Vec::new();
    for (name, m) in &inputs.local.modules {
        let Some(src) = m.jsdsp.as_deref().filter(|s| !s.is_empty()) else {
            continue;
        };
        if m.authored.plugin.is_none() {
            continue;
        }
        if !graphs.contains_key(src) {
            graphs.insert(src, jsdsp_graph(source, src)?);
        }
        out.extend(jsdsp_finding(&graphs[src], &m.own_label(), name));
    }
    Ok(out)
}

/// Everything wrong with a project (decisions 639 to 641); `inputs` are the compile's reads when the caller holds them.
pub fn project_errors(
    source: &dyn Source,
    cache: &mut Cache,
    req: &Request,
    compiled: &Compiled,
    inputs: Option<&Inputs>,
) -> Result<ProjectErrors, SourceError> {
    let project = spell(&req.roots.project, req.roots.paths)?;
    if let Err(failure) = &compiled.engine {
        let (diagnostics, errors) = listed(&failure.diagnostics, true);
        let format_too_new = if failure.format_too_new {
            too_new(source, cache, &project)?
        } else {
            None
        };
        return Ok(ProjectErrors {
            errors,
            diagnostics,
            format_too_new,
        });
    }
    let owned;
    let inputs = match inputs {
        Some(i) => i,
        None => {
            owned = load_inputs(source, cache, &req.roots, req.rig.clone())?;
            &owned
        }
    };
    let mut all = plugin_findings(&inputs.nodes.nodes, &inputs.local.modules);
    all.extend(jsdsp_findings(source, inputs)?);
    all.extend(compiled.diagnostics.iter().cloned());
    if let Ok(doc) = &compiled.engine {
        all.extend(dead_automations(source, &project, doc, req.roots.paths)?);
    }
    all.extend(inputs.vocabulary.findings.iter().cloned());
    all.extend(dead_project_fields(source, cache, &project)?);
    let surfaces = project_surfaces(source, cache, &project)?;
    all.extend(surface_issues(source, inputs, &surfaces)?);
    all.extend(assembly_issues(source, cache, &req.roots)?);
    let (diagnostics, errors) = listed(&all, false);
    Ok(ProjectErrors {
        errors,
        diagnostics,
        format_too_new: None,
    })
}
