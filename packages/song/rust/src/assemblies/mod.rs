// assemblies/mod.rs — a project's `assemblies/` read and resolved against the library's templates: boxes to draw, never to compile.
// A reader would get wrong that nothing here joins `Inputs`, `Lanes` or `Ownership` (278): an assembly is a DRAWING.
// The weight of what is COPIED is budgeted (276); a missing, refused or unreadable template costs its own box only (275).

pub mod file;

use crate::cache::Cache;
use crate::files::scalar::NonEmpty;
use crate::files::{Finding, Severity};
use crate::lanes::inputs::family_or_warning;
use crate::modules::PortType;
use crate::modules::assets::{Files, Places};
use crate::modules::types::library_ref_parts;
use crate::roots::{Roots, spell};
use crate::source::path::{PathRule, join};
use crate::source::{Kind, Source, SourceError};
pub use file::{
    AssemblyFile, AssemblyParam, AssemblyPlug, AssemblyTemplate, decode_assembly, decode_assembly_template,
};
use indexmap::IndexMap;
use serde::Serialize;
use std::sync::Arc;

/// The folder a project keeps its assemblies in, and the library family they are authored in.
pub const ASSEMBLIES_DIR: &str = "assemblies";
/// The file that makes a library folder an assembly rather than a chain template.
pub const ASSEMBLY_FILE: &str = "assembly.k7x";
/// What one call may copy: a member, a plug and a param each weigh `1 + bytes / 64` (276).
pub const BUDGET: usize = 1_000_000;

/// One assembly of a project as read: its id (the file's base), its file and its value.
#[derive(Clone, Debug)]
pub struct ProjectAssembly {
    pub id: String,
    pub file: String,
    pub assembly: Arc<AssemblyFile>,
}

/// Every assembly a project declares, in filename order, and what would not read.
#[derive(Debug, Default)]
pub struct ProjectAssemblies {
    pub assemblies: Vec<ProjectAssembly>,
    pub findings: Vec<Finding>,
}

/// Every assembly a project declares; an unreadable one is a finding and skipped (273).
pub fn project_assemblies(
    source: &dyn Source,
    cache: &mut Cache,
    project: &str,
) -> Result<ProjectAssemblies, SourceError> {
    let fam = family_or_warning::<AssemblyFile>(source, cache, project)?;
    let assemblies = fam
        .values
        .into_iter()
        .zip(fam.names)
        .map(|((id, assembly), name)| ProjectAssembly {
            id,
            file: format!("{ASSEMBLIES_DIR}/{name}"),
            assembly,
        })
        .collect();
    Ok(ProjectAssemblies {
        assemblies,
        findings: fam.findings,
    })
}

/// A segment of a template ref a folder may be called by: not `.` or `..`, and on Windows no `\` or `:` (274).
fn plain_segment(s: &str, rule: PathRule) -> bool {
    s != "." && s != ".." && !(rule.is_windows() && s.contains(['\\', ':']))
}

/// Whether every segment of a ref's path is plain; a run of `/` inside it is one separator, an empty first or last segment is refused (274).
fn plain_path(rest: &str, rule: PathRule) -> bool {
    let segments: Vec<&str> = rest.split('/').collect();
    let last = segments.len().saturating_sub(1);
    segments.iter().enumerate().all(|(i, s)| {
        if s.is_empty() {
            i > 0 && i < last
        } else {
            plain_segment(s, rule)
        }
    })
}

/// The folder a ref names, or None: `@k7-core/turntable` is the `k7-core` library's `assemblies/turntable` (274).
pub fn assembly_folder(
    source: &dyn Source,
    places: &Places,
    template_ref: &str,
) -> Result<Option<String>, SourceError> {
    let Some((lib, rest)) = library_ref_parts(template_ref) else {
        return Ok(None);
    };
    // `@project/` names the project's own folders, never a library's (295).
    if lib == "project" || !plain_path(rest, places.rule) {
        return Ok(None);
    }
    let mut files = Files::new(source, places, "");
    let Some(folder) = files.library_file(ASSEMBLIES_DIR, template_ref)? else {
        return Ok(None);
    };
    match source.stat(&join(&folder, ASSEMBLY_FILE)) {
        Ok(st) if st.kind == Kind::File => Ok(Some(folder)),
        Ok(_) | Err(SourceError::NotFound | SourceError::Io(_) | SourceError::Outside) => Ok(None),
        Err(e) => Err(e),
    }
}

/// One template as the library holds it.
#[derive(Clone, Debug)]
pub enum Loaded {
    /// The ref resolves to nothing: a WARNING, never fatal (275).
    Missing,
    /// A template; an unreadable one is an EMPTY one with its complaint attached, so the box still draws.
    Template {
        template: Arc<AssemblyTemplate>,
        folder: String,
        error: Option<Finding>,
    },
}

fn load_with(
    source: &dyn Source,
    cache: &mut Cache,
    places: &Places,
    template_ref: &str,
) -> Result<Loaded, SourceError> {
    let Some(folder) = assembly_folder(source, places, template_ref)? else {
        return Ok(Loaded::Missing);
    };
    let path = join(&folder, ASSEMBLY_FILE);
    let (template, error) = match cache.read::<AssemblyTemplate>(source, &path) {
        Ok(Ok(t)) => (t, None),
        Ok(Err(f)) => (Arc::new(AssemblyTemplate::default()), Some(f)),
        Err(SourceError::NotFound | SourceError::Outside) => return Ok(Loaded::Missing),
        Err(SourceError::Io(why)) => {
            let f = Finding {
                file: ASSEMBLY_FILE.to_owned(),
                line: 1,
                col: 1,
                message: format!(
                    "the template could not be read ({})",
                    why.chars().take(80).collect::<String>()
                ),
                severity: Severity::Error,
            };
            (Arc::new(AssemblyTemplate::default()), Some(f))
        }
        Err(e) => return Err(e),
    };
    Ok(Loaded::Template {
        template,
        folder,
        error,
    })
}

/// One template of a library, read through the cache.
pub fn load_template(
    source: &dyn Source,
    cache: &mut Cache,
    roots: &Roots,
    template_ref: &str,
) -> Result<Loaded, SourceError> {
    load_with(source, cache, &Places::of(roots, false)?, template_ref)
}

/// One PLUG, its member already resolved to the node this project calls it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AssemblyPort {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: PortType,
    pub node: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<String>,
    pub mandatory: bool,
}

/// One assembly on the model: the box, with every template name already turned into a node name.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Assembly {
    pub uses: String,
    /// What the box is CALLED: the template's `label`, else the instance's own id.
    pub label: String,
    pub members: IndexMap<String, String>,
    pub inputs: Vec<AssemblyPort>,
    pub outputs: Vec<AssemblyPort>,
    pub params: IndexMap<String, AssemblyParam>,
    /// The template resolved to nothing: the box draws from `members` alone.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub missing: Option<bool>,
}

/// Every assembly resolved, and what the reading of them found.
#[derive(Debug, Default)]
pub struct ResolvedAssemblies {
    pub assemblies: IndexMap<String, Assembly>,
    pub findings: Vec<Finding>,
    /// Whether the budget stopped the call: the instance it stopped at and every later one are absent.
    pub capped: bool,
    pub weight: usize,
}

/// What a copy of these strings weighs (276).
fn weigh(parts: &[&str]) -> usize {
    1 + parts.iter().map(|p| p.len()).sum::<usize>() / 64
}

fn finding(file: &str, severity: Severity, message: String) -> Finding {
    Finding {
        file: file.to_owned(),
        line: 1,
        col: 1,
        message,
        severity,
    }
}

/// The plugs of one side whose member this instance maps; the rest are dropped, each visit weighing one.
fn plugs(
    list: &Option<Vec<AssemblyPlug>>,
    members: &IndexMap<String, String>,
    spent: &mut usize,
) -> Option<Vec<AssemblyPort>> {
    let mut out = Vec::new();
    for p in list.iter().flatten() {
        let Some(node) = members.get(p.node.0.as_str()) else {
            *spent = spent.saturating_add(1);
            continue;
        };
        let port = p.port.as_ref().map(|n: &NonEmpty| n.0.as_str());
        *spent = spent.saturating_add(weigh(&[&p.name.0, node, port.unwrap_or("")]));
        if *spent > BUDGET {
            return None;
        }
        out.push(AssemblyPort {
            name: p.name.0.clone(),
            kind: p.kind,
            node: node.clone(),
            port: port.map(str::to_owned),
            mandatory: p.mandatory != Some(false),
        });
    }
    Some(out)
}

/// Every assembly a project declares, RESOLVED: what the graph carries and the issues check (275, 276).
pub fn resolve_assemblies(
    source: &dyn Source,
    cache: &mut Cache,
    roots: &Roots,
) -> Result<ResolvedAssemblies, SourceError> {
    let project = spell(&roots.project, roots.paths)?;
    let places = Places::of(roots, false)?;
    let read = project_assemblies(source, cache, &project)?;
    let mut out = ResolvedAssemblies {
        findings: read.findings,
        ..ResolvedAssemblies::default()
    };
    let mut spent = 0usize;
    for ProjectAssembly { id, file, assembly } in read.assemblies {
        let uses = assembly.uses.0.as_str();
        let loaded = load_with(source, cache, &places, uses)?;
        let (template, missing) = match &loaded {
            Loaded::Missing => {
                let message = format!(
                    "assembly '{}' uses '{}', which no library this project can see holds — its box draws its members with no plugs and no knobs",
                    id.chars().take(40).collect::<String>(),
                    uses.chars().take(40).collect::<String>()
                );
                out.findings.push(finding(&file, Severity::Warning, message));
                (Arc::new(AssemblyTemplate::default()), true)
            }
            Loaded::Template { template, error, .. } => {
                if let Some(f) = error {
                    let message = format!(
                        "assembly '{}': its template {} will not read — {}",
                        id.chars().take(40).collect::<String>(),
                        uses.chars().take(40).collect::<String>(),
                        f.message.chars().take(120).collect::<String>()
                    );
                    out.findings.push(finding(&file, Severity::Error, message));
                }
                (template.clone(), false)
            }
        };
        let members = &assembly.members;
        let label = template.label.as_ref().map_or(id.as_str(), |l| l.0.as_str());
        let mut here = spent.saturating_add(weigh(&[label]));
        for (k, v) in members {
            here = here.saturating_add(weigh(&[k, v]));
        }
        let built = (here <= BUDGET)
            .then(|| {
                let inputs = plugs(&template.inputs, members, &mut here)?;
                let outputs = plugs(&template.outputs, members, &mut here)?;
                let mut params = IndexMap::new();
                for (name, t) in template.params.iter().flatten() {
                    let Some(node) = members.get(t.node.0.as_str()) else {
                        here = here.saturating_add(1);
                        continue;
                    };
                    here = here.saturating_add(weigh(&[name, node, &t.param.0]));
                    if here > BUDGET {
                        return None;
                    }
                    params.insert(
                        name.clone(),
                        AssemblyParam {
                            node: NonEmpty(node.clone()),
                            param: t.param.clone(),
                        },
                    );
                }
                (here <= BUDGET).then_some((inputs, outputs, params))
            })
            .flatten();
        let Some((inputs, outputs, params)) = built else {
            out.capped = true;
            let message =
                format!("the assemblies hold more than {BUDGET} entries — this one and the rest are left out");
            out.findings.push(finding(&file, Severity::Error, message));
            out.weight = here;
            return Ok(out);
        };
        spent = here;
        out.assemblies.insert(
            id.clone(),
            Assembly {
                uses: uses.to_owned(),
                label: label.to_owned(),
                members: members.clone(),
                inputs,
                outputs,
                params,
                missing: missing.then_some(true),
            },
        );
    }
    out.weight = spent;
    Ok(out)
}

/// What the editor is TOLD about the assemblies: files that would not read, refs that resolve to nothing, templates that will not read.
pub fn assembly_issues(source: &dyn Source, cache: &mut Cache, roots: &Roots) -> Result<Vec<Finding>, SourceError> {
    Ok(resolve_assemblies(source, cache, roots)?.findings)
}
